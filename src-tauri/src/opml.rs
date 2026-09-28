//! OPML 1.0/2.0 import and OPML 2.0 export (SPEC §6.4).

use std::fmt::Write as _;

use crate::feed::encoding;

/// A feed found in an OPML file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpmlFeed {
    /// `xmlUrl` as written in the file (not yet validated).
    pub url: String,
    /// The outline's `text`: what the exporting app displayed.
    pub text: Option<String>,
    /// The outline's `title`: usually the feed's own title.
    pub title: Option<String>,
    pub site_url: Option<String>,
    /// Folder path, flattened to one level as `Parent / Child` (SPEC §6.4).
    pub folder: Option<String>,
}

/// Reads the feeds in an OPML document. Outlines without `xmlUrl` are folders; nesting
/// deeper than one level is flattened into names like `Parent / Child`.
pub fn parse(bytes: &[u8]) -> Result<Vec<OpmlFeed>, String> {
    let text = encoding::decode(bytes, None);
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..roxmltree::ParsingOptions::default()
    };
    let doc = roxmltree::Document::parse_with_options(&text, options)
        .map_err(|err| format!("This isn't a valid OPML file ({err})"))?;
    let root = doc.root_element();
    if !root.tag_name().name().eq_ignore_ascii_case("opml") {
        return Err("This isn't an OPML file".into());
    }
    let body = root
        .children()
        .find(|n| n.is_element() && n.tag_name().name().eq_ignore_ascii_case("body"))
        .ok_or("This OPML file has no <body>")?;
    let mut feeds = Vec::new();
    collect(body, &mut Vec::new(), &mut feeds);
    Ok(feeds)
}

fn attr<'a>(node: roxmltree::Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.attributes()
        .find(|a| a.name().eq_ignore_ascii_case(name))
        .map(|a| a.value().trim())
        .filter(|v| !v.is_empty())
}

fn collect(node: roxmltree::Node, path: &mut Vec<String>, out: &mut Vec<OpmlFeed>) {
    for child in node
        .children()
        .filter(|n| n.is_element() && n.tag_name().name().eq_ignore_ascii_case("outline"))
    {
        let text = attr(child, "text").map(str::to_string);
        let title = attr(child, "title").map(str::to_string);
        if let Some(url) = attr(child, "xmlUrl") {
            out.push(OpmlFeed {
                url: url.to_string(),
                text,
                title,
                site_url: attr(child, "htmlUrl").map(str::to_string),
                folder: (!path.is_empty()).then(|| path.join(" / ")),
            });
            // Some exporters nest feeds under feeds; keep them in the same folder.
            collect(child, path, out);
        } else if let Some(name) = text.or(title) {
            path.push(name);
            collect(child, path, out);
            path.pop();
        } else {
            collect(child, path, out);
        }
    }
}

/// A feed to export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportFeed {
    /// Display title (custom title if set).
    pub text: String,
    /// The feed's own title.
    pub title: String,
    pub url: String,
    pub site_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportFolder {
    pub name: String,
    pub feeds: Vec<ExportFeed>,
}

/// Writes an OPML 2.0 document: folders as outlines containing their feeds, then top-level
/// feeds. `text` carries the displayed (custom) title and `title` the feed's own.
pub fn export(folders: &[ExportFolder], feeds: &[ExportFeed], created: &str) -> String {
    let mut out =
        String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<opml version=\"2.0\">\n");
    let _ = write!(
        out,
        "  <head>\n    <title>Omarss subscriptions</title>\n    <dateCreated>{}</dateCreated>\n  </head>\n  <body>\n",
        escape(created)
    );
    for folder in folders {
        let name = escape(&folder.name);
        let _ = writeln!(out, "    <outline text=\"{name}\" title=\"{name}\">");
        for feed in &folder.feeds {
            write_feed(&mut out, feed, "      ");
        }
        out.push_str("    </outline>\n");
    }
    for feed in feeds {
        write_feed(&mut out, feed, "    ");
    }
    out.push_str("  </body>\n</opml>\n");
    out
}

