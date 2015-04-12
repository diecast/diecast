use std::sync::Arc;
use std::collections::HashSet;
use std::borrow::Cow;
use std::convert::Into;

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
            handler: Arc::new(Box::new(util::handler::binding::stub)),
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
    pub fn depends_on<D>(mut self, dependency: D) -> Rule
    where D: Into<String> {
        self.dependencies.insert(dependency.into());

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

impl<'a> Into<String> for &'a Rule {
    fn into(self) -> String {
        self.name.clone()
    }
}

