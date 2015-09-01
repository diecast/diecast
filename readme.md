Work in Progress

Diecast is a parallel, modular, and middleware-oriented static site generator infrastructure for Rust which enables the creation of custom static site generators.

Documentation and examples are forthcoming, but here's a taste of what it's like. For a full working example see [my setup](https://github.com/blaenk/site).

## Primitives

The set of core Diecast primitives starts with `Rule`, which is used to associated behavior with particular input files. Each `Rule` is essentially _bound_ to a set of matching files. This set is represented using the `Bind` type, which consists of one `Item` for every file in the input directory that is matched by the `Rule` in some way, e.g. a glob pattern. An `Item` is a representation of an input file and/or its artifacts.

A `Rule` defines how its corresponding `Bind` should be processed by associating it with a handler, which is any type that implements the `Handle` trait. There can be `Bind`-level and `Item`-level handlers, corresponding to traits `Handle<Bind>` and `Handle<Item>` respectively, meaning that they operate either on an entire `Bind` at a time or a single `Item` at a time.

## Example

Here's a rule that matches static assets and simply copies them to the output directory:

``` rust
let statics =
    Rule::named("statics")
    .handler(chain![
        bind::select(or!(
            glob!("images/**/*"),
            glob!("images/**/*"),
            glob!("static/**/*"),
            glob!("js/**/*"),
            "favicon.png",
            "CNAME")),
        bind::each(chain![
            route::identity,
            item::copy])])
    .build();
```

The example above defines a rule called `"statics"` and associates with a `Bind`-level handler. This handler is called `Chain` and it's simply a handler that chains together multiple handlers into one. Note that `chain!` is simply a helper macro for defining handlers of type `Chain`. In this case, it chains together the `bind::select` and `bind::each` handlers, both of which are `Bind`-level handlers.

`bind::select` is a handler that takes a path pattern and creates an `Item` in the `Bind` for each matching file in the input directory. This handler is useful because it helps to populate a `Bind` with `Item`s.

`bind::each` is a handler that takes an `Item`-level handler and applies it to each `Item` in the `Bind`. In this case, the handler being applied to each `Item` is itself a chain of `Item`-level handlers: `route::identity` and `item::copy`. The first simply routes the input path to an output path, while the second performs a file-system copy of the file.

In plain English, the above rule essentially means:

1. define a rule named "statics"
2. find all files in the input directory matching the specified pattern and add a corresponding `Item` for each one into this rule's `Bind`
3. for each `Item` in the `Bind`:
   1. route the file from the input directory directly to the output directory. e.g `input/a/b/c.txt` would route to `output/a/b/c.txt`
   2. copy the file represented by the `Item` from the input directory to the output directory

### In Depth

The example below defines a rule called `"posts"` which will match any file in the input directory that matches the glob pattern `posts/*.md`. The rule then does the following:

1. reads each match
2. parses its metadata
3. prunes away drafts
4. parses the date
5. renders the markdown
6. saves a version of the content under the name "rendered" for future use (e.g. in an RSS feed)
7. routes the output file
8. renders the post template
9. renders that into the site layout
10. writes the result to the target file
11. sorts each post by date (useful for things like the post index that follows)

Notice that it depends on the templates rule, which guarantees that it will be processed only after the templates have been processed.

``` rust
let posts =
    Rule::named("posts")
    .depends_on(&templates)
    .handler(chain![
        bind::select(glob!("posts/*.markdown"))
        bind::each(chain![item::read, metadata::toml::parse]),
        bind::retain(helpers::publishable),
        bind::each(chain![
            helpers::set_date,
            markdown::markdown,
            versions::save("rendered"),
            route::pretty,
            handlebars::render(&templates, "post", view::post_template),
            handlebars::render(&templates, "layout", view::layout_template),
            item::write]),
        bind::sort_by(|a, b| {
            let a = a.extensions.get::<PublishDate>().unwrap();
            let b = b.extensions.get::<PublishDate>().unwrap();
            b.cmp(a)
        })
    ])
    .build();
```

Here's a `"post index"` rule which will create an index of the posts. Note that we're no longer using `bind::select`. Here we're using `bind::create` instead, which creates an `Item` in the bind that represents the creation of a file without reading from one first.

``` rust
let posts_index =
    Rule::named("post index")
    .depends_on(&posts)
    .depends_on(&templates)
    .handler(chain![
        bind::create("index.html"),
        bind::each(chain![
            handlebars::render(&templates, "index", render_index),
            handlebars::render(&templates, "layout", view::layout_template),
            item::write])])
    .build();
```

The `render_index` function above could look something like this:

``` rust
fn render_index(item: &mut Item) -> diecast::Result<()> {
  // notice "post index" depends on "posts",
  // so it has access to the "posts" dependency within its handlers.
  // useful for enumerating the posts in the index we're creating

  for post in item.bind().dependencies["posts"].iter() {
    // do something for each post
  }

  Ok(())
}
```

This can then be wired up to the Diecast command-line interface:

``` rust
let mut site = Site::new(vec![statics, posts, index]);

// selects appropriate command based on
// process arguments. can also attach new commands
let command = command::Builder::new().build();

cmd.run(&mut site);
```

## Middleware

Thanks to its extensible middleware nature, there are already a couple of packages that extend Diecast:

### Previewing

* [live](https://github.com/diecast/live): watches input directory for file changes and rebuilds site accordingly
* [websocket](https://github.com/diecast/websocket): item updating for previews via websockets

### Templating

* [handlebars](https://github.com/diecast/handlebars): handlebars templating
* [liquid](https://github.com/diecast/liquid): liquid templating

### Document Processing

* [hoedown](https://github.com/diecast/hoedown): markdown processing via the [hoedown](https://github.com/hoedown/hoedown) C library
* [commonmark](https://github.com/diecast/commonmark): markdown processing via the [pulldown-cmark](https://github.com/google/pulldown-cmark) Rust library
* [metadata](https://github.com/diecast/metadata): document frontmatter/metadata parsing (TOML, JSON, YAML)
* [tags](https://github.com/diecast/tags): tag collections
* [scss](https://github.com/diecast/scss): scss compilation
* [feed](https://feedhub.com/diecast/feed): feed generation (RSS and Atom)

### Miscellaneous

* [git](https://github.com/diecast/git): git information for items, e.g. last commit SHA and message that affected the given item
* [versions](https://github.com/diecast/versions): saving and loading different versions of items. e.g. a feed-friendly version, before other processors are applied
* [adjacent](https://github.com/diecast/adjacent): next/previous article references
