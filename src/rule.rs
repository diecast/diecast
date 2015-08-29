use std::sync::Arc;
use std::collections::HashSet;
use std::convert::Into;

use bind::Bind;
use util;
use handler::Handle;

#[must_use]
pub struct Builder {
    name: String,
    handler: Arc<Handle<Bind> + Sync + Send>,
    dependencies: HashSet<String>,
}

impl Builder {
    fn new(name: String) -> Builder {
        Builder {
            name: name,
            handler: Arc::new(util::handle::bind::missing),
            dependencies: HashSet::new(),
        }
    }

    /// Associate a handler with this rule.
    pub fn handler<H>(mut self, handler: H) -> Builder
    where H: Handle<Bind> + Sync + Send + 'static {
        self.handler = Arc::new(handler);
        self
    }

    /// Register a dependency for this rule.
    pub fn depends_on<D>(mut self, dependency: D) -> Builder
    where D: Into<String> {
        self.dependencies.insert(dependency.into());
        self
    }

    pub fn build(self) -> Rule {
        Rule {
            name: self.name,
            handler: self.handler,
            dependencies: self.dependencies,
        }
    }
}

/// Represents a rule that the static site generator must follow.
///
/// A rule consists of a name and handler, as well as any dependencies
/// it may have.
pub struct Rule {
    name: String,
    handler: Arc<Handle<Bind> + Sync + Send>,
    dependencies: HashSet<String>,
}

impl Rule {
    pub fn named<N>(name: N) -> Builder
    where N: Into<String> {
        Builder::new(name.into())
    }

    pub fn handler(&self) -> Arc<Handle<Bind> + Sync + Send> {
        self.handler.clone()
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
