//! Turning raw feed bytes into UTF-8 text (SPEC §7.3), and repairing common XML breakage.

use encoding_rs::{Encoding, UTF_8};

use crate::content::named_entity;

/// Decodes `bytes` to UTF-8. The encoding comes from, in order: a byte-order mark, the HTTP
/// `charset`, the XML prolog; otherwise UTF-8 with invalid sequences replaced. The XML
/// declaration is removed from the result so the parser doesn't re-decode it.
pub fn decode(bytes: &[u8], content_type: Option<&str>) -> String {
    let (encoding, body) = match Encoding::for_bom(bytes) {
        Some((encoding, bom_len)) => (encoding, &bytes[bom_len..]),
        None => {
            let declared = content_type
                .and_then(charset_param)
                .or_else(|| prolog_encoding(bytes))
                .and_then(|label| Encoding::for_label(label.as_bytes()));
            (declared.unwrap_or(UTF_8), bytes)
        }
    };
    // UTF-16 labels in an ASCII-compatible document are wrong by definition: ignore them.
    let encoding = if !encoding.is_ascii_compatible() && Encoding::for_bom(bytes).is_none() {
        UTF_8
    } else {
        encoding
    };
    let (text, _) = encoding.decode_without_bom_handling(body);
    strip_xml_declaration(&text).to_string()
}

fn charset_param(content_type: &str) -> Option<String> {
    content_type.split(';').skip(1).find_map(|param| {
        let (key, value) = param.split_once('=')?;
        key.trim()
            .eq_ignore_ascii_case("charset")
            .then(|| value.trim().trim_matches(['"', '\'']).to_string())
            .filter(|v| !v.is_empty())
    })
}

fn prolog_encoding(bytes: &[u8]) -> Option<String> {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]);
    let head = head.trim_start();
    if !head.starts_with("<?xml") {
        return None;
    }
    let decl = &head[..head.find("?>")?];
    let rest = &decl[decl.find("encoding")? + "encoding".len()..];
    let rest = rest.trim_start().strip_prefix('=')?.trim_start();
    let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let value = &rest[1..];
    Some(value[..value.find(quote)?].to_string())
}

fn strip_xml_declaration(text: &str) -> &str {
    let trimmed = text.trim_start_matches(['\u{feff}', ' ', '\t', '\r', '\n']);
    if trimmed.starts_with("<?xml") {
        if let Some(end) = trimmed.find("?>") {
            return &trimmed[end + 2..];
        }
    }
    trimmed
}

/// Best-effort fixes for malformed XML that browsers-era feeds commonly contain:
/// unescaped `&`, HTML named entities that XML doesn't define, and `<rss>` without a version.
/// Text inside CDATA sections is left untouched.
pub fn repair(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len() + 64);
    let mut rest = xml;
    loop {
        match rest.find("<![CDATA[") {
            Some(start) => {
                out.push_str(&repair_text(&rest[..start]));
                let cdata = &rest[start..];
                match cdata.find("]]>") {
                    Some(end) => {
                        out.push_str(&cdata[..end + 3]);
                        rest = &cdata[end + 3..];
                    }
                    None => {
                        out.push_str(cdata);
                        break;
                    }
                }
            }
            None => {
                out.push_str(&repair_text(rest));
                break;
            }
        }
    }
    add_rss_version(&out)
}

fn repair_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + 1..];
        let reference = after
            .find(';')
            .filter(|&end| end <= 10)
            .map(|end| &after[..end]);
        match reference {
            Some(name) if is_xml_reference(name) => out.push('&'),
            Some(name) => match named_entity(name) {
                Some(ch) => {
                    out.push_str(&format!("&#{};", ch as u32));
                    rest = &after[name.len() + 1..];
                    continue;
                }
                None => out.push_str("&amp;"),
            },
            None => out.push_str("&amp;"),
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

fn is_xml_reference(name: &str) -> bool {
    matches!(name, "amp" | "lt" | "gt" | "quot" | "apos")
        || name
            .strip_prefix("#x")
            .or_else(|| name.strip_prefix("#X"))
            .is_some_and(|hex| !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit()))
        || name
            .strip_prefix('#')
            .is_some_and(|dec| !dec.is_empty() && dec.chars().all(|c| c.is_ascii_digit()))
}

fn add_rss_version(xml: &str) -> String {
    for open in ["<rss>", "<rss "] {
        if let Some(pos) = xml.find(open) {
            let tag_end = xml[pos..].find('>').map_or(xml.len(), |e| pos + e);
            if !xml[pos..tag_end].contains("version") {
                return format!("{}<rss version=\"2.0\"{}", &xml[..pos], &xml[pos + 4..]);
            }
        }
    }
    xml.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honours_bom_then_http_charset_then_prolog() {
        let latin1 = b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><t>caf\xe9</t>";
        assert_eq!(decode(latin1, None), "<t>café</t>");
        // The HTTP charset wins over the prolog.
        let utf8 = "<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><t>café</t>".as_bytes();
        assert_eq!(
            decode(utf8, Some("text/xml; charset=\"utf-8\"")),
            "<t>café</t>"
        );
        // A BOM wins over everything.
        let mut bom = vec![0xEF, 0xBB, 0xBF];
        bom.extend_from_slice("<t>café</t>".as_bytes());
        assert_eq!(
            decode(&bom, Some("text/xml; charset=iso-8859-1")),
            "<t>café</t>"
        );
    }

    #[test]
    fn decodes_utf16_with_bom() {
        let mut bytes = vec![0xFF, 0xFE];
        for unit in "<?xml version=\"1.0\" encoding=\"UTF-16\"?><t>ü</t>".encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        assert_eq!(decode(&bytes, None), "<t>ü</t>");
    }

    #[test]
    fn windows_1252_and_invalid_utf8_fall_back_gracefully() {
        assert_eq!(
            decode(
                b"<t>\x93quoted\x94</t>",
                Some("text/xml; charset=windows-1252")
            ),
            "<t>\u{201c}quoted\u{201d}</t>"
        );
        assert_eq!(
            decode(b"<t>bad \xff byte</t>", None),
            "<t>bad \u{fffd} byte</t>"
        );
    }

    #[test]
    fn repairs_ampersands_and_html_entities_outside_cdata() {
        let xml = "<rss><channel><title>A & B &mdash; C &amp; D &#38; &#x26;</title>\
                   <description><![CDATA[<p>a & b</p>]]></description></channel></rss>";
        assert_eq!(
            repair(xml),
            "<rss version=\"2.0\"><channel><title>A &amp; B &#8212; C &amp; D &#38; &#x26;</title>\
             <description><![CDATA[<p>a & b</p>]]></description></channel></rss>"
        );
    }

    #[test]
    fn leaves_versioned_rss_alone() {
        let xml = "<rss version=\"0.91\"><channel/></rss>";
        assert_eq!(repair(xml), xml);
    }
}
