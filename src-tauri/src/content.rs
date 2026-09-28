//! Article HTML handling: sanitising (SPEC §8.3), plain-text excerpts, entity decoding and
//! change hashes.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use ammonia::UrlRelative;
use sha2::{Digest, Sha256};
use url::Url;

mod rewrite;

pub use rewrite::{
    first_image, for_display, is_tracker, strip_tracking_params, DisplayOptions, ImageDisplay,
};

/// Tags kept in article HTML (SPEC §8.3). Everything else is removed; the contents of
/// `script` and `style` are dropped entirely. Iframes survive only as YouTube/Vimeo embeds,
/// which then become click-to-load placeholders (SPEC §8.2).
const ALLOWED_TAGS: &[&str] = &[
    "a",
    "abbr",
    "address",
    "article",
    "aside",
    "audio",
    "b",
    "bdi",
    "bdo",
    "blockquote",
    "br",
    "caption",
    "center",
    "cite",
    "code",
    "col",
    "colgroup",
    "data",
    "dd",
    "del",
    "details",
    "dfn",
    "div",
    "dl",
    "dt",
    "em",
    "figcaption",
    "figure",
    "footer",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hgroup",
    "hr",
    "i",
    "iframe",
    "img",
    "ins",
    "kbd",
    "li",
    "main",
    "mark",
    "nav",
    "ol",
    "p",
    "picture",
    "pre",
    "q",
    "rp",
    "rt",
    "ruby",
    "s",
    "samp",
    "section",
    "small",
    "source",
    "span",
    "strike",
    "strong",
    "sub",
    "summary",
    "sup",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "time",
    "tr",
    "tt",
    "u",
    "ul",
    "var",
    "video",
    "wbr",
];

const TAG_ATTRIBUTES: &[(&str, &[&str])] = &[
    ("a", &["href", "hreflang"]),
    ("img", &["src", "alt", "width", "height", "srcset", "sizes"]),
    ("video", &["src", "controls", "poster", "width", "height"]),
    ("audio", &["src", "controls"]),
    ("source", &["src", "srcset", "type", "media", "sizes"]),
    ("blockquote", &["cite"]),
    ("q", &["cite"]),
    ("del", &["cite", "datetime"]),
    ("ins", &["cite", "datetime"]),
    ("time", &["datetime"]),
    ("data", &["value"]),
    ("td", &["colspan", "rowspan", "headers"]),
    ("th", &["colspan", "rowspan", "headers", "scope", "abbr"]),
    ("col", &["span"]),
    ("colgroup", &["span"]),
    ("ol", &["start", "reversed", "type"]),
    ("li", &["value"]),
    ("details", &["open"]),
    ("iframe", &["src"]),
];

/// Sanitises feed-provided HTML with a strict allowlist and resolves relative URLs against
/// `base`. Links get `rel="noopener noreferrer"`; `javascript:` URLs, event handlers, `style`
/// attributes and every tag outside the allowlist are removed. Video embeds become
/// click-to-load placeholders and tracking pixels are dropped (SPEC §8.2, §8.4).
pub fn sanitize_html(html: &str, base: Option<&Url>) -> String {
    let tag_attributes: HashMap<&str, HashSet<&str>> = TAG_ATTRIBUTES
        .iter()
        .map(|(tag, attrs)| (*tag, attrs.iter().copied().collect()))
        .collect();
    let filter_base = base.cloned();

    let mut builder = ammonia::Builder::default();
    builder
        .tags(ALLOWED_TAGS.iter().copied().collect())
        .tag_attributes(tag_attributes)
        .generic_attributes(["title", "lang", "dir"].into_iter().collect())
        // `data:` passes the scheme check only so the filter can keep `data:image/*` on images.
        .url_schemes(["http", "https", "mailto", "data"].into_iter().collect())
        .link_rel(Some("noopener noreferrer"))
        .strip_comments(true)
        .attribute_filter(move |element, attribute, value| {
            filter_attribute(element, attribute, value, filter_base.as_ref())
        });
    match base {
        Some(base) => builder.url_relative(UrlRelative::RewriteWithBase(base.clone())),
        None => builder.url_relative(UrlRelative::Deny),
    };
    rewrite::finish_sanitized(&builder.clean(html).to_string())
}

