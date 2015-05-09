use std::sync::Arc;
use std::path::PathBuf;
use std::ops::Range;
use std::any::Any;

use regex::Regex;
use toml;
use chrono;
use hoedown::{self, Render};
use hoedown::renderer::html;
use typemap;

use handle::{self, Handle, Result};
use item::Item;

use super::{Chain, Extender};

impl Handle<Item> for Chain<Item> {
    fn handle(&self, item: &mut Item) -> Result {
        for handler in &self.handlers {
            try!(handler.handle(item));
        }

        Ok(())
    }
}

impl<T> Handle<Item> for Extender<T>
where T: typemap::Key, T::Value: Any + Sync + Send + Clone {
    fn handle(&self, item: &mut Item) -> handle::Result {
        item.extensions.insert::<T>(self.payload.clone());
        Ok(())
    }
}

pub fn copy(item: &mut Item) -> handle::Result {
    use std::fs;

    if let Some(from) = item.source() {
        if let Some(to) = item.target() {
            // TODO: once path normalization is in, make sure
            // writing to output folder

            if let Some(parent) = to.parent() {
                // TODO: this errors out if the path already exists? dumb
                ::mkdir_p(parent).unwrap();
            }

            try!(fs::copy(from, to));
        }
    }

    Ok(())
}

/// Handle<Item> that reads the `Item`'s body.
pub fn read(item: &mut Item) -> handle::Result {
    use std::fs::File;
    use std::io::Read;

    if let Some(from) = item.source() {
        let mut buf = String::new();

        // TODO: use try!
        File::open(from)
            .unwrap()
            .read_to_string(&mut buf)
            .unwrap();

        item.body = buf;
    }

    Ok(())
}

/// Handle<Item> that writes the `Item`'s body.
pub fn write(item: &mut Item) -> handle::Result {
    use std::fs::File;
    use std::io::Write;

    if let Some(to) = item.target() {
        // TODO: once path normalization is in, make sure
        // writing to output folder
        if let Some(parent) = to.parent() {
            // TODO: this errors out if the path already exists? dumb
            ::mkdir_p(parent).unwrap();
        }

        trace!("writing file {:?}", to);

        // TODO: this sometimes crashes
        File::create(&to)
            .unwrap()
            .write_all(item.body.as_bytes())
            .unwrap();
    }

    Ok(())
}

/// Handle<Item> that prints the `Item`'s body.
pub fn print(item: &mut Item) -> handle::Result {
    println!("{}", item.body);

    Ok(())
}

pub struct Metadata;

impl typemap::Key for Metadata {
    type Value = toml::Value;
}

pub fn parse_metadata(item: &mut Item) -> handle::Result {
    // TODO:
    // should probably allow arbitrary amount of
    // newlines after metadata block?
    let re =
        Regex::new(
            concat!(
                "(?ms)",
                r"\A---\s*\n",
                r"(?P<metadata>.*?\n?)",
                r"^---\s*$",
                r"\n*",
                r"(?P<body>.*)"))
            .unwrap();

    let body = if let Some(captures) = re.captures(&item.body) {
        if let Some(metadata) = captures.name("metadata") {
            if let Ok(parsed) = metadata.parse() {
                item.extensions.insert::<Metadata>(parsed);
            }
        }

        captures.name("body").map(String::from)
    } else { None };

    if let Some(body) = body {
        item.body = body;
    }

    Ok(())
}

