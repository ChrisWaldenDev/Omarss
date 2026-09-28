//! Parsing fixtures (SPEC §13): real-world feed formats and the ways they break.

use super::fixtures::{feed_fixture, feed_fixture_names};
use crate::feed::{parse, ParsedFeed, ParsedItem};

const FEED_URL: &str = "https://feeds.example/path/feed.xml";
const INVALID: [&str; 2] = ["not_a_feed.html", "truncated.xml"];

fn content_type(name: &str) -> Option<&'static str> {
    if name.ends_with(".json") {
        Some("application/feed+json")
    } else if name.starts_with("windows1252") {
        Some("text/xml; charset=windows-1252")
    } else {
        Some("application/rss+xml")
    }
}

fn load(name: &str) -> ParsedFeed {
    parse(&feed_fixture(name), content_type(name), FEED_URL)
        .unwrap_or_else(|err| panic!("{name}: {err}"))
}

fn item<'a>(feed: &'a ParsedFeed, guid: &str) -> &'a ParsedItem {
    feed.items
        .iter()
        .find(|i| i.guid == guid)
        .unwrap_or_else(|| {
            panic!(
                "no item {guid:?} in {:?}",
                feed.items.iter().map(|i| &i.guid).collect::<Vec<_>>()
            )
        })
}

fn html(item: &ParsedItem) -> String {
    format!(
        "{}{}",
        item.summary_html.as_deref().unwrap_or(""),
        item.content_html.as_deref().unwrap_or("")
    )
}

fn ts(rfc3339: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .unwrap()
        .timestamp()
}

#[test]
fn the_corpus_has_at_least_30_feeds() {
    let valid = feed_fixture_names().len() - INVALID.len();
    assert!(valid >= 30, "only {valid} valid fixtures");
}

#[test]
fn every_fixture_parses_into_clean_items_except_the_broken_ones() {
    for name in feed_fixture_names() {
        let result = parse(&feed_fixture(&name), content_type(&name), FEED_URL);
        if INVALID.contains(&name.as_str()) {
            assert!(result.is_err(), "{name} should not parse");
            continue;
        }
        let feed = result.unwrap_or_else(|err| panic!("{name}: {err}"));
        assert!(feed.title.is_some(), "{name}: no title");
        if name != "empty_channel.xml" {
            assert!(!feed.items.is_empty(), "{name}: no items");
        }
        for item in &feed.items {
            assert!(!item.guid.trim().is_empty(), "{name}: empty guid");
            assert!(!item.title.trim().is_empty(), "{name}: empty title");
            if let Some(url) = &item.url {
                assert!(
                    url.starts_with("http://") || url.starts_with("https://"),
                    "{name}: {url}"
                );
            }
            let body = html(item).to_ascii_lowercase();
            for bad in [
                "<script",
                "javascript:",
                "onerror",
                "onclick",
                "<iframe",
                "<style",
                "style=",
            ] {
                assert!(!body.contains(bad), "{name}: {bad} in {body}");
            }
        }
    }
}

#[test]
fn wordpress() {
    let feed = load("wordpress_rss2.xml");
    assert_eq!(feed.title.as_deref(), Some("Garden Notes"));
    assert_eq!(feed.site_url.as_deref(), Some("https://garden.example/"));
    assert_eq!(
        feed.icon_url.as_deref(),
        Some("https://garden.example/wp-content/uploads/cropped-icon-32x32.png")
    );
    assert_eq!(feed.update_hint_secs, Some(3600), "sy:updatePeriod hourly");
    let post = item(&feed, "https://garden.example/?p=412");
    assert_eq!(post.title, "Tomatoes in September");
    assert_eq!(post.author.as_deref(), Some("Priya"));
    assert_eq!(post.published_at, Some(ts("2024-09-02T09:12:44Z")));
    let content = post.content_html.as_deref().unwrap();
    assert!(
        content.contains("<figure>") && content.contains("<figcaption>"),
        "{content}"
    );
    assert!(
        content.contains("tomatoes-300x225.jpg 300w"),
        "srcset kept: {content}"
    );
    assert!(post.summary_html.as_deref().unwrap().contains('…'));
}