fn filter_attribute<'u>(
    element: &str,
    attribute: &str,
    value: &'u str,
    base: Option<&Url>,
) -> Option<Cow<'u, str>> {
    match attribute {
        "src" if element == "iframe" => rewrite::canonical_embed(value).map(Cow::Owned),
        "src" | "href" | "poster" => {
            let lowered = value.trim_start().to_ascii_lowercase();
            if lowered.starts_with("data:") {
                let image_src = matches!(element, "img" | "source") && attribute == "src";
                return (image_src && lowered.starts_with("data:image/"))
                    .then_some(Cow::Borrowed(value));
            }
            Some(Cow::Borrowed(value))
        }
        "srcset" => resolve_srcset(value, base).map(Cow::Owned),
        "cite" => resolve_web_url(value, base).map(Cow::Owned),
        _ => Some(Cow::Borrowed(value)),
    }
}

/// Resolves every candidate in a `srcset` against `base`, dropping anything that isn't http(s).
fn resolve_srcset(value: &str, base: Option<&Url>) -> Option<String> {
    let candidates: Vec<String> = value
        .split(',')
        .filter_map(|candidate| {
            let mut parts = candidate.split_whitespace();
            let url = resolve_web_url(parts.next()?, base)?;
            let descriptor: Vec<&str> = parts.collect();
            Some(if descriptor.is_empty() {
                url
            } else {
                format!("{url} {}", descriptor.join(" "))
            })
        })
        .collect();
    (!candidates.is_empty()).then(|| candidates.join(", "))
}

/// Resolves `value` against `base` and returns it only if it's an http(s) URL.
pub fn resolve_web_url(value: &str, base: Option<&Url>) -> Option<String> {
    let value = value.trim();
    let url = match base {
        Some(base) => base.join(value).ok()?,
        None => Url::parse(value).ok()?,
    };
    matches!(url.scheme(), "http" | "https").then(|| url.to_string())
}

/// Escapes plain text so it can be shown as HTML (for feeds that send text, not markup).
pub fn text_to_html(text: &str) -> String {
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    escaped
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| format!("<p>{}</p>", p.replace('\n', "<br>")))
        .collect()
}

/// The visible text of an HTML fragment, whitespace-collapsed, cut to at most `max_chars`
/// characters on a word boundary (with an ellipsis). `max_chars = 0` means no limit.
pub fn html_to_text(html: &str, max_chars: usize) -> String {
    use ego_tree::iter::Edge;
    use scraper::Node;

    let fragment = scraper::Html::parse_fragment(html);
    let mut text = String::new();
    // Block-level elements separate words; inline ones ("<b>x</b>.") must not add spaces.
    for edge in fragment.tree.root().traverse() {
        match edge {
            Edge::Open(node) => match node.value() {
                Node::Text(t) => text.push_str(t),
                Node::Element(e) if is_block(e.name()) => text.push(' '),
                _ => {}
            },
            Edge::Close(node) => {
                if let Node::Element(e) = node.value() {
                    if is_block(e.name()) {
                        text.push(' ');
                    }
                }
            }
        }
    }
    let collapsed = collapse_whitespace(&text);
    if max_chars == 0 {
        collapsed
    } else {
        truncate_words(&collapsed, max_chars)
    }
}

fn is_block(tag: &str) -> bool {
    matches!(
        tag,
        "p" | "div"
            | "br"
            | "hr"
            | "li"
            | "ul"
            | "ol"
            | "dl"
            | "dt"
            | "dd"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "pre"
            | "blockquote"
            | "figure"
            | "figcaption"
            | "table"
            | "tr"
            | "td"
            | "th"
            | "section"
            | "article"
            | "header"
            | "footer"
            | "aside"
            | "details"
            | "summary"
            | "img"
    )
}

