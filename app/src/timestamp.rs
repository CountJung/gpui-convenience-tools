//! 부동소수점 없이 Unix 초(최대 나노초)와 한국어 날짜를 상호 변환한다.
use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone, Timelike, Utc};

fn zone(utc: bool) -> FixedOffset {
    FixedOffset::east_opt(if utc { 0 } else { 9 * 3600 }).expect("상수 시간대")
}

pub fn parse_epoch(input: &str) -> Result<DateTime<Utc>> {
    let input = input.trim();
    if input.len() > 40 || input.is_empty() {
        bail!("Unix 초를 입력하세요 (예: 1792461476.171).");
    }
    let negative = input.starts_with('-');
    let unsigned = input.strip_prefix(['-', '+']).unwrap_or(input);
    let (whole, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    if whole.is_empty()
        || !whole.bytes().all(|c| c.is_ascii_digit())
        || fraction.len() > 9
        || !fraction.bytes().all(|c| c.is_ascii_digit())
        || (unsigned.contains('.') && fraction.is_empty())
    {
        bail!("초 단위 십진수만 지원합니다. 소수는 최대 9자리입니다.");
    }
    let seconds: i128 = whole.parse().context("지원 범위를 벗어난 Unix 초입니다.")?;
    let nanos = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<i128>()? * 10_i128.pow(9 - fraction.len() as u32)
    };
    let total = seconds
        .checked_mul(1_000_000_000)
        .and_then(|s| s.checked_add(nanos))
        .ok_or_else(|| anyhow!("지원 범위를 벗어난 Unix 초입니다."))?
        * if negative { -1 } else { 1 };
    let secs = i64::try_from(total.div_euclid(1_000_000_000))
        .context("지원 범위를 벗어난 Unix 초입니다.")?;
    DateTime::from_timestamp(secs, total.rem_euclid(1_000_000_000) as u32)
        .ok_or_else(|| anyhow!("지원 범위를 벗어난 날짜입니다."))
}

pub fn epoch_to_date(input: &str, utc: bool) -> Result<String> {
    let date = parse_epoch(input)?
        .naive_utc()
        .checked_add_signed(chrono::Duration::seconds(i64::from(
            zone(utc).local_minus_utc(),
        )))
        .ok_or_else(|| anyhow!("선택한 시간대의 날짜 범위를 벗어났습니다."))?;
    let hour = match date.hour() % 12 {
        0 => 12,
        value => value,
    };
    let suffix = fractional(date.nanosecond());
    Ok(format!(
        "{} {} {hour}:{:02}:{:02}{suffix}",
        date.format("%Y. %-m. %-d."),
        if date.hour() < 12 { "오전" } else { "오후" },
        date.minute(),
        date.second()
    ))
}

fn fractional(nanos: u32) -> String {
    if nanos == 0 {
        String::new()
    } else {
        format!(".{nanos:09}").trim_end_matches('0').to_string()
    }
}