pub fn is_draft(item: &Item) -> bool {
    item.extensions.get::<Metadata>()
        .map(|meta| {
            meta.lookup("draft")
                .and_then(::toml::Value::as_bool)
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

pub fn publishable(item: &Item) -> bool {
    !(is_draft(item) && !item.bind().configuration.is_preview)
}

// TODO: should this just contain the items itself instead of the range?
#[derive(Clone)]
pub struct Page {
    pub first: (usize, Arc<PathBuf>),
    pub next: Option<(usize, Arc<PathBuf>)>,
    pub curr: (usize, Arc<PathBuf>),
    pub prev: Option<(usize, Arc<PathBuf>)>,
    pub last: (usize, Arc<PathBuf>),

    pub range: Range<usize>,

    pub page_count: usize,
    pub post_count: usize,
    pub posts_per_page: usize,
}

impl typemap::Key for Page {
    type Value = Page;
}

pub struct Date;

impl typemap::Key for Date {
    type Value = chrono::NaiveDate;
}

// TODO
// * make time type generic
// * customizable format
pub fn date(item: &mut Item) -> handle::Result {
    let date = {
        if let Some(meta) = item.extensions.get::<Metadata>() {
            let date = meta.lookup("published").and_then(toml::Value::as_str).unwrap();

            Some(chrono::NaiveDate::parse_from_str(date, "%B %e, %Y").unwrap())
        } else {
            None
        }
    };

    if let Some(date) = date {
        item.extensions.insert::<Date>(date);
    }

    Ok(())
}

pub fn markdown(item: &mut Item) -> handle::Result {
    use std::collections::HashMap;
    use regex::Captures;

    let pattern = Regex::new(r"(?m)^\*\[(?P<abbr>.+)\]: (?P<full>.+)$").unwrap();
    let mut abbrs = HashMap::new();

    let clean = pattern.replace_all(&item.body, |caps: &Captures| -> String {
        let abbr = String::from(caps.name("abbr").unwrap());
        let full = String::from(caps.name("full").unwrap());

        assert!(
            !abbr.chars().any(|c| c == '|'),
            "abbreviations shouldn't contain the '|' character!");

        abbrs.insert(abbr, full);
        String::new()
    });

    trace!("collected abbreviations");

    let meta = item.extensions.get::<Metadata>();

    if let Some(meta) = meta {
        if !meta.lookup("toc.show").and_then(toml::Value::as_bool).unwrap_or(false) {
            // TODO: tell render not to generate toc
        }
    }

    // if there is metadata, parse the field
    // otherwise assume left align
    let align =
        meta.and_then(|m|
            m.lookup("toc.align")
            .and_then(toml::Value::as_str)
            .map(|align| {
                match align {
                    "left" => renderer::Align::Left,
                    "right" => renderer::Align::Right,
                    _ => panic!("invalid value for toc.align. either `left` or `right`"),
                }
            }))
        .unwrap_or(renderer::Align::Left);

    trace!("got toc alignment");

    let document =
        hoedown::Markdown::new(&clean)
        .extensions({
            use hoedown::*;

            AUTOLINK |
            FENCED_CODE |
            FOOTNOTES |
            MATH |
            MATH_EXPLICIT |
            SPACE_HEADERS |
            STRIKETHROUGH |
            SUPERSCRIPT |
            TABLES
        });

    let mut renderer = self::renderer::Renderer::new(abbrs, align);

    trace!("constructed renderer");

    let buffer = renderer.render(&document);

    trace!("rendered markdown");

    let pattern = Regex::new(r"<p>::toc::</p>").unwrap();

    let mut smartypants = hoedown::Buffer::new(64);
    html::smartypants(&buffer, &mut smartypants);

    trace!("smartypants");

    item.body = pattern.replace(&smartypants.to_str().unwrap(), &renderer.toc[..]);

    trace!("inserted toc");

    Ok(())
}

mod renderer {
    use hoedown::{Buffer, Render, Wrapper, Markdown};
    use hoedown::renderer;
    use std::collections::HashMap;
    use regex::Regex;

    pub enum Align {
        Left,
        Right,
    }

    pub struct Pass;
    impl Render for Pass {
        fn link(&mut self, output: &mut Buffer, content: &Buffer, _link: &Buffer, _title: &Buffer) -> bool {
            output.pipe(content);
            true
        }
    }

    fn sanitize(content: &str) -> String {
        let doc =
            Markdown::new(content)
            .extensions({
                use hoedown::*;

                AUTOLINK |
                FENCED_CODE |
                FOOTNOTES |
                MATH |
                MATH_EXPLICIT |
                SPACE_HEADERS |
                STRIKETHROUGH |
                SUPERSCRIPT |
                TABLES
            });

        let output = String::from(Pass.render_inline(&doc).to_str().unwrap());

        output.chars()
        .filter(|&c|
            c.is_alphabetic() || c.is_digit(10) ||
            c == '_' || c == '-' || c == '.' || c == ' '
        )
        .map(|c| {
            let c = c.to_lowercase().next().unwrap();

            if c.is_whitespace() { '-' }
            else { c }
        })
        .skip_while(|c| !c.is_alphabetic())
        .collect()
    }

    pub struct Renderer {
        pub html: renderer::html::Html,
        abbreviations: HashMap<String, String>,
        matcher: Regex,

        pub toc: String,

        /// the current header level
        toc_level: i32,

        /// the offset of the first header sighted from 0
        toc_offset: i32,

        toc_align: Align,
    }

    impl Renderer {
        pub fn new(abbrs: HashMap<String, String>, align: Align) -> Renderer {
            let joined: String =
                abbrs.keys().cloned().collect::<Vec<String>>().connect("|");

            // TODO: shouldn't have | in abbr
            let matcher = Regex::new(&joined).unwrap();

            Renderer {
                html: renderer::html::Html::new(renderer::html::Flags::empty(), 0),
                abbreviations: abbrs,
                matcher: matcher,

                toc: String::new(),
                toc_level: 0,
                toc_offset: 0,
                toc_align: align,
            }
        }
    }

    #[allow(unused_variables)]
    impl Wrapper for Renderer {
        type Base = renderer::html::Html;

        fn base(&mut self) -> &mut renderer::html::Html {
            &mut self.html
        }

        fn code_block(&mut self, output: &mut Buffer, code: &Buffer, lang: &Buffer) {
            use zmq;
            use std::io::Write;

            let lang = if lang.is_empty() {
                "text"
            } else {
                lang.to_str().unwrap()
            };

            let mut ctx = zmq::Context::new();
            let mut socket = ctx.socket(zmq::REQ).unwrap();
            socket.connect("tcp://127.0.0.1:5555").unwrap();

            write!(output,
r#"<figure class="codeblock">
<pre>
<code class="highlight language-{}">"#, lang).unwrap();

            if lang == "text" {
                output.pipe(code);
            } else {
                let lang = zmq::Message::from_slice(lang.as_bytes()).unwrap();
                socket.send_msg(lang, zmq::SNDMORE).unwrap();

                let code = zmq::Message::from_slice(&code).unwrap();
                socket.send_msg(code, 0).unwrap();

                let highlighted = socket.recv_msg(0).unwrap();

                output.write(&highlighted).unwrap();
            }

            output.write(b"</code></pre></figure>").unwrap();
        }

        fn normal_text(&mut self, output: &mut Buffer, text: &Buffer) {
            use regex::Captures;
            use std::io::Write;

            if self.abbreviations.is_empty() {
                output.pipe(text);
                return;
            }

            // replace abbreviations with their full form
            let replaced = self.matcher.replace_all(text.to_str().unwrap(), |caps: &Captures| -> String {
                let abbr = caps.at(0).unwrap();
                let full = self.abbreviations.get(abbr).unwrap().clone();
                trace!("replacing {:?} with {:?}", abbr, full);

                format!(r#"<abbr title="{}">{}</abbr>"#, full, abbr)
            });

            output.write(replaced.as_bytes()).unwrap();
        }

        fn after_render(&mut self, output: &mut Buffer, inline_render: bool) {
            if inline_render {
                return;
            }

            while self.toc_level > 0 {
                self.toc.push_str("</li>\n</ol>\n");
                self.toc_level -= 1;
            }

            self.toc.push_str("</nav>");
        }

        fn header(&mut self, output: &mut Buffer, content: &Buffer, level: i32) {
            use std::io::Write;

            // first header sighted
            if self.toc_level == 0 {
                self.toc_offset = level - 1;

                self.toc.push_str(r#"<nav id="toc""#);

                if let Align::Right = self.toc_align {
                    self.toc.push_str(r#"class="right-toc""#)
                }

                self.toc.push_str(">\n<h3>Contents</h3>");
            }

            let level = level - self.toc_offset;

            if level > self.toc_level {
                while level > self.toc_level {
                    self.toc.push_str("<ol>\n<li>\n");
                    self.toc_level += 1;
                }
            } else if level < self.toc_level {
                self.toc.push_str("</li>\n");

                while level < self.toc_level {
                    self.toc.push_str("</ol>\n</li>\n");
                    self.toc_level -= 1;
                }

                self.toc.push_str("<li>\n");
            } else {
                self.toc.push_str("</li>\n<li>\n");
            }

            let sanitized = sanitize(content.to_str().unwrap());
            self.toc.push_str(r##"<a href="#"##);
            self.toc.push_str(&sanitized);
            self.toc.push_str(r#"">"#);

            let bytes: &[u8] = content.as_ref();

            let doc =
                Markdown::from(bytes)
                .extensions({
                    use hoedown::*;

                    AUTOLINK |
                    FENCED_CODE |
                    FOOTNOTES |
                    MATH |
                    MATH_EXPLICIT |
                    SPACE_HEADERS |
                    STRIKETHROUGH |
                    SUPERSCRIPT |
                    TABLES
                });

            let rendered = self.html.render_inline(&doc);

            self.toc.push_str(rendered.to_str().unwrap());
            self.toc.push_str("</a>\n");

            write!(output,
r##"<h2 id="{}">
<span class="hash">#</span>
<a href="#{}" class="header-link">{}</a>
</h2>"##, sanitized, sanitized, content.to_str().unwrap()).unwrap();
        }
    }

    wrap!(Renderer);
}

pub struct HandleIf<C, H>
where C: Fn(&Item) -> bool, C: Sync + Send + 'static,
      H: Handle<Item> + Sync + Send + 'static {
    condition: C,
    handler: H,
}

impl<C, H> Handle<Item> for HandleIf<C, H>
where C: Fn(&Item) -> bool, C: Sync + Send + 'static,
      H: Handle<Item> + Sync + Send + 'static {
    fn handle(&self, item: &mut Item) -> handle::Result {
        if (self.condition)(item) {
            (self.handler.handle(item))
        } else {
            Ok(())
        }
    }
}

#[inline]
pub fn handle_if<C, H>(condition: C, handler: H) -> HandleIf<C, H>
where C: Fn(&Item) -> bool, C: Copy + Sync + Send + 'static,
      H: Handle<Item> + Sync + Send + 'static {
    HandleIf {
        condition: condition,
        handler: handler,
    }
}

