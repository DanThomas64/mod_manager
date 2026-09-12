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

/// Howard Hinnant's `days_from_civil`: the inverse of `civil_from_days`,
/// (year, month, day) -> days since the Unix epoch.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let mp = if m > 2 { m - 3 } else { m + 9 } as u64;
    let doy = (153 * mp + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe as i64 - 719468
}

fn parse_stamp(stamp: &str) -> Option<(i64, u32, u32, u32, u32, u32)> {
    let (date, time) = stamp.split_once('-')?;
    if date.len() != 8 || time.len() != 6 {
        return None;
    }
    Some((
        date[0..4].parse().ok()?,
        date[4..6].parse().ok()?,
        date[6..8].parse().ok()?,
        time[0..2].parse().ok()?,
        time[2..4].parse().ok()?,
        time[4..6].parse().ok()?,
    ))
}

/// Reformat a `YYYYMMDD-HHMMSS` UTC stamp (as produced by `now_stamp`) into a
/// human-readable `YYYY-MM-DD HH:MM:SS` string in the *local* time zone, so
/// what's shown matches the clock on the wall rather than UTC. Falls back to
/// the raw stamp if it doesn't parse (e.g. a backup folder renamed by hand).
pub fn to_display(stamp: &str) -> String {
    let Some((y, m, d, h, mi, s)) = parse_stamp(stamp) else {
        return stamp.to_string();
    };
    let epoch = days_from_civil(y, m, d) * 86400 + h as i64 * 3600 + mi as i64 * 60 + s as i64;
    let offset = local_offset_seconds(epoch);
    format_local(y, m, d, h, mi, s, offset)
}

/// Apply a UTC offset (in seconds, positive east of UTC) to a UTC civil time
/// and format the result. Split out from `to_display` so the date-rolling
/// math is unit-testable without depending on the host machine's actual time
/// zone.
fn format_local(y: i64, m: u32, d: u32, h: u32, mi: u32, s: u32, offset_secs: i64) -> String {
    let epoch = days_from_civil(y, m, d) * 86400 + h as i64 * 3600 + mi as i64 * 60 + s as i64;
    let local_epoch = epoch + offset_secs;
    let local_days = local_epoch.div_euclid(86400);
    let time_of_day = local_epoch.rem_euclid(86400);
    let (ly, lm, ld) = civil_from_days(local_days);
    format!(
        "{ly:04}-{lm:02}-{ld:02} {:02}:{:02}:{:02}",
        time_of_day / 3600,
        (time_of_day % 3600) / 60,
        time_of_day % 60
    )
}

/// The local time zone's offset from UTC (in seconds, positive east of UTC)
/// at the given moment — accounts for DST since a backup's timestamp could
/// have been taken on either side of a transition.
#[cfg(unix)]
fn local_offset_seconds(epoch: i64) -> i64 {
    unsafe {
        let t = epoch as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&t, &mut tm).is_null() {
            return 0;
        }
        tm.tm_gmtoff
    }
}

#[cfg(windows)]
fn local_offset_seconds(epoch: i64) -> i64 {
    #[repr(C)]
    struct SystemTime {
        w_year: u16,
        w_month: u16,
        w_day_of_week: u16,
        w_day: u16,
        w_hour: u16,
        w_minute: u16,
        w_second: u16,
        w_milliseconds: u16,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SystemTimeToTzSpecificLocalTime(
            lp_time_zone_information: *const core::ffi::c_void,
            lp_universal_time: *const SystemTime,
            lp_local_time: *mut SystemTime,
        ) -> i32;
    }

    let days = epoch.div_euclid(86400);
    let time_of_day = epoch.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    let utc = SystemTime {
        w_year: y as u16,
        w_month: m as u16,
        w_day_of_week: 0,
        w_day: d as u16,
        w_hour: (time_of_day / 3600) as u16,
        w_minute: ((time_of_day % 3600) / 60) as u16,
        w_second: (time_of_day % 60) as u16,
        w_milliseconds: 0,
    };
    let mut local = SystemTime {
        w_year: 0,
        w_month: 0,
        w_day_of_week: 0,
        w_day: 0,
        w_hour: 0,
        w_minute: 0,
        w_second: 0,
        w_milliseconds: 0,
    };

    let ok = unsafe { SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local) };
    if ok == 0 {
        return 0;
    }

    let local_days = days_from_civil(local.w_year as i64, local.w_month as u32, local.w_day as u32);
    let local_epoch =
        local_days * 86400 + local.w_hour as i64 * 3600 + local.w_minute as i64 * 60 + local.w_second as i64;
    local_epoch - epoch
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

    #[test]
    fn days_from_civil_round_trips_with_civil_from_days() {
        for days in [-100000i64, -1, 0, 1, 10000, 19858] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
    }

    #[test]
    fn format_local_zero_offset_matches_utc() {
        assert_eq!(format_local(2024, 1, 15, 12, 34, 56, 0), "2024-01-15 12:34:56");
    }

    #[test]
    fn format_local_positive_offset_rolls_into_next_day() {
        // 2024-01-15 23:30:00 UTC, UTC+2 -> 2024-01-16 01:30:00 local
        assert_eq!(format_local(2024, 1, 15, 23, 30, 0, 2 * 3600), "2024-01-16 01:30:00");
    }

    #[test]
    fn format_local_negative_offset_rolls_into_previous_day() {
        // 2024-01-15 00:30:00 UTC, UTC-2 -> 2024-01-14 22:30:00 local
        assert_eq!(format_local(2024, 1, 15, 0, 30, 0, -2 * 3600), "2024-01-14 22:30:00");
    }

    #[test]
    fn to_display_falls_back_on_bad_input() {
        assert_eq!(to_display("not-a-stamp"), "not-a-stamp");
        assert_eq!(to_display("garbage"), "garbage");
    }
}
