use std::path::Path;

use diecast::{self, Handle, Item};
use diecast::util::handle::item;

use rss;
use toml;

// TODO
// probably invert this so that you give it the slice of items and it
// produces an Rss?
pub fn rss(item: &mut Item) -> diecast::Result {
    let site = String::from("http://www.blaenkdenum.com");

    let count = item.bind().dependencies["posts"].items().len();

    let feed_items =
        item.bind().dependencies["posts"].iter()
        .take(10)
        .map(|i| {
            let mut feed_item: rss::Item = Default::default();

            feed_item.pub_date =
                i.extensions.get::<item::Date>()
                .map(ToString::to_string);

            feed_item.description =
                i.extensions.get::<item::Versions>()
                .and_then(|versions| versions.get("rendered").map(Clone::clone));

            if let Some(meta) = i.extensions.get::<item::Metadata>() {
                feed_item.title =
                    meta.lookup("title")
                    .and_then(toml::Value::as_str)
                    .map(String::from);

                feed_item.link =
                    i.route.writing()
                    .and_then(Path::parent)
                    .and_then(Path::to_str)
                    .map(|p| format!("{}/{}", &site, p));
            }

            feed_item
        })
        .collect::<Vec<rss::Item>>();

    let channel = rss::Channel {
        title: String::from("Blaenk Denum"),
        link: site,
        items: feed_items,
        .. Default::default()
    };

    item.body = rss::Rss(channel).to_string();

    Ok(())
}
