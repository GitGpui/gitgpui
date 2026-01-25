#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DateTimeFormat {
    YmdHm,
    YmdHms,
    DmyHm,
    MdyHm,
}

impl DateTimeFormat {
    pub(super) fn all() -> &'static [DateTimeFormat] {
        &[
            DateTimeFormat::YmdHm,
            DateTimeFormat::YmdHms,
            DateTimeFormat::DmyHm,
            DateTimeFormat::MdyHm,
        ]
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            DateTimeFormat::YmdHm => "YYYY-MM-DD HH:MM (UTC)",
            DateTimeFormat::YmdHms => "YYYY-MM-DD HH:MM:SS (UTC)",
            DateTimeFormat::DmyHm => "DD.MM.YYYY HH:MM (UTC)",
            DateTimeFormat::MdyHm => "MM/DD/YYYY HH:MM (UTC)",
        }
    }

    pub(super) fn key(self) -> &'static str {
        match self {
            DateTimeFormat::YmdHm => "ymd_hm_utc",
            DateTimeFormat::YmdHms => "ymd_hms_utc",
            DateTimeFormat::DmyHm => "dmy_hm_utc",
            DateTimeFormat::MdyHm => "mdy_hm_utc",
        }
    }

    pub(super) fn from_key(s: &str) -> Option<Self> {
        match s {
            "ymd_hm_utc" => Some(DateTimeFormat::YmdHm),
            "ymd_hms_utc" => Some(DateTimeFormat::YmdHms),
            "dmy_hm_utc" => Some(DateTimeFormat::DmyHm),
            "mdy_hm_utc" => Some(DateTimeFormat::MdyHm),
            _ => None,
        }
    }
}

pub(super) fn format_datetime_utc(time: std::time::SystemTime, format: DateTimeFormat) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unix_seconds(t: SystemTime) -> i64 {
        match t.duration_since(UNIX_EPOCH) {
            Ok(d) => d.as_secs() as i64,
            Err(e) => -(e.duration().as_secs() as i64),
        }
    }

    fn floor_div(a: i64, b: i64) -> i64 {
        let mut q = a / b;
        let r = a % b;
        if (r != 0) && ((r < 0) != (b < 0)) {
            q -= 1;
        }
        q
    }

    // Howard Hinnant's `civil_from_days` algorithm.
    fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
        let z = days_since_epoch + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = z - era * 146_097; // [0, 146096]
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
        let mp = (5 * doy + 2) / 153; // [0, 11]
        let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
        let m = mp + if mp < 10 { 3 } else { -9 }; // [1, 12]
        let y = y + i64::from(m <= 2);
        (y as i32, m as u32, d as u32)
    }

    let secs = unix_seconds(time);
    let days = floor_div(secs, 86_400);
    let sec_of_day = secs - days * 86_400;
    let sec_of_day: i64 = if sec_of_day < 0 {
        sec_of_day + 86_400
    } else {
        sec_of_day
    };

    let hour = (sec_of_day / 3600) as u32;
    let minute = ((sec_of_day % 3600) / 60) as u32;
    let second = (sec_of_day % 60) as u32;

    let (y, m, d) = civil_from_days(days);

    match format {
        DateTimeFormat::YmdHm => format!("{y:04}-{m:02}-{d:02} {hour:02}:{minute:02}"),
        DateTimeFormat::YmdHms => {
            format!("{y:04}-{m:02}-{d:02} {hour:02}:{minute:02}:{second:02}")
        }
        DateTimeFormat::DmyHm => format!("{d:02}.{m:02}.{y:04} {hour:02}:{minute:02}"),
        DateTimeFormat::MdyHm => format!("{m:02}/{d:02}/{y:04} {hour:02}:{minute:02}"),
    }
}
