Work in Progress

Diecast is a parallel, modular, and middleware-oriented static site generator infrastructure for Rust which enables the creation of custom static site generators.

Markdown processing is enabled through the use of [idiomatic Hoedown bindings](https://github.com/blaenk/hoedown) that I wrote.

Documentation and examples are forthcoming, but here's a taste of what it's like:

Define a rule called `"posts"` which will match any file in the input directory that matches the glob pattern `posts/*.md`:

``` rust
let posts =
    Rule::read("posts")
    .depends_on(&templates)
    // collect the posts
    .source(source::select("posts/*.markdown".parse::<Glob>().unwrap()))
    // process each post
    .handler(Chain::new()
        // process this chain for each item in parallel
        .link(binding::parallel_each(Chain::new()
            .link(item::read)
            .link(item::parse_metadata)
            // parse date from metadata
            .link(item::date)))
        // only retain publishable posts, e.g. non-drafts
        .link(binding::retain(item::publishable))
        .link(binding::parallel_each(Chain::new()
            // render markdown
            .link(item::markdown)
            // route to target destination
            .link(route::pretty)
            // render post template and layout
            .link(hbs::render_template(&templates, "post", post_template))
            .link(hbs::render_template(&templates, "layout", layout_template))
            // write to the target file
            .link(item::write)))
        // sort posts by date for future rules, such as post index
        .link(binding::sort_by(|a, b| {
            let a = a.extensions.get::<item::Date>().unwrap();
            let b = b.extensions.get::<item::Date>().unwrap();
            b.cmp(a)
        })));
```

A custom compiler which would render the post index:

``` rust
fn render_index(item: &mut Item) -> compiler::Result {
  // notice "post index" depends on "posts",
  // so it has access to the "posts" dependency within its compilers

  for post in item.bind().dependencies["posts"].iter() {
    // do something for each post
  }
}
```

Define a rule called `"post index"` which will create a paginated index of the posts:

``` rust
let posts_index =
    Rule::create("post index")
    // this ensures that the post index is only run after
    // the posts and templates rules are finished
    .depends_on(&posts)
    .depends_on(&templates)
    // paginate the posts rule with 5 posts per page
    .source(source::paginate(&posts, 5, |page: usize| -> PathBuf {
        if page == 0 {
            PathBuf::from("index.html")
        } else {
            PathBuf::from(&format!("{}/index.html", page))
        }
    }))
    .handler(binding::parallel_each(Chain::new()
        .link(hbs::render_template(&templates, "index", render_index))
        .link(hbs::render_template(&templates, "layout", layout_template))
        .link(item::write)));
```

Define a base configuration, register the rules, and run the user-provided command:

``` rust
let config =
  Configuration::new("input/", "output/")
  // ignore common editor files
  .ignore(regex!(r"^\.|^#|~$|\.swp$"));

// possibly overrides above configuration based on user input
let mut command = command::from_args(config);

// register the rules with the site
command.site().register(posts);
command.site().register(index);

// run the command supplied by the user
command.run();
```

