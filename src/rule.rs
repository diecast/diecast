use std::sync::Arc;
use std::collections::HashSet;
use std::convert::Into;

use bind::Bind;
use util;
use handle::Handle;
use pattern::Pattern;

pub enum Kind {
    Matching(Box<Pattern + Sync + Send + 'static>),
    Creating,
}

#[must_use]
pub struct RuleBuilder {
    name: String,
    handler: Arc<Box<Handle<Bind> + Sync + Send>>,
    dependencies: HashSet<String>,
    kind: Kind,
}

impl RuleBuilder {
    fn new(name: String, kind: Kind) -> RuleBuilder {
        RuleBuilder {
            name: name,
            handler: Arc::new(Box::new(util::handle::bind::missing)),
            kind: kind,
            dependencies: HashSet::new(),
        }
    }

    /// Associate a handler with this rule.
    pub fn handler<H>(mut self, handler: H) -> RuleBuilder
    where H: Handle<Bind> + Sync + Send + 'static {
        self.handler = Arc::new(Box::new(handler));
        self
    }

    /// Register a dependency for this rule.
    pub fn depends_on<D>(mut self, dependency: D) -> RuleBuilder
    where D: Into<String> {
        self.dependencies.insert(dependency.into());
        self
    }

    pub fn build(self) -> Rule {
        Rule {
            name: self.name,
            handler: self.handler,
            dependencies: self.dependencies,
            kind: Arc::new(self.kind),
        }
    }
}

/// Represents a rule that the static site generator must follow.
///
/// A rule consists of a name and handler, as well as any dependencies
/// it may have.
pub struct Rule {
    name: String,
    handler: Arc<Box<Handle<Bind> + Sync + Send>>,
    dependencies: HashSet<String>,
    kind: Arc<Kind>,
}

impl Rule {
    pub fn matching<S, P>(name: S, pattern: P) -> RuleBuilder
    where S: Into<String>, P: Pattern + Sync + Send + 'static {
        RuleBuilder::new(name.into(), Kind::Matching(Box::new(pattern)))
    }

    pub fn creating<S>(name: S) -> RuleBuilder
    where S: Into<String> {
        RuleBuilder::new(name.into(), Kind::Creating)
    }

    // TODO: why &Arc? make it &T or just Arc
    // accessors
    pub fn handler(&self) -> &Arc<Box<Handle<Bind> + Sync + Send + 'static>> {
        &self.handler
    }

    pub fn kind(&self) -> Arc<Kind> {
        self.kind.clone()
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

