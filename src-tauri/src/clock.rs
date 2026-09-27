//! Time helpers. All stored timestamps are UTC Unix seconds (SPEC §5).

use chrono::{Duration, Local, TimeZone};

pub fn now_unix() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Unix timestamp of the most recent local midnight (start of "Today").
pub fn local_day_start() -> i64 {
    let now = Local::now();
    let midnight = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("midnight is valid");
    match Local.from_local_datetime(&midnight).earliest() {
        Some(start) => start.timestamp(),
        // Midnight skipped by a DST change: fall back to 24 hours ago.
        None => (now - Duration::days(1)).timestamp(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_start_is_within_the_last_day() {
        let now = now_unix();
        let start = local_day_start();
        assert!(start <= now);
        assert!(now - start <= 25 * 3600, "DST days can be 25h long");
    }
}
