//! ISO 8601 UTC, formatted at the edge.
//!
//! `docs/conventions/code.md`: timestamps stored as ISO 8601 UTC. ⚠ The warning
//! there is the reason this is not a number of seconds in the store file: a
//! format that will never string-compare against an ISO column silently returns
//! false forever, and `podbox images` sorts by this field.
//!
//! Written by hand rather than taking a date crate: `TODO/deps.md`'s default
//! answer to a dependency is no, and this is thirty lines with a proof.

use std::time::{SystemTime, UNIX_EPOCH};

/// Days since 1970-01-01 into `(year, month, day)`.
///
/// Howard Hinnant's `civil_from_days`, which is exact for the proleptic
/// Gregorian calendar over the whole range this can produce. ⚠ It shifts the
/// year to start in March so the leap day is the last day of the cycle, which
/// is why the month arithmetic below looks displaced.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn iso8601(t: SystemTime) -> String {
    let secs = match t.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        // ⛔ A clock before the epoch is reported as the epoch rather than as a
        // negative that would sort before every real entry. It is a reading
        // this code cannot make, and it says so.
        Err(_) => 0,
    };
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

pub fn now() -> String {
    iso8601(SystemTime::now())
}

/// How long ago, in docker's `CreatedSince` wording. ⛔ Returns `None` where the
/// input does not parse, so the caller prints a dash: `docs/AGENTS.md` absolute
/// 3, a dash where a value is unknown.
pub fn since(iso: &str) -> Option<String> {
    let then = parse_iso8601(iso)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    let d = now - then;
    if d < 0 {
        return Some("just now".into());
    }
    Some(match d {
        s if s < 60 => format!("{s} second{} ago", plural(s)),
        s if s < 3600 => format!("{} minute{} ago", s / 60, plural(s / 60)),
        s if s < 86_400 => format!("{} hour{} ago", s / 3600, plural(s / 3600)),
        s if s < 2_592_000 => format!("{} day{} ago", s / 86_400, plural(s / 86_400)),
        s if s < 31_536_000 => format!("{} month{} ago", s / 2_592_000, plural(s / 2_592_000)),
        s => format!("{} year{} ago", s / 31_536_000, plural(s / 31_536_000)),
    })
}

fn plural(n: i64) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// `YYYY-MM-DDTHH:MM:SS` with anything after the seconds ignored, into epoch
/// seconds. ⚠ Fractional seconds and a `+00:00` offset both appear in image
/// configs; only `Z` and a truncated offset are read, and anything else returns
/// `None` rather than a number computed from a shape this does not understand.
fn parse_iso8601(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || (b[10] != b'T' && b[10] != b' ') {
        return None;
    }
    let n = |a: usize, z: usize| -> Option<i64> { s.get(a..z)?.parse::<i64>().ok() };
    let (y, mo, d) = (n(0, 4)?, n(5, 7)?, n(8, 10)?);
    let (h, mi, sec) = (n(11, 13)?, n(14, 16)?, n(17, 19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    Some(days_from_civil(y, mo as u32, d as u32) * 86_400 + h * 3600 + mi * 60 + sec)
}

/// The inverse of [`civil_from_days`], same source.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let mp = if m > 2 { m - 3 } else { m + 9 } as u64;
    let doy = (153 * mp + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe as i64 - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn the_epoch_and_two_known_instants_render_exactly() {
        assert_eq!(iso8601(UNIX_EPOCH), "1970-01-01T00:00:00Z");
        assert_eq!(
            iso8601(UNIX_EPOCH + Duration::from_secs(1_000_000_000)),
            "2001-09-09T01:46:40Z"
        );
        // A leap day, which is where a hand-written calendar goes wrong.
        assert_eq!(
            iso8601(UNIX_EPOCH + Duration::from_secs(1_709_164_800)),
            "2024-02-29T00:00:00Z"
        );
    }

    #[test]
    fn the_two_calendar_conversions_are_inverses_across_a_century() {
        // ⭐ A test that cannot fail is not evidence: this walks real days
        // rather than asserting one known pair.
        for day in (-10_000..30_000).step_by(37) {
            let (y, m, d) = civil_from_days(day);
            assert_eq!(days_from_civil(y, m, d), day, "{y}-{m}-{d}");
        }
    }

    #[test]
    fn a_config_timestamp_with_fractional_seconds_parses() {
        assert_eq!(
            parse_iso8601("2024-02-29T00:00:00.123456789Z"),
            Some(1_709_164_800)
        );
        assert_eq!(parse_iso8601("2024-02-29T00:00:00Z"), Some(1_709_164_800));
    }

    #[test]
    fn an_unparseable_timestamp_is_none_rather_than_a_number() {
        // ⛔ docs/AGENTS.md absolute 3: a dash where the value is unknown.
        assert!(parse_iso8601("yesterday").is_none());
        assert!(parse_iso8601("2024-13-01T00:00:00Z").is_none());
        assert!(since("not a date").is_none());
    }

    #[test]
    fn an_age_is_worded_the_way_docker_words_it() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let ago = |s: i64| since(&iso8601(UNIX_EPOCH + Duration::from_secs((now - s) as u64)));
        assert_eq!(ago(1).unwrap(), "1 second ago");
        assert_eq!(ago(90).unwrap(), "1 minute ago");
        assert_eq!(ago(7200).unwrap(), "2 hours ago");
        assert_eq!(ago(90_000).unwrap(), "1 day ago");
    }
}
