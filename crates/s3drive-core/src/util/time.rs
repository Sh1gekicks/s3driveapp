//! 日時の変換。IPC では RFC 3339（UTC）の文字列で受け渡す（05 §2）。

use chrono::{DateTime, Datelike, Local, NaiveDate, SecondsFormat, TimeZone, Utc};

pub fn now() -> DateTime<Utc> {
    Utc::now()
}

pub fn to_rfc3339(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn now_rfc3339() -> String {
    to_rfc3339(now())
}

pub fn parse_rfc3339(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

/// AWS SDK の日時を変換する。
pub fn from_aws(dt: &aws_smithy_types::DateTime) -> DateTime<Utc> {
    Utc.timestamp_opt(dt.secs(), dt.subsec_nanos())
        .single()
        .unwrap_or_default()
}

pub fn aws_to_rfc3339(dt: &aws_smithy_types::DateTime) -> String {
    to_rfc3339(from_aws(dt))
}

/// HTTP 日付（`Fri, 21 Dec 2012 00:00:00 GMT`）を変換する。
pub fn parse_http_date(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc2822(s.trim())
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

/// バージョンを指定したダウンロードのファイル名に付ける日時（`YYYY-MM-DD HH.mm`、ローカル時刻。04 §5.1）。
pub fn version_file_stamp(dt: DateTime<Utc>) -> String {
    dt.with_timezone(&Local)
        .format("%Y-%m-%d %H.%M")
        .to_string()
}

/// 月の初日。
pub fn first_of_month(date: NaiveDate) -> NaiveDate {
    date.with_day(1).expect("day 1 exists")
}

/// 翌月の初日。
pub fn first_of_next_month(date: NaiveDate) -> NaiveDate {
    let (y, m) = if date.month() == 12 {
        (date.year() + 1, 1)
    } else {
        (date.year(), date.month() + 1)
    };
    NaiveDate::from_ymd_opt(y, m, 1).expect("valid date")
}

/// 前月の初日。
pub fn first_of_prev_month(date: NaiveDate) -> NaiveDate {
    let (y, m) = if date.month() == 1 {
        (date.year() - 1, 12)
    } else {
        (date.year(), date.month() - 1)
    };
    NaiveDate::from_ymd_opt(y, m, 1).expect("valid date")
}

pub fn days_in_month(date: NaiveDate) -> u32 {
    (first_of_next_month(date) - first_of_month(date)).num_days() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_rfc3339_in_utc() {
        let dt = Utc.with_ymd_and_hms(2026, 9, 27, 5, 30, 0).unwrap();
        assert_eq!(to_rfc3339(dt), "2026-09-27T05:30:00Z");
        assert_eq!(parse_rfc3339("2026-09-27T14:30:00+09:00"), Some(dt));
    }

    #[test]
    fn converts_aws_datetimes() {
        let aws = aws_smithy_types::DateTime::from_secs(1_790_000_000);
        assert_eq!(from_aws(&aws).timestamp(), 1_790_000_000);
    }

    #[test]
    fn parses_http_dates() {
        let dt = parse_http_date("Fri, 21 Dec 2012 00:00:00 GMT").unwrap();
        assert_eq!(dt, Utc.with_ymd_and_hms(2012, 12, 21, 0, 0, 0).unwrap());
    }

    #[test]
    fn computes_month_boundaries() {
        let d = NaiveDate::from_ymd_opt(2026, 12, 15).unwrap();
        assert_eq!(
            first_of_month(d),
            NaiveDate::from_ymd_opt(2026, 12, 1).unwrap()
        );
        assert_eq!(
            first_of_next_month(d),
            NaiveDate::from_ymd_opt(2027, 1, 1).unwrap()
        );
        let jan = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
        assert_eq!(
            first_of_prev_month(jan),
            NaiveDate::from_ymd_opt(2025, 12, 1).unwrap()
        );
        assert_eq!(
            days_in_month(NaiveDate::from_ymd_opt(2028, 2, 3).unwrap()),
            29
        );
        assert_eq!(
            days_in_month(NaiveDate::from_ymd_opt(2026, 9, 3).unwrap()),
            30
        );
    }
}