pub fn date_to_epoch(input: &str, utc: bool) -> Result<String> {
    let input = input.trim();
    if input.len() > 120 {
        bail!("날짜 입력이 너무 깁니다.");
    }
    // Chrono는 나노초보다 작은 자릿수를 버리므로 파싱 전에 명시적으로 거부한다.
    if input
        .rsplit_once('.')
        .is_some_and(|(_, fraction)| fraction.bytes().take_while(u8::is_ascii_digit).count() > 9)
    {
        bail!("소수 초는 최대 9자리입니다. 정밀도를 줄인 뒤 다시 입력하세요.");
    }
    // ISO 8601의 명시적인 offset은 선택한 표시 시간대보다 우선한다.
    let date = if let Ok(date) = DateTime::parse_from_rfc3339(input) {
        date.with_timezone(&Utc)
    } else {
        let normalized = if input.contains("오전") || input.contains("오후") {
            let (prefix, time, afternoon) = if let Some((prefix, time)) = input.split_once("오전")
            {
                (prefix, time, false)
            } else {
                let (prefix, time) = input
                    .split_once("오후")
                    .context("오전/오후 표기를 확인하세요.")?;
                (prefix, time, true)
            };
            let (hour, rest) = time
                .trim()
                .split_once(':')
                .context("시간 형식은 시:분:초입니다.")?;
            let hour = hour.trim().parse::<u32>().context("시간을 확인하세요.")?;
            if !(1..=12).contains(&hour) {
                bail!("오전/오후 시간은 1~12입니다.");
            }
            let day = prefix
                .split('.')
                .map(str::trim)
                .filter(|x| !x.is_empty())
                .collect::<Vec<_>>();
            if day.len() != 3 {
                bail!("날짜 형식은 2026. 10. 20. 오전 10:57:56 입니다.");
            }
            format!(
                "{}-{}-{} {}:{rest}",
                day[0],
                day[1],
                day[2],
                hour % 12 + if afternoon { 12 } else { 0 }
            )
        } else {
            input.replace('T', " ")
        };
        let naive = NaiveDateTime::parse_from_str(&normalized, "%Y-%m-%d %H:%M:%S%.f")
            .context("날짜를 확인하세요: 한국어 오전/오후, YYYY-MM-DD HH:MM:SS 또는 ISO 8601.")?;
        if naive.nanosecond() >= 1_000_000_000 {
            bail!("윤초는 Unix 초 변환에서 지원하지 않습니다.");
        }
        zone(utc)
            .from_local_datetime(&naive)
            .single()
            .context("날짜 범위를 확인하세요.")?
            .with_timezone(&Utc)
    };
    if date.nanosecond() >= 1_000_000_000 {
        bail!("윤초는 지원하지 않습니다.");
    }
    let total = i128::from(date.timestamp()) * 1_000_000_000 + i128::from(date.nanosecond());
    let absolute = total.abs();
    Ok(format!(
        "{}{}{}",
        if total < 0 { "-" } else { "" },
        absolute / 1_000_000_000,
        fractional((absolute % 1_000_000_000) as u32)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timestamp_korean_example_and_fraction_roundtrip() {
        assert_eq!(
            epoch_to_date("1792461476.171", false).unwrap(),
            "2026. 10. 20. 오전 10:57:56.171"
        );
        assert_eq!(
            date_to_epoch("2026. 10. 20. 오전 10:57:56.171", false).unwrap(),
            "1792461476.171"
        );
        assert_eq!(
            date_to_epoch("2026. 10. 20. 오전 10:57:56", false).unwrap(),
            "1792461476"
        );
    }
    #[test]
    fn timestamp_negative_nanos_utc_and_midnight_roundtrip() {
        for utc in [true, false] {
            for value in [
                "0",
                "-0.001",
                "-1.999999999",
                "1.000000001",
                "253402300799.99",
            ] {
                let text = epoch_to_date(value, utc).unwrap();
                assert_eq!(date_to_epoch(&text, utc).unwrap(), value);
            }
        }
        assert_eq!(
            date_to_epoch("1970. 1. 1. 오전 12:00:00", true).unwrap(),
            "0"
        );
        assert_eq!(
            date_to_epoch("1970. 1. 1. 오후 12:00:00", true).unwrap(),
            "43200"
        );
        assert_eq!(
            date_to_epoch("1970-01-01T09:00:00+09:00", true).unwrap(),
            "0"
        );
    }
    #[test]
    fn timestamp_rejects_invalid_dates_and_precision() {
        for value in [
            "",
            ".1",
            "1.",
            "NaN",
            "1e9",
            "1.0000000001",
            "999999999999999999999999999999999999999",
        ] {
            assert!(parse_epoch(value).is_err(), "{value}");
        }
        for value in [
            "2026-02-30 10:00:00",
            "2026. 1. 1. 오후 13:00:00",
            "2016-12-31T23:59:60Z",
            "1970-01-01T00:00:00.0000000001Z",
            "1970-01-01T09:00:00.0000000001+09:00",
            "1970-01-01 00:00:00.1234567891",
            "1970. 1. 1. 오전 12:00:00.1234567891",
        ] {
            assert!(date_to_epoch(value, false).is_err(), "{value}");
        }
        assert!(epoch_to_date(&DateTime::<Utc>::MAX_UTC.timestamp().to_string(), false).is_err());
    }
}
