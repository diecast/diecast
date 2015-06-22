Work in Progress

Diecast is a parallel, modular, and middleware-oriented static site generator infrastructure for Rust which enables the creation of custom static site generators.

Markdown processing is enabled through the use of [idiomatic Hoedown bindings](https://github.com/blaenk/hoedown) that I wrote.

Documentation and examples are forthcoming, but here's a taste of what it's like:

A rule that matches static assets and simply copies them to the output directory:

``` rust
let statics =
    Rule::matching("statics", or!(
        "images/**/*".parse::<Glob>().unwrap(),
        "static/**/*".parse::<Glob>().unwrap(),
        "js/**/*".parse::<Glob>().unwrap(),
        "favicon.png",
        "CNAME"
    ))
    .handler(binding::each(Chain::new()
        .link(route::identity)
        .link(item::copy)))
    .build();
```

Define a rule called `"posts"` which will match any file in the input directory that matches the glob pattern `posts/*.md`:

``` rust
let posts =
    Rule::matching("posts", "posts/*.markdown".parse::<Glob>().unwrap())
    .depends_on(&templates)
    .handler(Chain::new()
        .link(binding::each(Chain::new()
            .link(item::read)
            .link(toml::parse)
            .link(item::date)))
        .link(binding::retain(item::publishable))
        .link(binding::each(Chain::new()
            .link(item::markdown)
            .link(route::pretty)
            .link(hbs::render(&templates, "post", post_template))
            .link(hbs::render(&templates, "layout", layout_template))
            .link(item::write)))
        .link(binding::sort_by(|a, b| {
            let a = a.extensions.get::<item::Date>().unwrap();
            let b = b.extensions.get::<item::Date>().unwrap();
            b.cmp(a)
        })))
    .build();
```

Define a rule called `"post index"` which will create a paginated index of the posts:

``` rust
let posts_index =
    Rule::creating("post index")
    .depends_on(&posts)
    .depends_on(&templates)
    .handler(Chain::new()
        .link(bind::create("index.html"))
        .link(bind::each(Chain::new()
          .link(handlebars::render(&templates, "index", render_index))
          .link(handlebars::render(&templates, "layout", layout_template))
          .link(item::write))))
    .build();
```

A custom handler which would render the post index:

``` rust
fn render_index(item: &mut Item) -> diecast::Result {
  // notice "post index" depends on "posts",
  // so it has access to the "posts" dependency within its handlers

  for post in item.bind().dependencies["posts"].iter() {
    // do something for each post
  }
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

