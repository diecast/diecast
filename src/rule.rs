use std::sync::Arc;
use std::collections::HashSet;
use std::convert::Into;

use binding::Bind;
use source::Source;
use util;
use handle::Handle;

pub enum Kind {
    Read,
    Create,
}

/// Represents a rule that the static site generator must follow.
///
/// A rule consists of a name and handler, as well as any dependencies
/// it may have.
pub struct Rule {
    name: String,
    source: Arc<Box<Source + Sync + Send>>,
    handler: Arc<Box<Handle<Bind> + Sync + Send>>,
    dependencies: HashSet<String>,
    kind: Kind,
}

impl Rule {
    /// Requires the name of the rule.
    ///
    /// The parameter type can be an `&str` or `String`.
    pub fn read<S>(name: S) -> Rule
    where S: Into<String> {
        Rule {
            name: name.into(),
            source: Arc::new(Box::new(util::source::none)),
            handler: Arc::new(Box::new(util::handle::binding::stub)),
            dependencies: HashSet::new(),
            kind: Kind::Read,
        }
    }

    pub fn create<S>(name: S) -> Rule
    where S: Into<String> {
        Rule {
            name: name.into(),
            source: Arc::new(Box::new(util::source::none)),
            handler: Arc::new(Box::new(util::handle::binding::stub)),
            dependencies: HashSet::new(),
            kind: Kind::Create,
        }
    }

    pub fn source<S>(mut self, source: S) -> Rule
    where S: Source + Sync + Send + 'static {
        self.source = Arc::new(Box::new(source));
        self
    }

    /// Associate a handler with this rule.
    pub fn handler<H>(mut self, handler: H) -> Rule
    where H: Handle<Bind> + Sync + Send + 'static {
        self.handler = Arc::new(Box::new(handler));
        self
    }

    // TODO: don't return &Arc, just return Arc.clone()
    pub fn get_source(&self) -> &Arc<Box<Source + Sync + Send + 'static>> {
        &self.source
    }

    /// Access the handler.
    pub fn get_handler(&self) -> &Arc<Box<Handle<Bind> + Sync + Send + 'static>> {
        &self.handler
    }

    /// Register a dependency for this rule.
    pub fn depends_on<D>(mut self, dependency: D) -> Rule
    where D: Into<String> {
        self.dependencies.insert(dependency.into());

        return self;
    }

    // accessors
    pub fn kind(&self) -> &Kind {
        &self.kind
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn dependencies(&self) -> &HashSet<String> {
        &self.dependencies
    }
}

impl<'a> Into<String> for &'a Rule {
    fn into(self) -> String {
        self.name.clone()
    }
}