pub fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Truncates to at most `max` characters on a word boundary, adding an ellipsis when cut.
pub fn truncate_words(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max.saturating_sub(1)).collect();
    let cut = match cut.rsplit_once(' ') {
        Some((head, _)) if !head.is_empty() => head,
        _ => cut.as_str(),
    };
    format!("{}…", cut.trim_end_matches(|c: char| !c.is_alphanumeric()))
}

/// Decodes character references (`&amp;`, `&#8217;`, `&nbsp;`, …) in plain text such as titles.
/// Unknown named references are left as they are.
pub fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let decoded = after.find(';').filter(|&end| end <= 10).and_then(|end| {
            let name = &after[..end];
            decode_reference(name).map(|ch| (ch, end + 1))
        });
        match decoded {
            Some((ch, consumed)) => {
                out.push(ch);
                rest = &after[consumed..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn decode_reference(name: &str) -> Option<char> {
    if let Some(num) = name.strip_prefix('#') {
        let code = match num.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => num.parse().ok()?,
        };
        return char::from_u32(code).filter(|c| *c != '\0');
    }
    named_entity(name)
}

/// The HTML named character references that commonly leak into feed text.
pub fn named_entity(name: &str) -> Option<char> {
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{a0}',
        "ensp" => '\u{2002}',
        "emsp" => '\u{2003}',
        "thinsp" => '\u{2009}',
        "shy" => '\u{ad}',
        "ndash" => '–',
        "mdash" => '—',
        "hellip" => '…',
        "lsquo" => '‘',
        "rsquo" => '’',
        "sbquo" => '‚',
        "ldquo" => '“',
        "rdquo" => '”',
        "bdquo" => '„',
        "laquo" => '«',
        "raquo" => '»',
        "lsaquo" => '‹',
        "rsaquo" => '›',
        "bull" => '•',
        "middot" => '·',
        "copy" => '©',
        "reg" => '®',
        "trade" => '™',
        "deg" => '°',
        "plusmn" => '±',
        "times" => '×',
        "divide" => '÷',
        "euro" => '€',
        "pound" => '£',
        "yen" => '¥',
        "cent" => '¢',
        "sect" => '§',
        "para" => '¶',
        "dagger" => '†',
        "frac12" => '½',
        "frac14" => '¼',
        "frac34" => '¾',
        "iexcl" => '¡',
        "iquest" => '¿',
        "aacute" => 'á',
        "agrave" => 'à',
        "acirc" => 'â',
        "atilde" => 'ã',
        "auml" => 'ä',
        "aring" => 'å',
        "aelig" => 'æ',
        "ccedil" => 'ç',
        "eacute" => 'é',
        "egrave" => 'è',
        "ecirc" => 'ê',
        "euml" => 'ë',
        "iacute" => 'í',
        "igrave" => 'ì',
        "icirc" => 'î',
        "iuml" => 'ï',
        "ntilde" => 'ñ',
        "oacute" => 'ó',
        "ograve" => 'ò',
        "ocirc" => 'ô',
        "otilde" => 'õ',
        "ouml" => 'ö',
        "oslash" => 'ø',
        "uacute" => 'ú',
        "ugrave" => 'ù',
        "ucirc" => 'û',
        "uuml" => 'ü',
        "yacute" => 'ý',
        "yuml" => 'ÿ',
        "szlig" => 'ß',
        "Aacute" => 'Á',
        "Agrave" => 'À',
        "Auml" => 'Ä',
        "Aring" => 'Å',
        "Ccedil" => 'Ç',
        "Eacute" => 'É',
        "Egrave" => 'È',
        "Ntilde" => 'Ñ',
        "Oacute" => 'Ó',
        "Ouml" => 'Ö',
        "Oslash" => 'Ø',
        "Uacute" => 'Ú',
        "Uuml" => 'Ü',
        _ => return None,
    })
}

/// Hash of an article's visible content, used to notice when a feed updates an article.
pub fn content_hash(title: &str, summary_html: Option<&str>, content_html: Option<&str>) -> String {
    let mut hasher = Sha256::new();
    for part in [
        title,
        summary_html.unwrap_or(""),
        content_html.unwrap_or(""),
    ] {
        hasher.update(part.as_bytes());
        hasher.update([0u8]);
    }
    hex(&hasher.finalize())
}

