//! Built-in demo data for the M1 skeleton (SPEC §14: "three-pane layout with dummy data").
//!
//! This stands in for the feed/article repositories until M2 adds real fetching and storage.
//! It never touches the database, so no dummy rows end up in a user's data.

use std::sync::OnceLock;

use crate::clock;
use crate::models::{
    Article, ArticleListItem, ArticleQuery, Enclosure, FeedNode, FolderNode, Sidebar, SortOrder,
    View, ViewCounts,
};

struct DemoFolder {
    id: i64,
    name: &'static str,
}

struct DemoFeed {
    id: i64,
    folder_id: Option<i64>,
    title: &'static str,
    slug: &'static str,
    error_count: u32,
    author: Option<&'static str>,
    technical: bool,
    podcast: bool,
    titles: &'static [&'static str],
}

struct DemoArticle {
    id: i64,
    feed_id: i64,
    title: &'static str,
    url: String,
    author: Option<&'static str>,
    content_html: String,
    summary: String,
    published_at: i64,
    is_read: bool,
    is_starred: bool,
    enclosures: Vec<Enclosure>,
}

struct DemoData {
    folders: Vec<DemoFolder>,
    feeds: Vec<DemoFeed>,
    articles: Vec<DemoArticle>,
}

const FOLDERS: [(i64, &str); 3] = [(1, "Technology"), (2, "News"), (3, "Blogs")];

fn demo_feeds() -> Vec<DemoFeed> {
    vec![
        DemoFeed {
            id: 1,
            folder_id: Some(1),
            title: "Systems Weekly",
            slug: "systems-weekly",
            error_count: 0,
            author: Some("Dana Whitfield"),
            technical: true,
            podcast: false,
            titles: &[
                "Why your p99 latency lies to you",
                "A gentle tour of io_uring",
                "Designing back-pressure that actually works",
                "The hidden cost of small allocations",
                "Profiling a cold start, frame by frame",
                "Lock-free queues: when not to bother",
                "What WAL mode really buys you",
            ],
        },
        DemoFeed {
            id: 2,
            folder_id: Some(1),
            title: "The Compiler Journal",
            slug: "compiler-journal",
            error_count: 0,
            author: Some("Ren Okafor"),
            technical: true,
            podcast: false,
            titles: &[
                "Incremental compilation, explained with diagrams",
                "Trait objects versus generics in practice",
                "Error messages are a user interface",
                "Inside a borrow checker",
                "Shrinking binaries without losing your mind",
                "Async traits, one year later",
            ],
        },
        DemoFeed {
            id: 3,
            folder_id: Some(1),
            title: "Frontend Field Notes",
            slug: "frontend-field-notes",
            error_count: 0,
            author: Some("Mika Laine"),
            technical: true,
            podcast: false,
            titles: &[
                "Virtual lists that scroll at 120 fps",
                "Colour tokens that survive a dark mode",
                "Focus rings are not optional",
                "Signals, stores and when to use which",
                "Typography for long-form reading on screens",
            ],
        },
        DemoFeed {
            id: 4,
            folder_id: Some(2),
            title: "Morning Wire",
            slug: "morning-wire",
            error_count: 0,
            author: None,
            technical: false,
            podcast: false,
            titles: &[
                "City council approves new cycling corridor",
                "Regional rail timetable changes this weekend",
                "Library extends evening opening hours",
                "Harbour cleanup volunteers exceed target",
                "Local bakery wins national bread award",
                "Heat advisory issued for the valley",
                "Night market returns to the old square",
                "School robotics team heads to finals",
            ],
        },
        DemoFeed {
            id: 5,
            folder_id: Some(2),
            title: "Science Digest",
            slug: "science-digest",
            error_count: 0,
            author: Some("Science Digest staff"),
            technical: false,
            podcast: false,
            titles: &[
                "Deep-sea microbes that breathe metal",
                "What tree rings tell us about old storms",
                "A new map of the Milky Way's warp",
                "Why octopuses dream, maybe",
                "The chemistry of a perfect crust",
            ],
        },
        DemoFeed {
            id: 6,
            folder_id: Some(3),
            title: "Slow Kitchen",
            slug: "slow-kitchen",
            error_count: 0,
            author: Some("Ana Castell"),
            technical: false,
            podcast: false,
            titles: &[
                "A weeknight ragù that tastes like Sunday",
                "Notes on fermenting hot sauce",
                "The case for cast iron",
                "Bread, water, salt, time",
            ],
        },
        DemoFeed {
            id: 7,
            folder_id: Some(3),
            title: "Trail Notes",
            slug: "trail-notes",
            error_count: 0,
            author: Some("Sam Oduya"),
            technical: false,
            podcast: true,
            titles: &[
                "Episode 41: Walking the ridge in fog",
                "Episode 40: Packing light for three days",
                "Episode 39: Reading a topographic map",
                "Episode 38: Why we hike in winter",
            ],
        },
        DemoFeed {
            id: 8,
            folder_id: None,
            title: "Personal Log",
            slug: "personal-log",
            error_count: 0,
            author: Some("Me"),
            technical: false,
            podcast: false,
            titles: &[
                "Trying a new reading routine",
                "Things I learned this month",
                "Keyboard shortcuts I actually use",
            ],
        },
        DemoFeed {
            id: 9,
            folder_id: None,
            title: "Broken Example Feed",
            slug: "broken-example",
            error_count: 4,
            author: None,
            technical: false,
            podcast: false,
            titles: &[],
        },
    ]
}