#[test]
fn blogger_uses_the_alternate_link() {
    let feed = load("blogger_atom.xml");
    let post = &feed.items[0];
    assert_eq!(
        post.url.as_deref(),
        Some("https://trains.example/2024/06/night-train.html")
    );
    assert_eq!(post.published_at, Some(ts("2024-06-01T16:30:00Z")));
    assert!(html(post).contains("<b>midnight</b>"));
    assert_eq!(feed.site_url.as_deref(), Some("https://trains.example/"));
}

#[test]
fn github_releases_fall_back_to_updated_dates() {
    let feed = load("github_releases_atom.xml");
    let release = &feed.items[0];
    assert_eq!(release.title, "v2.1.0");
    assert_eq!(release.published_at, Some(ts("2024-08-05T14:00:00Z")));
    assert!(
        html(release).contains("What's changed"),
        "{}",
        html(release)
    );
}

#[test]
fn youtube_uses_the_media_description() {
    let feed = load("youtube_atom.xml");
    let video = &feed.items[0];
    assert_eq!(
        video.url.as_deref(),
        Some("https://video.example/watch?v=abc123")
    );
    assert!(html(video).contains("Oak, glue and patience."));
}

#[test]
fn thumbnails_come_from_media_rss_then_content() {
    let youtube = load("youtube_atom.xml");
    assert_eq!(
        youtube.items[0].thumbnail_url.as_deref(),
        Some("https://i.video.example/vi/abc123/hqdefault.jpg")
    );
    let photos = load("media_rss.xml");
    assert_eq!(
        item(&photos, "p1").thumbnail_url.as_deref(),
        Some("https://photos.example/thumb/p1.jpg"),
        "media:thumbnail wins over the full-size media:content"
    );
    assert_eq!(
        item(&photos, "p2").thumbnail_url,
        None,
        "videos aren't thumbnails"
    );
    let letter = load("substack_rss.xml");
    assert_eq!(
        letter.items[0].thumbnail_url.as_deref(),
        Some("https://letter.example/img/envelopes.jpeg"),
        "first image in the content"
    );
}

#[test]
fn podcast_enclosures_and_ttl() {
    let feed = load("podcast_itunes.xml");
    assert_eq!(feed.update_hint_secs, Some(3600), "ttl 60 minutes");
    let episode = item(&feed, "slowkitchen-ep-12");
    assert_eq!(episode.enclosures.len(), 1);
    assert_eq!(
        episode.enclosures[0].url,
        "https://media.slowkitchen.example/ep12.mp3"
    );
    assert_eq!(
        episode.enclosures[0].mime_type.as_deref(),
        Some("audio/mpeg")
    );
    assert_eq!(episode.enclosures[0].length, Some(48_213_000));
}

#[test]
fn zero_length_enclosures_have_no_length() {
    let feed = load("substack_rss.xml");
    assert_eq!(feed.items[0].enclosures[0].length, None);
    let feed = load("podcast_namespace.xml");
    assert_eq!(
        feed.items[0].enclosures.len(),
        1,
        "transcripts and chapters aren't enclosures"
    );
}

#[test]
fn missing_guids_fall_back_to_link_then_hash() {
    let feed = load("missing_guids.xml");
    assert_eq!(feed.items[0].guid, "https://noguid.example/a");
    let (b, c) = (&feed.items[1].guid, &feed.items[2].guid);
    assert_eq!(b.len(), 64, "sha-256 hex: {b}");
    assert_ne!(b, c);
    assert_eq!(
        load("missing_guids.xml").items[1].guid,
        *b,
        "stable across fetches"
    );
    let hn = load("hn_rss.xml");
    assert_eq!(hn.items[0].guid, "https://tinydb.example/");
}