pub fn sha256_hex(input: &str) -> String {
    hex(&Sha256::digest(input.as_bytes()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://blog.example/posts/2024/hello.html").unwrap()
    }

    fn clean(html: &str) -> String {
        sanitize_html(html, Some(&base()))
    }

    #[test]
    fn keeps_allowed_structure() {
        let html = "<h2>Title</h2><p>Some <strong>bold</strong> and <em>em</em>.</p>\
                    <ul><li>one</li></ul><pre><code>let x = 1;</code></pre>\
                    <table><tr><td colspan=\"2\">cell</td></tr></table>\
                    <details open><summary>More</summary>text</details><sup>1</sup>";
        let out = clean(html);
        for tag in [
            "<h2>",
            "<strong>",
            "<em>",
            "<li>",
            "<pre><code>",
            "colspan=\"2\"",
            "<details",
            "<summary>",
            "<sup>",
        ] {
            assert!(out.contains(tag), "{tag} missing from {out}");
        }
    }

    #[test]
    fn resolves_relative_urls_and_marks_links() {
        let out = clean(
            r#"<a href="../other.html">x</a><img src="img/a.png" srcset="img/a.png 1x, /b.png 2x" alt="A">"#,
        );
        assert!(
            out.contains(r#"href="https://blog.example/posts/other.html""#),
            "{out}"
        );
        assert!(out.contains(r#"rel="noopener noreferrer""#), "{out}");
        assert!(
            out.contains(r#"src="https://blog.example/posts/2024/img/a.png""#),
            "{out}"
        );
        assert!(
            out.contains(r#"srcset="https://blog.example/posts/2024/img/a.png 1x, https://blog.example/b.png 2x""#),
            "{out}"
        );
    }

    #[test]
    fn data_urls_only_survive_on_images() {
        let out = clean(
            r#"<img src="data:image/png;base64,AAAA"><a href="data:text/html,<script>alert(1)</script>">x</a>"#,
        );
        assert!(out.contains("data:image/png;base64,AAAA"), "{out}");
        assert!(!out.contains("data:text/html"), "{out}");
    }

    /// A corpus of common XSS vectors (SPEC §13). None may survive sanitising.
    #[test]
    fn xss_corpus_is_neutralised() {
        let corpus = [
            "<script>alert(1)</script>",
            "<SCRIPT SRC=//evil.example/x.js></SCRIPT>",
            "<img src=x onerror=alert(1)>",
            "<IMG SRC=\"javascript:alert('XSS');\">",
            "<img src=\"jav&#x09;ascript:alert(1)\">",
            "<a href=\"javascript:alert(1)\">x</a>",
            "<a href=\"JaVaScRiPt:alert(1)\">x</a>",
            "<a href=\" javascript:alert(1)\">x</a>",
            "<a href=\"vbscript:msgbox(1)\">x</a>",
            "<svg onload=alert(1)><circle r=1></svg>",
            "<svg><script>alert(1)</script></svg>",
            "<math><mtext><table><mglyph><style><img src=x onerror=alert(1)>",
            "<iframe src=\"https://evil.example\"></iframe>",
            "<iframe srcdoc=\"<script>alert(1)</script>\"></iframe>",
            "<object data=\"javascript:alert(1)\"></object>",
            "<embed src=\"javascript:alert(1)\">",
            "<form action=\"javascript:alert(1)\"><input type=submit></form>",
            "<button formaction=\"javascript:alert(1)\">x</button>",
            "<body onload=alert(1)>",
            "<div style=\"background:url(javascript:alert(1))\">x</div>",
            "<p style=\"color:red\" onclick=\"alert(1)\">x</p>",
            "<meta http-equiv=\"refresh\" content=\"0;url=javascript:alert(1)\">",
            "<link rel=stylesheet href=\"javascript:alert(1)\">",
            "<style>@import 'javascript:alert(1)';</style>",
            "<base href=\"javascript:alert(1)//\">",
            "<video poster=\"javascript:alert(1)\"></video>",
            "<audio src=\"javascript:alert(1)\"></audio>",
            "<details open ontoggle=alert(1)>",
            "<a href=\"data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==\">x</a>",
            "<!--<img src=x onerror=alert(1)>-->",
            "<noscript><p title=\"</noscript><img src=x onerror=alert(1)>\"></noscript>",
            "<img src=x:alert(1) onerror=eval(src)>",
            "<a href=\"&#106;avascript:alert(1)\">x</a>",
            "<table background=\"javascript:alert(1)\">",
        ];
        for payload in corpus {
            let out = clean(payload).to_ascii_lowercase();
            for bad in [
                "<script",
                "javascript:",
                "vbscript:",
                "onerror",
                "onload",
                "onclick",
                "ontoggle",
                "<iframe",
                "<object",
                "<embed",
                "<form",
                "<style",
                "<meta",
                "<link",
                "<base",
                "style=",
                "srcdoc",
                "data:text",
            ] {
                assert!(
                    !out.contains(bad),
                    "{bad:?} survived in {out:?} (from {payload:?})"
                );
            }
        }
    }

    #[test]
    fn video_embeds_become_click_to_load_placeholders() {
        let out = clean(
            r#"<p>Watch:</p><iframe width="560" src="https://www.youtube.com/embed/dQw4w9WgXcQ?autoplay=1" onload="alert(1)" allowfullscreen></iframe><iframe src="https://ads.example/frame"></iframe>"#,
        );
        assert_eq!(
            out,
            "<p>Watch:</p><figure class=\"omarss-embed\" data-embed=\"https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ\"><a href=\"https://www.youtube.com/watch?v=dQw4w9WgXcQ\" rel=\"noopener noreferrer\">YouTube video</a></figure>"
        );
    }

    #[test]
    fn tracking_pixels_are_dropped_at_ingest() {
        let out = clean(
            r#"<p>Hi</p><img src="https://feeds.feedburner.com/~r/x/~4/y" height="1" width="1" alt=""><img src="/photo.jpg" alt="Photo">"#,
        );
        assert_eq!(
            out,
            r#"<p>Hi</p><img src="https://blog.example/photo.jpg" alt="Photo">"#
        );
    }

    #[test]
    fn text_is_extracted_and_truncated() {
        assert_eq!(
            html_to_text("<p>Hello&nbsp;<b>world</b></p>\n<p>again</p>", 0),
            "Hello world again"
        );
        assert_eq!(
            html_to_text(
                "<p>Has <strong>formatting</strong>.</p><ul><li>one</li><li>two</li></ul>x<br>y",
                0
            ),
            "Has formatting. one two x y"
        );
        let long = format!("<p>{}</p>", "word ".repeat(60));
        let short = html_to_text(&long, 140);
        assert!(short.chars().count() <= 140);
        assert!(short.ends_with("word…"));
    }

    #[test]
    fn plain_text_becomes_paragraphs() {
        assert_eq!(
            text_to_html("a < b\n\nsecond\nline"),
            "<p>a &lt; b</p><p>second<br>line</p>"
        );
    }

    #[test]
    fn decodes_entities_in_titles() {
        assert_eq!(
            decode_entities("Tom &amp; Jerry&nbsp;&#8212; &#x2019;s &mdash; ok"),
            "Tom & Jerry\u{a0}— ’s — ok"
        );
        assert_eq!(
            decode_entities("AT&T &unknown; & more"),
            "AT&T &unknown; & more"
        );
        assert_eq!(decode_entities("Vec<T>"), "Vec<T>");
    }

    #[test]
    fn content_hash_changes_with_content() {
        let a = content_hash("t", Some("s"), Some("c"));
        assert_eq!(a, content_hash("t", Some("s"), Some("c")));
        assert_ne!(a, content_hash("t", Some("s"), Some("c2")));
        assert_ne!(
            content_hash("ab", None, Some("c")),
            content_hash("a", Some("b"), Some("c"))
        );
        assert_eq!(a.len(), 64);
    }
}