/// Deterministic pseudo-random numbers (SplitMix64), so the demo looks the same every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

const PARAGRAPHS: [&str; 6] = [
    "Most of the interesting work happens in the details that nobody sees at first glance. \
     This piece walks through them one at a time, with notes on what surprised us along the way.",
    "The first attempt was simple and mostly worked. The second attempt was clever and mostly \
     did not. The third attempt, described below, is simple again, but for better reasons.",
    "If you only remember one thing, make it this: measure before and after, and write down \
     what you expected to happen before you look at the numbers.",
    "There is a trade-off here, and it is worth naming explicitly. Faster is not always better \
     if it makes the next change harder to reason about.",
    "We asked a few readers how they approach the same problem. Their answers were more varied \
     than expected, and a couple of them changed how we think about it.",
    "None of this is final. Treat it as a field report rather than a recipe, and adapt it to \
     the constraints you actually have.",
];

const CODE_SAMPLE: &str = "fn main() {\n    let items = load_items()?;\n    for item in items.iter().filter(|i| !i.is_read) {\n        println!(\"{}\", item.title);\n    }\n}";

fn content_for(feed: &DemoFeed, title: &str, url: &str, rng: &mut Rng) -> (String, String) {
    let first = PARAGRAPHS[rng.below(PARAGRAPHS.len() as u64) as usize];
    let lead = format!("{title}. {first}");
    let mut html = format!("<p>{lead}</p>");
    for _ in 0..2 {
        let p = PARAGRAPHS[rng.below(PARAGRAPHS.len() as u64) as usize];
        html.push_str(&format!("<p>{p}</p>"));
    }
    html.push_str("<h2>What we found</h2><ul><li>Small changes compound.</li>");
    html.push_str("<li>Defaults matter more than options.</li>");
    html.push_str("<li>Write it down while it is fresh.</li></ul>");
    if feed.technical {
        html.push_str(&format!("<pre><code>{CODE_SAMPLE}</code></pre>"));
    }
    html.push_str(
        "<blockquote><p>The best time to simplify is before you add the second feature.</p>\
         </blockquote>",
    );
    let p = PARAGRAPHS[rng.below(PARAGRAPHS.len() as u64) as usize];
    html.push_str(&format!(
        "<p>{p} <a href=\"{url}\">Read the original post</a> for the full discussion.</p>"
    ));
    (html, excerpt(&lead, 140))
}

/// Truncates `text` to at most `max` characters on a word boundary, adding an ellipsis.
fn excerpt(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max - 1).collect();
    let cut = cut.rsplit_once(' ').map_or(cut.as_str(), |(head, _)| head);
    format!("{}…", cut.trim_end_matches(|c: char| !c.is_alphanumeric()))
}