fn write_feed(out: &mut String, feed: &ExportFeed, indent: &str) {
    let _ = write!(
        out,
        "{indent}<outline type=\"rss\" text=\"{}\" title=\"{}\" xmlUrl=\"{}\"",
        escape(&feed.text),
        escape(&feed.title),
        escape(&feed.url)
    );
    if let Some(site) = &feed.site_url {
        let _ = write!(out, " htmlUrl=\"{}\"", escape(site));
    }
    out.push_str("/>\n");
}

fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            '\t' => out.push_str("&#9;"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(url: &str, text: Option<&str>, folder: Option<&str>) -> OpmlFeed {
        OpmlFeed {
            url: url.into(),
            text: text.map(Into::into),
            title: text.map(Into::into),
            site_url: None,
            folder: folder.map(Into::into),
        }
    }

    #[test]
    fn nested_outlines_become_flat_folders() {
        let opml = br#"<?xml version="1.0"?>
<opml version="1.0"><head><title>x</title></head><body>
  <outline text="Top" title="Top" type="rss" xmlUrl="https://top.example/feed"/>
  <outline text="Tech">
    <outline text="Rust" title="Rust" type="rss" xmlUrl="https://rust.example/feed.xml" htmlUrl="https://rust.example/"/>
    <outline title="Deep">
      <outline text="Deeper">
        <outline text="Nested" xmlUrl="https://nested.example/rss"/>
      </outline>
    </outline>
  </outline>
  <outline text="Empty"/>
  <outline xmlurl="https://lowercase.example/rss" text="Lower &amp; case"/>
</body></opml>"#;
        let feeds = parse(opml).unwrap();
        assert_eq!(feeds.len(), 4);
        assert_eq!(
            feeds[0],
            feed("https://top.example/feed", Some("Top"), None)
        );
        assert_eq!(feeds[1].folder.as_deref(), Some("Tech"));
        assert_eq!(feeds[1].site_url.as_deref(), Some("https://rust.example/"));
        assert_eq!(feeds[2].folder.as_deref(), Some("Tech / Deep / Deeper"));
        assert_eq!(feeds[3].text.as_deref(), Some("Lower & case"));
        assert_eq!(feeds[3].folder, None);
    }

    #[test]
    fn rejects_documents_that_are_not_opml() {
        assert!(parse(b"<rss><channel/></rss>").is_err());
        assert!(parse(b"not xml").is_err());
        assert!(parse(b"<opml version=\"2.0\"><head/></opml>").is_err());
        assert_eq!(parse(b"<opml><body/></opml>").unwrap(), Vec::new());
    }

    #[test]
    fn decodes_legacy_encodings() {
        let mut bytes =
            b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><opml><body><outline text=\"Caf"
                .to_vec();
        bytes.push(0xE9);
        bytes.extend_from_slice(b"\" xmlUrl=\"https://cafe.example/rss\"/></body></opml>");
        assert_eq!(parse(&bytes).unwrap()[0].text.as_deref(), Some("Café"));
    }

    #[test]
    fn export_round_trips() {
        let folders = vec![ExportFolder {
            name: "News & \"Views\"".into(),
            feeds: vec![ExportFeed {
                text: "My <name>".into(),
                title: "Original".into(),
                url: "https://a.example/feed?x=1&y=2".into(),
                site_url: Some("https://a.example/".into()),
            }],
        }];
        let root = vec![ExportFeed {
            text: "Root".into(),
            title: "Root".into(),
            url: "https://b.example/rss".into(),
            site_url: None,
        }];
        let xml = export(&folders, &root, "Sun, 27 Sep 2026 10:00:00 +0000");
        assert!(xml.contains("<opml version=\"2.0\">"));
        let parsed = parse(xml.as_bytes()).unwrap();
        assert_eq!(
            parsed,
            vec![
                OpmlFeed {
                    url: "https://a.example/feed?x=1&y=2".into(),
                    text: Some("My <name>".into()),
                    title: Some("Original".into()),
                    site_url: Some("https://a.example/".into()),
                    folder: Some("News & \"Views\"".into()),
                },
                feed("https://b.example/rss", Some("Root"), None),
            ]
        );
    }
}
