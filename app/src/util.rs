//! 도메인 주인이 없는 순수 헬퍼.
//!
//! UI 엘리먼트를 반환하지 않고 `config`·`sync`·`logging` 어디에도 속하지 않는 함수만 둔다.

/// 주기 입력에 쓰는 시간 단위.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeUnit {
    Seconds,
    Minutes,
    Hours,
}

impl TimeUnit {
    /// 드롭다운에 나열할 순서.
    pub const ALL: [TimeUnit; 3] = [TimeUnit::Seconds, TimeUnit::Minutes, TimeUnit::Hours];

    pub fn label(self) -> &'static str {
        match self {
            TimeUnit::Seconds => "초",
            TimeUnit::Minutes => "분",
            TimeUnit::Hours => "시간",
        }
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|unit| unit.label() == label)
    }

    fn seconds_per(self) -> u64 {
        match self {
            TimeUnit::Seconds => 1,
            TimeUnit::Minutes => 60,
            TimeUnit::Hours => 3_600,
        }
    }
}

/// 주기로 허용하는 범위. 0초는 무한 루프가 되고, 24시간을 넘는 주기는 실수로 본다.
pub const MIN_INTERVAL_SECS: u32 = 1;
pub const MAX_INTERVAL_SECS: u32 = 24 * 60 * 60;

/// 사용자가 입력한 (값, 단위)를 초로 바꾼다.
///
/// 범위를 벗어나면 사용자에게 그대로 보여줄 사유를 반환한다.
pub fn interval_to_secs(amount: &str, unit: TimeUnit) -> Result<u32, String> {
    let trimmed = amount.trim();
    if trimmed.is_empty() {
        return Err("시간을 입력하세요.".to_string());
    }

    let Ok(amount) = trimmed.parse::<u64>() else {
        return Err("숫자만 입력할 수 있습니다.".to_string());
    };

    let total = amount.saturating_mul(unit.seconds_per());
    if total < MIN_INTERVAL_SECS as u64 {
        return Err(format!("{MIN_INTERVAL_SECS}초보다 짧게 설정할 수 없습니다."));
    }
    if total > MAX_INTERVAL_SECS as u64 {
        return Err("24시간보다 긴 주기는 설정할 수 없습니다.".to_string());
    }

    Ok(total as u32)
}

/// 초 단위 주기를 사람이 읽는 표기로 바꾼다.
///
/// 딱 떨어지지 않는 값은 큰 단위부터 이어 붙인다(`90` → `1분 30초`).
/// 값을 잘라 버리면 사용자가 고른 주기와 화면이 어긋나기 때문이다.
pub fn format_interval(secs: u32) -> String {
    if secs == 0 {
        return "0초".to_string();
    }

    let hours = secs / 3_600;
    let minutes = (secs % 3_600) / 60;
    let seconds = secs % 60;

    let mut parts = Vec::new();
    if hours > 0 {
        parts.push(format!("{hours}시간"));
    }
    if minutes > 0 {
        parts.push(format!("{minutes}분"));
    }
    if seconds > 0 {
        parts.push(format!("{seconds}초"));
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_whole_and_mixed_units() {
        assert_eq!(format_interval(10), "10초");
        assert_eq!(format_interval(30), "30초");
        assert_eq!(format_interval(60), "1분");
        assert_eq!(format_interval(120), "2분");
        assert_eq!(format_interval(3_600), "1시간");
        assert_eq!(format_interval(0), "0초");
    }

    #[test]
    fn mixed_values_keep_every_unit_instead_of_truncating() {
        assert_eq!(format_interval(90), "1분 30초");
        assert_eq!(format_interval(3_661), "1시간 1분 1초");
        assert_eq!(format_interval(3_630), "1시간 30초");
    }

    #[test]
    fn parses_amount_and_unit_into_seconds() {
        assert_eq!(interval_to_secs("10", TimeUnit::Seconds), Ok(10));
        assert_eq!(interval_to_secs(" 5 ", TimeUnit::Minutes), Ok(300));
        assert_eq!(interval_to_secs("2", TimeUnit::Hours), Ok(7_200));
    }

    #[test]
    fn rejects_empty_non_numeric_and_out_of_range_input() {
        assert!(interval_to_secs("", TimeUnit::Seconds).is_err());
        assert!(interval_to_secs("abc", TimeUnit::Seconds).is_err());
        assert!(interval_to_secs("-5", TimeUnit::Seconds).is_err());
        assert!(interval_to_secs("0", TimeUnit::Seconds).is_err());
        assert!(interval_to_secs("25", TimeUnit::Hours).is_err());
        // 곱셈 오버플로로 통과해 버리지 않는지 확인한다.
        assert!(interval_to_secs("99999999999999999999", TimeUnit::Hours).is_err());
    }

    #[test]
    fn unit_labels_round_trip() {
        for unit in TimeUnit::ALL {
            assert_eq!(TimeUnit::from_label(unit.label()), Some(unit));
        }
        assert_eq!(TimeUnit::from_label("일"), None);
    }
}