fn build(now: i64) -> DemoData {
    let feeds = demo_feeds();
    let mut rng = Rng(0x00DD_BA11);
    let mut articles = Vec::new();
    let mut next_id = 1;

    for feed in &feeds {
        let mut age_minutes = 15 + rng.below(240) as i64;
        for (i, &title) in feed.titles.iter().enumerate() {
            let url = format!("https://example.com/{}/{}", feed.slug, i + 1);
            let (content_html, summary) = content_for(feed, title, &url, &mut rng);
            let enclosures = if feed.podcast {
                vec![Enclosure {
                    url: format!("https://example.com/{}/episode-{}.mp3", feed.slug, i + 1),
                    mime_type: Some("audio/mpeg".into()),
                    length: Some(24_000_000 + rng.below(8_000_000) as i64),
                }]
            } else {
                Vec::new()
            };
            // Older articles are more likely to have been read already.
            let is_read = rng.below(10) < (2 + i as u64 * 2).min(8);
            let is_starred = rng.below(8) == 0;
            articles.push(DemoArticle {
                id: next_id,
                feed_id: feed.id,
                title,
                url,
                author: feed.author,
                content_html,
                summary,
                published_at: now - age_minutes * 60,
                is_read,
                is_starred,
                enclosures,
            });
            next_id += 1;
            age_minutes += 90 + rng.below(60 * 30) as i64;
        }
    }

    DemoData {
        folders: FOLDERS
            .iter()
            .map(|&(id, name)| DemoFolder { id, name })
            .collect(),
        feeds,
        articles,
    }
}

fn data() -> &'static DemoData {
    static DATA: OnceLock<DemoData> = OnceLock::new();
    DATA.get_or_init(|| build(clock::now_unix()))
}

impl DemoData {
    fn feed(&self, id: i64) -> Option<&DemoFeed> {
        self.feeds.iter().find(|f| f.id == id)
    }

    fn feed_title(&self, id: i64) -> String {
        self.feed(id)
            .map_or_else(String::new, |f| f.title.to_string())
    }

    fn unread_in_feed(&self, feed_id: i64) -> u32 {
        count(
            self.articles
                .iter()
                .filter(|a| a.feed_id == feed_id && !a.is_read),
        )
    }

    fn feed_node(&self, feed: &DemoFeed) -> FeedNode {
        FeedNode {
            id: feed.id,
            folder_id: feed.folder_id,
            title: feed.title.to_string(),
            site_url: Some(format!("https://example.com/{}", feed.slug)),
            unread_count: self.unread_in_feed(feed.id),
            error_count: feed.error_count,
        }
    }

    fn in_view(&self, article: &DemoArticle, view: &View, today_start: i64) -> bool {
        match view {
            View::All => true,
            View::Unread => !article.is_read,
            View::Starred => article.is_starred,
            View::Today => article.published_at >= today_start,
            View::Feed { id } => article.feed_id == *id,
            View::Folder { id } => self
                .feed(article.feed_id)
                .is_some_and(|f| f.folder_id == Some(*id)),
        }
    }
}

fn count<'a>(it: impl Iterator<Item = &'a DemoArticle>) -> u32 {
    u32::try_from(it.count()).unwrap_or(u32::MAX)
}

pub fn sidebar() -> Sidebar {
    let data = data();
    let today_start = clock::local_day_start();
    let folders = data
        .folders
        .iter()
        .map(|folder| {
            let feeds: Vec<FeedNode> = data
                .feeds
                .iter()
                .filter(|f| f.folder_id == Some(folder.id))
                .map(|f| data.feed_node(f))
                .collect();
            FolderNode {
                id: folder.id,
                name: folder.name.to_string(),
                unread_count: feeds.iter().map(|f| f.unread_count).sum(),
                feeds,
            }
        })
        .collect();
    let feeds = data
        .feeds
        .iter()
        .filter(|f| f.folder_id.is_none())
        .map(|f| data.feed_node(f))
        .collect();
    let unread = || data.articles.iter().filter(|a| !a.is_read);
    Sidebar {
        counts: ViewCounts {
            unread: count(unread()),
            starred: count(data.articles.iter().filter(|a| a.is_starred)),
            today: count(unread().filter(|a| a.published_at >= today_start)),
        },
        folders,
        feeds,
    }
}

