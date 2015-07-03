Work in Progress

Diecast is a parallel, modular, and middleware-oriented static site generator infrastructure for Rust which enables the creation of custom static site generators.

Markdown processing is enabled through the use of [idiomatic Hoedown bindings](https://github.com/blaenk/hoedown) that I wrote.

Documentation and examples are forthcoming, but here's a taste of what it's like. For a full working example see [my setup](https://github.com/blaenk/site).

Here's a rule that matches static assets and simply copies them to the output directory:

``` rust
let statics =
    Rule::named("statics")
    .pattern(or!(
        glob!("images/**/*"),
        glob!("images/**/*"),
        glob!("static/**/*"),
        glob!("js/**/*"),
        "favicon.png",
        "CNAME"
    ))
    .handler(bind::each(chain![route::identity, item::copy]))
    .build();
```

Here's a rule called `"posts"` which will match any file in the input directory that matches the glob pattern `posts/*.md`. The rule then does the following:

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

Each of the above are just types that implement the `Handle` trait. `chain!` is a helper macro for the `Chain` handler that simply chains multiple handlers together in a sequence. Common combinations of handlers could be condensed into a single handler for ease of use.

Notice that it depends on the templates rule, which guarantees that it will be processed only after the templates have been processed.

``` rust
let posts =
    Rule::named("posts")
    .depends_on(&templates)
    .pattern(glob!("posts/*.markdown"))
    .handler(chain![
        bind::each(chain![item::read, metadata::toml::parse]),
        bind::retain(helpers::publishable),
        bind::each(chain![
            helpers::set_date,
            markdown::markdown(),
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

Here's a `"post index"` rule which will create an index of the posts:

``` rust
let posts_index =
    Rule::named("post index")
    .depends_on(&posts)
    .depends_on(&templates)
    .handler(chain![
        bind::create("index.html"),
        bind::each(chain![
            handlebars::render(&templates, "index", view::posts_index_template),
            handlebars::render(&templates, "layout", view::layout_template),
            item::write])])
    .build();
```

A custom handler which would render the post index:

``` rust
fn render_index(item: &mut Item) -> diecast::Result<()> {
  // notice "post index" depends on "posts",
  // so it has access to the "posts" dependency within its handlers
  // useful for enumerating the posts in the index we're creating

  for post in item.bind().dependencies["posts"].iter() {
    // do something for each post
  }
  
  Ok(())
}
```

Define a base configuration, register the rules, and run the user-provided command:

``` rust
let command =
    command::Builder::new()
    .rules(vec![statics, posts, index])
    .build();

cmd.run();
```

