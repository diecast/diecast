//! Site generation.

use pattern::Pattern;
use compile::Compile;
use item::Item;
use dependency::Graph;

/// A generator scans the input path to find
/// files that match the given pattern. It then
/// takes each of those files and passes it through
/// the compiler chain.
pub struct Generator {
  input: Path,
  output: Path,

  bindings: Vec<Binding<Box<Pattern + Send + Sync>,
                        Box<Compile + Send + Sync>>>,
}

impl Generator {
  pub fn new(input: Path, output: Path) -> Generator {
    Generator {
      input: input,
      output: output,
      bindings: vec![]
    }
  }
}

impl Generator {
  pub fn generate(&mut self) {
    use std::io::fs::PathExtensions;
    use std::io::fs;

    let mut items =
      fs::walk_dir(&self.input).unwrap()
        .filter_map(|p| {
          if p.is_file() {
            Some(Item::new(p))
          } else {
            None
          }
        });

    for binding in self.bindings.iter() {
      for mut item in items {
        let relative = item.path.path_relative_from(&self.input).unwrap();

        if binding.pattern.matches(&relative) {
          binding.compiler.compile(&mut item);
        }
      }
    }
  }

  pub fn bind<P, C>(mut self, pattern: P, compiler: C) -> Generator
    where P: Pattern + Send + Sync, C: Compile + Send + Sync {
      self.bindings.push(
        Binding {
          pattern: box pattern as Box<Pattern + Send + Sync>,
          compiler: box compiler as Box<Compile + Send + Sync>
        });
      self
  }
}

struct Binding<P, C>
  where P: Pattern, C: Compile {
  pattern: P,
  compiler: C,
}

