use std::sync::Arc;
use std::path::{PathBuf, Path};
use std::collections::{BTreeMap, VecDeque, HashMap};
use std::mem;

use syncbox::{ThreadPool, TaskBox};
use eventual::{
    Future,
    Async,
    AsyncError,
    join,
    background,
    defer
};

use configuration::Configuration;
use dependency::Graph;
use rule::Rule;
use bind::{self, Bind};
use super::Job;

pub struct Manager {
    configuration: Arc<Configuration>,

    rules: HashMap<String, Arc<Rule>>,

    graph: Graph<String>,

    /// Dependency count of each bind
    dependencies: BTreeMap<String, usize>,

    /// Map of binds to the list of jobs that haven't been processed yet
    waiting: Vec<Job>,

    /// Finished dependencies
    finished: BTreeMap<String, Arc<Bind>>,

    // TODO
    // feels weird to have this here, but it's in-line with making
    // matching Patterns first-class
    /// Paths being considered
    paths: Arc<Vec<PathBuf>>,

    futures: VecDeque<Future<Bind, ::Error>>,
}

impl Manager {
    pub fn new(configuration: Arc<Configuration>) -> Manager {
        Manager {
            configuration: configuration,
            rules: HashMap::new(),
            graph: Graph::new(),
            dependencies: BTreeMap::new(),
            waiting: Vec::new(),
            finished: BTreeMap::new(),
            paths: Arc::new(Vec::new()),
            futures: VecDeque::new(),
        }
    }

    // TODO
    // it's probably beneficial to keep this stuff here
    // that way the files are only enumerated once and each handler
    // sees the same set of files, makes things slightly more
    // deterministic?

    /// Re-enumerate the paths in the input directory
    pub fn update_paths(&mut self) {
        use walker::Walker;

        self.paths = Arc::new({
            Walker::new(&self.configuration.input).unwrap()
            .filter_map(|p| {
                let path = p.unwrap().path();

                if let Some(ref ignore) = self.configuration.ignore {
                    if ignore.matches(&Path::new(path.file_name().unwrap())) {
                        return None;
                    }
                }

                ::std::fs::metadata(&path)
                .map(|m|
                     if m.is_file() { Some(path.to_path_buf()) }
                     else { None }
                 ).unwrap_or(None)
            })
            .collect()
        });
    }

    pub fn add(&mut self, rule: Arc<Rule>) {
        // prepare bind-data with the name and configuration
        let data = bind::Data::new(
            String::from(rule.name()),
            self.configuration.clone());
        let name = data.name.clone();

        // TODO
        // instead of rule_count == 0,
        // check if self.waiting.is_empty()?

        // construct job from bind-data, rule kind, rule handler, and paths
        // push it to waiting queue
        self.waiting.push(
            Job::new(
                data,
                rule.handler()));

        self.graph.add_node(name.clone());

        // make its dependencies depend on this binding
        for dep in rule.dependencies() {
            self.graph.add_edge(dep.clone(), name.clone());
        }

        self.rules.insert(name.clone(), rule);
    }

    // TODO: will need Borrow bound
    fn satisfy(&mut self, bind: &str) {
        if let Some(dependents) = self.graph.dependents_of(bind) {
            let names = self.dependencies.keys().cloned().collect::<Vec<String>>();

            for name in names {
                if dependents.contains(&name) {
                    // FIXME this or_insert is incorrect, since using 0
                    // and subtracting 1 will cause underflow
                    if let Some(count) = self.dependencies.get_mut(&name) {
                        *count -= 1;
                    } else {
                        panic!("dependency count for {} is not available!", name);
                    }
                }
            }
        }
    }

    fn ready(&mut self) -> Vec<Job> {
        let waiting = mem::replace(&mut self.waiting, Vec::new());

        let (ready, waiting): (Vec<Job>, Vec<Job>) =
            waiting.into_iter()
               .partition(|job| self.dependencies[&job.bind.name] == 0);

        self.waiting = waiting;

        ready
    }

    pub fn sort_jobs(&mut self, order: VecDeque<String>) {
        assert!(self.waiting.len() == order.len(), "`waiting` and `order` are not the same length");

        let mut job_map =
            mem::replace(&mut self.waiting, Vec::new())
            .into_iter()
            .map(|job| {
                let name = job.bind.name.clone();
                (name, job)
            })
            .collect::<HashMap<String, Job>>();

        // put the jobs into the order provided
        let ordered =
            order.into_iter()
            .map(|name| {
                let job = job_map.remove(&name).unwrap();

                // set dep counts
                let name = job.bind.name.clone();

                let count = self.graph.dependency_count(&name);

                *self.dependencies.entry(name).or_insert(0) += count;

                return job;
            })
            .collect::<Vec<Job>>();

        mem::replace(&mut self.waiting, ordered);

        assert!(job_map.is_empty(), "not all jobs were sorted!");
    }

    pub fn build(&mut self) -> ::Result<()> {
        use util::handle::bind::InputPaths;

        if self.waiting.is_empty() {
            println!("there is nothing to do");
            return Ok(());
        }

        for job in &mut self.waiting {
            job.bind.extensions.write().unwrap().insert::<InputPaths>(self.paths.clone());
        }

        let order = try!(self.graph.resolve_all());

        self.sort_jobs(order);

        self.enqueue_ready();

        // TODO
        // remember one key thing:
        // the 'queue' can be empty temporarily when say the last
        // job on the queue finishes, but then its completion
        // will most likely place more jobs on the queue

        // TODO
        // perhaps instead of using a queue, this is where Streams
        // would be appropriate?

        let pool: ThreadPool<Box<TaskBox>> = ThreadPool::fixed_size(2);

        // TODO
        // try to get rid of the global dep count updating logic,
        // try to process all binds sequentially yet parallel

        // TODO
        // it seems pretty stupid to wait on a single defer at a time
        // preferably they'll all be being processed and we'd respond
        // as soon as they're finished, hopefully stream faciliates this

        while let Some(future) = self.futures.pop_front() {
            match defer(pool.clone(), future).await() {
                Ok(bind) => self.handle_done(bind),
                Err(e) => return Err(From::from(format!("a job panicked. stopping everything:\n{}", e))),
            }
        }

        // TODO
        // no longer necessary post-partial update purge?
        self.reset();

        Ok(())
    }

    // TODO: audit
    fn reset(&mut self) {
        self.graph = Graph::new();
        self.waiting.clear();
    }

    fn handle_done(&mut self, current: Bind) {
        let bind_name = current.name.clone();

        // if they're done, move from staging to finished
        self.finished.insert(bind_name.clone(), Arc::new({
            current
        }));

        self.satisfy(&bind_name);
        self.enqueue_ready();
    }

    fn enqueue_ready(&mut self) {
        for mut job in self.ready() {
            let name = job.bind.name.clone();

            if let Some(deps) = self.graph.dependencies_of(&name) {
                // insert each dependency
                for dep in deps {
                    // mutation of the bind dependencies is what necessitates
                    // Job using a bind::Data and only building the
                    // actual Bind on-the-fly, instead of only dealing with
                    // a Bind
                    job.bind.dependencies.insert(dep.clone(), self.finished[dep].clone());
                }
            }

            // TODO
            // this should push onto a queue of futures?
            self.futures.push_back(Future::<Bind, ::Error>::lazy(move || {
                // TODO
                // just use map_err
                match job.process() {
                    Ok(bind) => Ok(bind),
                    Err(e) => {
                        println!("{}", e);
                        Err(e)
                    }
                }
            }));
        }
    }
}
