use std::path::PathBuf;
use std::sync::Arc;

use git2;
use typemap;

use diecast::{self, Handle, Item, Bind};

#[derive(Clone)]
pub struct Git {
    pub sha: git2::Oid,
    pub message: String,
}

impl typemap::Key for Git {
    type Value = Arc<Git>;
}

pub fn git(bind: &mut Bind) -> diecast::Result {
    use std::collections::HashMap;
    use git2::{
        Repository,
        Pathspec,
        Commit,
        DiffOptions,
        Error,
        Diff,
        Oid,
    };

    fn match_with_parent(repo: &Repository, commit: &Commit, parent: &Commit,
                         opts: &mut DiffOptions) -> Result<bool, Error> {
        let a = try!(parent.tree());
        let b = try!(commit.tree());
        let diff = try!(Diff::tree_to_tree(repo, Some(&a), Some(&b), Some(opts)));
        Ok(diff.deltas().len() > 0)
    }

    let repo = Repository::open(".").unwrap();

    let mut cache: HashMap<Oid, Arc<Git>> = HashMap::new();
    let mut input: HashMap<PathBuf, (&mut Item, DiffOptions, Pathspec)> = HashMap::new();

    for item in bind {
        let path = item.source().unwrap();

        let mut diffopts = DiffOptions::new();
        diffopts.include_ignored(false);
        diffopts.recurse_ignored_dirs(false);
        diffopts.include_untracked(false);
        diffopts.recurse_untracked_dirs(false);
        diffopts.include_unmodified(false);
        diffopts.ignore_filemode(true);
        diffopts.ignore_submodules(true);
        diffopts.disable_pathspec_match(true);
        diffopts.skip_binary_check(true);
        diffopts.enable_fast_untracked_dirs(true);
        diffopts.include_unreadable(false);
        diffopts.force_text(true);

        diffopts.pathspec(path.to_str().unwrap());

        let pathspec = Pathspec::new(Some(path.to_str().unwrap()).into_iter()).unwrap();

        input.insert(path, (item, diffopts, pathspec));
    }

    let mut prune = vec![];

    let mut revwalk = repo.revwalk().unwrap();
    revwalk.push_head().unwrap();

    macro_rules! filter_try {
        ($e:expr) => (match $e { Ok(t) => t, Err(e) => continue })
    }

    for id in revwalk {
        let commit = filter_try!(repo.find_commit(id));
        let parents = commit.parents().len();

        // TODO: no merge commits?
        if parents > 1 { continue }

        for (path, &mut (ref mut item, ref mut diffopts, ref mut pathspec)) in &mut input {
            match commit.parents().len() {
                0 => {
                    let tree = filter_try!(commit.tree());
                    let flags = git2::PATHSPEC_NO_MATCH_ERROR;
                    if pathspec.match_tree(&tree, flags).is_err() { continue }
                },
                _ => {
                    let m = commit.parents().all(|parent| {
                        match_with_parent(&repo, &commit, &parent, diffopts)
                            .unwrap_or(false)
                    });

                    if !m { continue }
                },
            }

            let git =
                cache.entry(commit.id())
                .or_insert_with(|| {
                    let message = String::from_utf8_lossy(commit.message_bytes()).into_owned();
                    Arc::new(Git { sha: commit.id(), message: message })
                })
                .clone();

            item.extensions.insert::<Git>(git);
            prune.push(path.clone());
        }

        for path in prune.drain(..) {
            input.remove(&path).unwrap();
        }
    }

    Ok(())
}