pub fn list_articles(query: &ArticleQuery) -> Vec<ArticleListItem> {
    let data = data();
    let today_start = clock::local_day_start();
    // The Starred view always lists every starred article; hiding read ones would make
    // starred items seem to disappear once read.
    let unread_only = query.unread_only && query.view != View::Starred;
    let mut items: Vec<&DemoArticle> = data
        .articles
        .iter()
        .filter(|a| data.in_view(a, &query.view, today_start))
        .filter(|a| !unread_only || !a.is_read)
        .collect();
    items.sort_by_key(|a| (a.published_at, a.id));
    if query.sort == SortOrder::NewestFirst {
        items.reverse();
    }
    items
        .into_iter()
        .map(|a| ArticleListItem {
            id: a.id,
            feed_id: a.feed_id,
            feed_title: data.feed_title(a.feed_id),
            title: a.title.to_string(),
            summary: a.summary.clone(),
            published_at: Some(a.published_at),
            is_read: a.is_read,
            is_starred: a.is_starred,
        })
        .collect()
}

pub fn get_article(id: i64) -> Option<Article> {
    let data = data();
    data.articles.iter().find(|a| a.id == id).map(|a| Article {
        id: a.id,
        feed_id: a.feed_id,
        feed_title: data.feed_title(a.feed_id),
        title: a.title.to_string(),
        url: Some(a.url.clone()),
        author: a.author.map(str::to_string),
        content_html: a.content_html.clone(),
        published_at: Some(a.published_at),
        is_read: a.is_read,
        is_starred: a.is_starred,
        enclosures: a.enclosures.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(view: View, unread_only: bool) -> ArticleQuery {
        ArticleQuery {
            view,
            unread_only,
            sort: SortOrder::NewestFirst,
        }
    }

    #[test]
    fn has_a_useful_amount_of_data() {
        let all = list_articles(&query(View::All, false));
        assert!(all.len() >= 40);
        assert!(all.iter().any(|a| a.is_read));
        assert!(all.iter().any(|a| !a.is_read));
        assert!(all.iter().any(|a| a.is_starred));
        let sidebar = sidebar();
        assert!(sidebar.feeds.iter().any(|f| f.error_count >= 3));
        assert!(!sidebar.folders.is_empty());
    }

    #[test]
    fn sidebar_counts_match_article_lists() {
        let s = sidebar();
        let len = |v: View, unread_only: bool| list_articles(&query(v, unread_only)).len() as u32;
        assert_eq!(s.counts.unread, len(View::All, true));
        assert_eq!(s.counts.unread, len(View::Unread, false));
        assert_eq!(s.counts.starred, len(View::Starred, true));
        assert_eq!(s.counts.today, len(View::Today, true));
        for folder in &s.folders {
            assert_eq!(
                folder.unread_count,
                len(View::Folder { id: folder.id }, true)
            );
            for feed in &folder.feeds {
                assert_eq!(feed.unread_count, len(View::Feed { id: feed.id }, true));
            }
        }
    }

    #[test]
    fn sorts_by_published_date() {
        let newest = list_articles(&query(View::All, false));
        assert!(newest
            .windows(2)
            .all(|w| w[0].published_at >= w[1].published_at));
        let mut q = query(View::All, false);
        q.sort = SortOrder::OldestFirst;
        let oldest = list_articles(&q);
        assert!(oldest
            .windows(2)
            .all(|w| w[0].published_at <= w[1].published_at));
    }

    #[test]
    fn every_listed_article_can_be_opened() {
        for item in list_articles(&query(View::All, false)) {
            let article = get_article(item.id).expect("article exists");
            assert_eq!(article.title, item.title);
            assert_eq!(article.feed_title, item.feed_title);
        }
        assert!(get_article(-1).is_none());
    }

    #[test]
    fn excerpts_are_short_and_end_on_a_word() {
        let long = "word ".repeat(100);
        let short = excerpt(&long, 140);
        assert!(short.chars().count() <= 140);
        assert!(short.ends_with("word…"));
        assert_eq!(excerpt("short text", 140), "short text");
    }
}
