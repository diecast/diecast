Work in Progress

Diecast is a parallel, modular, and middleware-oriented static site generator infrastructure for Rust which enables the creation of custom static site generators.

Markdown processing is enabled through the use of [idiomatic Hoedown bindings](https://github.com/blaenk/hoedown) that I wrote.

Documentation and examples are forthcoming, but here's a taste of what it's like:

Define a rule called `"posts"` which will match any file in the input directory that matches the glob pattern `posts/*.md`:

``` rust
let posts =
  Rule::matching("posts", glob::Pattern::new("posts/*.md").unwrap())
  // specify the compiler to use to process each matched item
  // ItemChain is itself a compiler that allows chaining multiple compilers
  .compiler(
    ItemChain::new()
      // read the contents of the file
      .link(compiler::read)
      // parse the metadata front-matter
      .link(compiler::parse_metadata)
      // demonstrate a custom compiler using a closure
      .link(|item: &mut Item| -> compiler::Result {
        println!("inside a custom compiler!");
        Ok(())
      })
      // filter out the drafts if not in preview-mode
      .link(compiler::retain(publishable))
      // render the markdown
      .link(compiler::render_markdown)
      // specify the output file name, e.g. .md -> .html
      .link(router::set_extension("html"))
      // write the contents to the output file
      .link(compiler::write));
```

A custom compiler which would render the post index:

``` rust
fn render_index(item: &mut Item) -> compiler::Result {
  // notice since "post index" depends on "posts",
  // it has access to the "posts" dependency within its compilers
  let posts = item.bind().dependencies["posts"].items;

  // use the posts to render the index
}
```

Define a rule called `"post index"` which will create an `"index.html"` file:

``` rust
let index =
  Rule::creating("post index", "index.html")
  .compiler(
    ItemChain::new()
      .link(render_index)
      .link(compiler::write))
  // it will depend on the "posts" rule so that it:
  //   1. is evaluated _only_ after the "posts" rule has been evaluated
  //   2. has access to the "posts" dependency
  .depends_on(&posts);
```

Define a base configuration and allow it to be overridden from the command line:

``` rust
let config =
  Configuration::new("input/", "output/")
  // ignore common editor files
  .ignore(regex!(r"^\.|^#|~$|\.swp$"));

let mut command = command::from_args(config);

// register the rules with the site
command.site().bind(posts);
command.site().bind(index);

// run the command
command.run();
```

