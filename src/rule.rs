use std::sync::Arc;
use std::collections::HashSet;
use std::borrow::Cow;

use binding::Bind;
use util;
use handle::Handle;

/// Represents a rule that the static site generator must follow.
///
/// A rule consists of a name and handler, as well as any dependencies
/// it may have.
pub struct Rule {
    name: String,
    handler: Arc<Box<Handle<Bind> + Sync + Send>>,
    dependencies: HashSet<String>,
}

impl Rule {
    /// Requires the name of the rule.
    ///
    /// The parameter type can be an `&str` or `String`.
    pub fn new<'a, S: Into<Cow<'a, str>>>(name: S) -> Rule {
        Rule {
            name: name.into().into_owned(),
            handler: Arc::new(Box::new(util::handlers::binding::stub)),
            dependencies: HashSet::new(),
        }
    }

    /// Associate a handler with this rule.
    pub fn handler<H>(mut self, handler: H) -> Rule
    where H: Handle<Bind> + Sync + Send + 'static {
        self.handler = Arc::new(Box::new(handler));
        self
    }

    /// Access the handler.
    pub fn get_handler(&self) -> &Arc<Box<Handle<Bind> + Sync + Send + 'static>> {
        &self.handler
    }

    /// Register a dependency for this rule.
    pub fn depends_on<R: ?Sized>(mut self, dependency: &R) -> Rule
    where R: Register {
        dependency.register(&mut self.dependencies);

        return self;
    }

    // accessors
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn dependencies(&self) -> &HashSet<String> {
        &self.dependencies
    }
}

pub trait Register {
    fn register(&self, dependencies: &mut HashSet<String>);
}

impl Register for Rule {
    fn register(&self, dependencies: &mut HashSet<String>) {
        dependencies.insert(self.name.clone());
    }
}

// TODO: this has potential for adding string many times despite being the same
// each having diff ref-count
impl Register for str {
    fn register(&self, dependencies: &mut HashSet<String>) {
        dependencies.insert(self.to_string());
    }
}

impl<'a, I> Register for &'a [I] where I: Register {
    fn register(&self, dependencies: &mut HashSet<String>) {
        for i in *self {
            i.register(dependencies);
        }
    }
}

