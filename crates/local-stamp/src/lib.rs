//! `local-stamp`: `2026-10-27T04:40:38Z` → `27.10.2026 09:40` in the machine's time zone. No dependencies; the only OS call
//! (the zone's offset) is in [`offset_secs`]. The web build and unknown systems show UTC.
#![deny(unsafe_code)]

/// Days since 1970-01-01 of a civil date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// Unix seconds of `YYYY-MM-DDTHH:MM:SS` (the `Z` and fractions are ignored); `None` for anything else.
fn parse_utc(iso: &str) -> Option<i64> {
    let n = |r: std::ops::Range<usize>| iso.get(r)?.parse::<i64>().ok();
    let b = iso.as_bytes();
    if b.len() < 19
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let (mo, d, h, mi, s) = (n(5..7)?, n(8..10)?, n(11..13)?, n(14..16)?, n(17..19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 60 {
        return None;
    }
    Some(days_from_civil(n(0..4)?, mo, d) * 86_400 + h * 3600 + mi * 60 + s)
}

/// Seconds east of UTC in the machine's zone (as of now: a past date in the other half of the year is off by the DST step).
#[allow(unsafe_code)]
fn offset_secs(_unix: i64) -> i64 {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct Tzi {
            bias: i32,
            _std_name: [u16; 32],
            _std_date: [u16; 8],
            std_bias: i32,
            _dst_name: [u16; 32],
            _dst_date: [u16; 8],
            dst_bias: i32,
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetTimeZoneInformation(info: *mut Tzi) -> u32;
        }
        let mut t = Tzi {
            bias: 0,
            _std_name: [0; 32],
            _std_date: [0; 8],
            std_bias: 0,
            _dst_name: [0; 32],
            _dst_date: [0; 8],
            dst_bias: 0,
        };
        // SAFETY: `Tzi` matches TIME_ZONE_INFORMATION (172 bytes); the call only writes into it.
        let id = unsafe { GetTimeZoneInformation(&mut t) };
        let extra = match id {
            1 => t.std_bias,
            2 => t.dst_bias,
            _ => 0,
        };
        return -i64::from(t.bias + extra) * 60;
    }
    #[cfg(all(unix, not(target_arch = "wasm32")))]
    {
        #[repr(C)]
        struct Tm {
            fields: [i32; 9],
            gmtoff: std::ffi::c_long,
            _zone: *const std::ffi::c_char,
        }
        unsafe extern "C" {
            fn localtime_r(t: *const i64, out: *mut Tm) -> *mut Tm;
        }
        let mut tm = Tm {
            fields: [0; 9],
            gmtoff: 0,
            _zone: std::ptr::null(),
        };
        // SAFETY: `Tm` is glibc/musl/macOS `struct tm` (nine ints, tm_gmtoff, tm_zone); localtime_r fills only it.
        let ok = unsafe { !localtime_r(&_unix, &mut tm).is_null() };
        return if ok { tm.gmtoff as i64 } else { 0 };
    }
    #[allow(unreachable_code)]
    0
}

/// `dd.mm.yyyy HH:MM` in local time; the input unchanged when it is not an ISO UTC time.
pub fn local_stamp(iso: &str) -> String {
    let Some(u) = parse_utc(iso) else {
        return iso.to_string();
    };
    stamp_at(u + offset_secs(u))
}

fn stamp_at(local: i64) -> String {
    let (y, m, d) = civil_from_days(local.div_euclid(86_400));
    let s = local.rem_euclid(86_400);
    format!("{d:02}.{m:02}.{y:04} {:02}:{:02}", s / 3600, s % 3600 / 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_round_trips_and_formats() {
        let u = parse_utc("2026-10-27T04:40:38Z").unwrap();
        assert_eq!(u, 1_793_076_038);
        assert_eq!(stamp_at(u), "27.10.2026 04:40");
        assert_eq!(stamp_at(u + 5 * 3600), "27.10.2026 09:40");
        assert_eq!(
            stamp_at(parse_utc("2024-02-29T23:59:59Z").unwrap() + 60),
            "01.03.2024 00:00"
        );
    }

    #[test]
    fn junk_is_returned_unchanged() {
        assert_eq!(local_stamp("yesterday"), "yesterday");
        assert_eq!(local_stamp("2026-13-01T00:00:00Z"), "2026-13-01T00:00:00Z");
    }

    #[test]
    fn local_time_is_within_a_day_of_utc() {
        assert!(offset_secs(1_793_076_038).abs() <= 14 * 3600);
        assert_eq!(local_stamp("2026-10-27T04:40:38Z").len(), 16);
    }
}