#[test]
fn items_without_titles_get_an_excerpt() {
    let feed = load("mastodon_rss.xml");
    assert_eq!(
        feed.items[0].title,
        "Finally finished the quilt! Took three winters."
    );
    assert_eq!(
        feed.items[0].enclosures[0].mime_type.as_deref(),
        Some("image/jpeg")
    );
    let json = load("jsonfeed_v1.json");
    assert_eq!(
        json.items[0].title,
        "Short note with two paragraphs & an ampersand."
    );
}

#[test]
fn json_feed() {
    let v1 = load("jsonfeed_v1.json");
    assert_eq!(v1.site_url.as_deref(), Some("https://micro.example/"));
    assert_eq!(
        v1.items[0].content_html.as_deref(),
        Some("<p>Short note</p><p>with two paragraphs &amp; an ampersand.</p>")
    );
    let audio = item(&v1, "podcast-1");
    assert_eq!(audio.enclosures[0].url, "https://micro.example/audio/1.m4a");
    assert_eq!(audio.enclosures[0].length, Some(1_234_567));

    let v11 = load("jsonfeed_v11.json");
    let post = item(&v11, "42");
    assert_eq!(post.author.as_deref(), Some("Dee"));
    assert!(
        html(post).contains(r#"href="https://lab.example/posts/41""#),
        "{}",
        html(post)
    );
}

#[test]
fn rdf_and_old_rss_versions() {
    let rdf = load("rdf_rss1.xml");
    assert_eq!(rdf.items.len(), 2);
    assert_eq!(rdf.items[0].author.as_deref(), Some("kd"));
    assert_eq!(rdf.items[0].published_at, Some(ts("2024-09-08T21:00:00Z")));
    assert_eq!(rdf.update_hint_secs, Some(21_600), "daily, 4 times");

    let old = load("rss091.xml");
    assert_eq!(old.items[0].guid, "http://oldschool.example/redesign.html");
    let radio = load("rss092.xml");
    assert_eq!(radio.items[0].enclosures[0].length, Some(5_588_242));
    assert!(html(&radio.items[0]).contains("Music &amp; talk"));
}

#[test]
fn relative_urls_resolve_against_item_link_then_xml_base_then_feed() {
    let based = load("atom_xml_base.xml");
    let entry = &based.items[0];
    assert_eq!(
        entry.url.as_deref(),
        Some("https://based.example/blog/2024/relative.html")
    );
    let body = html(entry);
    assert!(
        body.contains(r#"href="https://based.example/blog/2024/other.html""#),
        "{body}"
    );
    assert!(
        body.contains(r#"src="https://based.example/blog/img/a.png""#),
        "{body}"
    );
    assert_eq!(
        based.icon_url.as_deref(),
        Some("https://based.example/icon.png")
    );

    let rob = load("rss_relative_links.xml");
    let with_link = item(&rob, "rob-1");
    assert_eq!(
        with_link.url.as_deref(),
        Some("https://feeds.example/posts/1.html")
    );
    assert!(
        html(with_link).contains(r#"src="https://feeds.example/posts/images/1.png""#),
        "{}",
        html(with_link)
    );
    assert!(html(item(&rob, "rob-2")).contains(r#"href="https://feeds.example/path/notes/2.html""#));
}

#[test]
fn broken_dates_are_tolerated() {
    let feed = load("bad_dates.xml");
    assert_eq!(item(&feed, "d1").published_at, None, "garbage");
    assert_eq!(
        item(&feed, "d2").published_at,
        Some(ts("2024-09-06T15:00:00Z")),
        "EST"
    );
    assert_eq!(
        item(&feed, "d3").published_at,
        Some(ts("2024-09-06T10:00:00Z")),
        "no weekday"
    );
    assert!(item(&feed, "d4").published_at.is_some(), "ISO without zone");
    assert_eq!(item(&feed, "d5").published_at, None);
    assert!(item(&feed, "d6").published_at.unwrap() > ts("2098-01-01T00:00:00Z"));
}

#[test]
fn malformed_xml_is_repaired() {
    let salt = load("unescaped_ampersand.xml");
    assert_eq!(salt.title.as_deref(), Some("Salt & Pepper"));
    assert_eq!(salt.items[0].title, "Fish & chips");
    assert_eq!(
        salt.items[0].url.as_deref(),
        Some("https://salt.example/fish?x=1&y=2")
    );
    assert!(html(&salt.items[0]).contains('©'));

    assert_eq!(load("rss_no_version.xml").items[0].title, "Still works");
}

#[test]
fn entities_in_titles_are_decoded() {
    let feed = load("html_entities.xml");
    assert_eq!(feed.title.as_deref(), Some("Entities Weekly — Issue 1"));
    assert_eq!(item(&feed, "e1").title, "It’s “quoted” …");
    assert_eq!(item(&feed, "e2").title, "Numeric – — refs");
    assert!(html(item(&feed, "e1")).contains("café &amp; crème"));
}

#[test]
fn text_constructs() {
    let cdata = load("cdata_everything.xml");
    assert_eq!(
        cdata.items[0].title, "Vec<T> & friends",
        "plain-text titles keep angle brackets"
    );

    let types = load("atom_text_types.xml");
    assert_eq!(types.title.as_deref(), Some("Types & Markup"));
    let entry = &types.items[0];
    assert_eq!(entry.title, "An XHTML title");
    assert_eq!(
        entry.summary_html.as_deref(),
        Some("<p>Plain &lt;not a tag&gt; summary</p>")
    );
    assert!(entry
        .content_html
        .as_deref()
        .unwrap()
        .contains("<strong>XHTML</strong>"));

    let comments = load("comment_feed_atom.xml");
    assert_eq!(
        comments.items[0].content_html.as_deref(),
        Some("<p>Nice post!<br>Second line.</p><p>New paragraph.</p>")
    );
    let padded = load("whitespace_padded.xml");
    assert_eq!(padded.title.as_deref(), Some("Padded Title"));
    assert_eq!(padded.items[0].title, "Spaced out");
    assert_eq!(padded.items[0].guid, "pad-1");
}

#[test]
fn media_rss_enclosures() {
    let feed = load("media_rss.xml");
    let photo = item(&feed, "p1");
    assert_eq!(
        photo.enclosures[0].url,
        "https://photos.example/full/p1.jpg"
    );
    assert_eq!(photo.enclosures[0].length, Some(3_000_000));
    assert_eq!(item(&feed, "p2").enclosures.len(), 2);
}

#[test]
fn character_encodings() {
    assert_eq!(load("latin1.xml").title.as_deref(), Some("Café Crème"));
    assert_eq!(load("latin1.xml").items[0].title, "Résumé des événements");
    assert_eq!(
        load("windows1252_no_decl.xml").items[0].title,
        "“Quoted” – and ’tis"
    );
    assert_eq!(load("utf16_bom.xml").title.as_deref(), Some("Sixteen über"));
    assert_eq!(load("utf8_bom.xml").items[0].title, "Checked ✓");
    // Declared UTF-8 but isn't: invalid bytes become U+FFFD instead of failing.
    assert_eq!(
        load("invalid_utf8.xml").items[0].title,
        "Broken \u{fffd} title"
    );
    // Without the HTTP charset, Windows-1252 bytes are invalid UTF-8.
    let raw = parse(&feed_fixture("windows1252_no_decl.xml"), None, FEED_URL).unwrap();
    assert!(raw.items[0].title.contains('\u{fffd}'));
}

#[test]
fn hostile_content_is_sanitised_but_text_survives() {
    let feed = load("xss_feed.xml");
    let body = html(item(&feed, "x1"));
    assert!(
        body.contains("Hi") && body.contains("styled") && body.contains("click"),
        "{body}"
    );
    assert!(item(&feed, "x2").title.ends_with("Title"));
}
