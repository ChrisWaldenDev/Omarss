//! Lenient date parsing (SPEC §7.3): RFC 3339/ISO 8601, RFC 2822/822, and the broken variants
//! feeds actually contain. Dates without a time zone are taken as UTC.

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};

const NAIVE_FORMATS: [&str; 5] = [
    "%Y-%m-%dT%H:%M:%S%.f",
    "%Y-%m-%dT%H:%M:%S",
    "%Y-%m-%d %H:%M:%S",
    "%Y-%m-%dT%H:%M",
    "%Y-%m-%d %H:%M",
];

pub fn parse_date(text: &str) -> Option<DateTime<Utc>> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(text) {
        return Some(dt.to_utc());
    }
    if let Ok(dt) = DateTime::parse_from_rfc2822(text) {
        return Some(dt.to_utc());
    }
    if let Some(dt) = parse_rfc2822_lenient(text) {
        return Some(dt);
    }
    // ISO 8601 with an offset written without a colon (+0200) or a space before it.
    for format in [
        "%Y-%m-%dT%H:%M:%S%z",
        "%Y-%m-%d %H:%M:%S%z",
        "%Y-%m-%d %H:%M:%S %z",
    ] {
        if let Ok(dt) = DateTime::parse_from_str(text, format) {
            return Some(dt.to_utc());
        }
    }
    let without_z = text.trim_end_matches(['Z', 'z']);
    for format in NAIVE_FORMATS {
        if let Ok(naive) = NaiveDateTime::parse_from_str(without_z, format) {
            return Some(naive.and_utc());
        }
    }
    NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|naive| naive.and_utc())
}

/// RFC 2822 with common mistakes fixed: full or odd month names ("Sept", "June"), wrong or
/// full weekday names, "UTC" as a zone, and missing zones.
fn parse_rfc2822_lenient(text: &str) -> Option<DateTime<Utc>> {
    // Drop any leading weekday: it's often wrong and chrono rejects a mismatch.
    let without_weekday = match text.split_once(',') {
        Some((head, rest)) if head.chars().all(|c| c.is_ascii_alphabetic()) => rest.trim(),
        _ => text,
    };
    let mut words: Vec<String> = without_weekday
        .split_whitespace()
        .map(str::to_string)
        .collect();
    if words.len() < 4 {
        return None;
    }
    for word in &mut words {
        if let Some(month) = month_abbreviation(word) {
            *word = month.to_string();
        } else if matches!(word.as_str(), "UTC" | "Z") {
            *word = "+0000".to_string();
        }
    }
    let fixed = words.join(" ");
    DateTime::parse_from_rfc2822(&fixed)
        .or_else(|_| DateTime::parse_from_rfc2822(&format!("{fixed} +0000")))
        .ok()
        .map(|dt| dt.to_utc())
}

fn month_abbreviation(word: &str) -> Option<&'static str> {
    let lower = word.trim_end_matches('.').to_ascii_lowercase();
    const MONTHS: [&str; 12] = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    const NAMES: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    if lower.len() < 3 || !lower.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let index = MONTHS.iter().position(|m| lower.starts_with(m))?;
    // "sept", "june", "january"… but not unrelated words that merely start alike.
    let full = [
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ][index];
    (full.starts_with(&lower) || lower == "sept").then_some(NAMES[index])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> Option<String> {
        parse_date(text).map(|d| d.to_rfc3339())
    }

    #[test]
    fn standard_formats() {
        assert_eq!(
            at("2024-09-06T10:00:00Z").unwrap(),
            "2024-09-06T10:00:00+00:00"
        );
        assert_eq!(
            at("2024-09-06T12:00:00+02:00").unwrap(),
            "2024-09-06T10:00:00+00:00"
        );
        assert_eq!(
            at("Fri, 06 Sep 2024 10:00:00 GMT").unwrap(),
            "2024-09-06T10:00:00+00:00"
        );
        assert_eq!(
            at("Fri, 06 Sep 2024 05:00:00 EST").unwrap(),
            "2024-09-06T10:00:00+00:00"
        );
    }

    #[test]
    fn broken_variants() {
        let expected = "2024-09-06T10:00:00+00:00";
        for text in [
            "06 Sep 2024 10:00:00 +0000",
            "Mon, 06 Sep 2024 10:00:00 +0000",
            "Friday, 06 Sep 2024 10:00:00 GMT",
            "Fri, 06 Sept 2024 10:00:00 GMT",
            "Fri, 06 September 2024 10:00:00 UTC",
            "Fri, 6 Sep 2024 10:00:00",
            "2024-09-06T10:00:00",
            "2024-09-06 10:00:00",
            "2024-09-06T10:00:00.000",
            "2024-09-06T12:00:00+0200",
        ] {
            assert_eq!(at(text).as_deref(), Some(expected), "{text}");
        }
        assert_eq!(at("2024-09-06").unwrap(), "2024-09-06T00:00:00+00:00");
    }

    #[test]
    fn garbage_is_rejected() {
        for text in [
            "",
            "sometime last week",
            "yesterday",
            "13/13/2024",
            "Mayday 1 2 3",
        ] {
            assert_eq!(at(text), None, "{text}");
        }
    }
}
