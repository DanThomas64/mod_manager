use std::time::{SystemTime, UNIX_EPOCH};

/// Current UTC time as `YYYYMMDD-HHMMSS` — lexically sortable and
/// dependency-free, so the shipped installer binary doesn't need a date/time
/// crate just to name backup folders.
pub fn now_stamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_stamp(secs)
}

fn format_stamp(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let time_of_day = secs % 86400;
    let (y, m, d) = civil_from_days(days);
    let (h, mi, s) = (time_of_day / 3600, (time_of_day % 3600) / 60, time_of_day % 60);
    format!("{y:04}{m:02}{d:02}-{h:02}{mi:02}{s:02}")
}

/// Howard Hinnant's `civil_from_days`: days since the Unix epoch -> (year, month, day).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_epoch_seconds_format_correctly() {
        // 2024-01-15 12:34:56 UTC
        assert_eq!(format_stamp(1705322096), "20240115-123456");
        // Unix epoch itself
        assert_eq!(format_stamp(0), "19700101-000000");
    }
}
