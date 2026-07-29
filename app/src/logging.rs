//! 롤링 파일 로거.
//!
//! `log` 파사드 뒤에 붙어 콘솔(env_logger 동작 대체)과 파일에 동시에 기록한다.
//! 파일은 `%APPDATA%/gpui-convenience-tools/logs/app.log`에 쓰이고,
//! 아래 세 가지 보존 기준을 함께 적용한다.
//!
//! - 파일 용량: `max_file_size_mb`를 넘으면 타임스탬프 이름으로 롤링
//! - 파일 개수: `max_files`개(현재 파일 포함)를 넘는 오래된 파일 삭제
//! - 날짜 범위: `max_age_days`일보다 오래된 파일 삭제
//!
//! 설정은 런타임에 [`update_config`]로 교체할 수 있으며, 즉시 반영된다.

use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime},
};

use log::{Level, LevelFilter, Log, Metadata, Record};

use crate::config::{logs_path, LogConfig};

const LOG_FILE_NAME: &str = "app.log";
const ROLLED_PREFIX: &str = "app-";
const ROLLED_SUFFIX: &str = ".log";

static LOGGER: OnceLock<&'static RollingLogger> = OnceLock::new();

struct LoggerInner {
    config: LogConfig,
    file: Option<File>,
    /// 현재 파일에 기록된 바이트 수. 롤링 판정에 사용한다.
    written: u64,
}

pub struct RollingLogger {
    inner: Mutex<LoggerInner>,
    /// 콘솔 출력 최소 레벨.
    console_level: LevelFilter,
    /// 파일 기록 최소 레벨.
    file_level: LevelFilter,
}

/// 파일에 기록할 기본 최소 레벨.
///
/// `Trace`를 허용하면 gpui의 프레임 단위 vsync 로그가 파일을 가득 채워
/// 실제 앱 로그가 롤링으로 밀려나므로 기본값은 `Debug`다.
/// `RUST_LOG=trace`를 지정하면 이 상한도 함께 올라간다.
const DEFAULT_FILE_LEVEL: LevelFilter = LevelFilter::Debug;

/// 로거를 설치한다. 프로세스당 한 번만 유효하며, 이후 호출은 무시된다.
///
/// `RUST_LOG` 환경변수로 콘솔 레벨을 조정할 수 있다(기본 `info`).
pub fn init(config: LogConfig) {
    let console_level = std::env::var("RUST_LOG")
        .ok()
        .and_then(|v| v.parse::<LevelFilter>().ok())
        .unwrap_or(LevelFilter::Info);

    // 파일은 기본적으로 Debug까지만 받되, RUST_LOG로 더 낮은 레벨을 요청하면 따른다.
    let file_level = console_level.max(DEFAULT_FILE_LEVEL);

    let logger: &'static RollingLogger = Box::leak(Box::new(RollingLogger {
        inner: Mutex::new(LoggerInner {
            config,
            file: None,
            written: 0,
        }),
        console_level,
        file_level,
    }));

    if LOGGER.set(logger).is_err() {
        return;
    }

    // 파사드 상한은 콘솔과 파일 중 더 자세한 쪽에 맞춘다.
    log::set_max_level(console_level.max(file_level));
    let _ = log::set_logger(logger);

    logger.open_file();
    logger.enforce_retention();
}

/// 설정 변경을 즉시 반영한다. 로거가 아직 설치되지 않았으면 아무 일도 하지 않는다.
pub fn update_config(config: LogConfig) {
    let Some(logger) = LOGGER.get() else {
        return;
    };

    let reopen = {
        let Ok(mut inner) = logger.inner.lock() else {
            return;
        };
        let was_enabled = inner.config.file_enabled;
        inner.config = config;
        if !inner.config.file_enabled {
            inner.file = None;
            inner.written = 0;
            false
        } else {
            !was_enabled || inner.file.is_none()
        }
    };

    if reopen {
        logger.open_file();
    }
    logger.enforce_retention();
}

/// 현재 로그 파일 경로.
pub fn current_log_file() -> PathBuf {
    logs_path().join(LOG_FILE_NAME)
}

/// 현재 지역 시각을 `HH:MM:SS` 문자열로 반환한다.
pub fn now_hms() -> String {
    let (_, _, _, h, mi, s) = local_time_parts();
    format!("{h:02}:{mi:02}:{s:02}")
}

/// 로그 디렉터리에 존재하는 파일 개수와 총 용량(바이트).
pub fn log_dir_stats() -> (usize, u64) {
    let Ok(entries) = fs::read_dir(logs_path()) else {
        return (0, 0);
    };

    let mut count = 0usize;
    let mut bytes = 0u64;
    for entry in entries.flatten() {
        if !is_log_file(&entry.path()) {
            continue;
        }
        count += 1;
        if let Ok(meta) = entry.metadata() {
            bytes += meta.len();
        }
    }
    (count, bytes)
}

impl RollingLogger {
    fn open_file(&self) {
        let dir = logs_path();
        if fs::create_dir_all(&dir).is_err() {
            return;
        }

        let path = dir.join(LOG_FILE_NAME);
        let existing_len = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

        let file = OpenOptions::new().create(true).append(true).open(&path);

        if let Ok(mut inner) = self.inner.lock() {
            match file {
                Ok(file) => {
                    inner.file = Some(file);
                    inner.written = existing_len;
                }
                Err(_) => {
                    inner.file = None;
                    inner.written = 0;
                }
            }
        }
    }

    /// 현재 파일을 타임스탬프 이름으로 옮기고 새 파일을 연다.
    fn roll(&self) {
        let dir = logs_path();
        let current = dir.join(LOG_FILE_NAME);
        let (y, mo, d, h, mi, s) = local_time_parts();
        let rolled = dir.join(format!(
            "{ROLLED_PREFIX}{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}{ROLLED_SUFFIX}"
        ));

        {
            // 파일 핸들을 먼저 닫아야 Windows에서 rename이 가능하다.
            if let Ok(mut inner) = self.inner.lock() {
                inner.file = None;
                inner.written = 0;
            }
        }

        // 같은 초에 두 번 롤링되면 이름이 겹치므로 접미사를 붙여 회피한다.
        let mut destination = rolled.clone();
        let mut dedup = 1u32;
        while destination.exists() && dedup < 100 {
            destination = dir.join(format!(
                "{ROLLED_PREFIX}{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}-{dedup}{ROLLED_SUFFIX}"
            ));
            dedup += 1;
        }

        let _ = fs::rename(&current, &destination);

        self.open_file();
        self.enforce_retention();
    }

    /// 개수·날짜 기준 보존 정책을 적용해 초과분을 삭제한다.
    fn enforce_retention(&self) {
        let config = {
            let Ok(inner) = self.inner.lock() else {
                return;
            };
            inner.config.clone()
        };

        let dir = logs_path();
        let Ok(entries) = fs::read_dir(&dir) else {
            return;
        };

        // 롤링된 파일만 대상으로 한다. 현재 파일(app.log)은 삭제하지 않는다.
        let mut rolled: Vec<(PathBuf, SystemTime)> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| is_rolled_log_file(p))
            .map(|p| {
                let modified = fs::metadata(&p)
                    .and_then(|m| m.modified())
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                (p, modified)
            })
            .collect();

        // 최신순 정렬
        rolled.sort_by(|a, b| b.1.cmp(&a.1));

        // ── 날짜 범위 기준 ──
        if config.max_age_days > 0 {
            let cutoff = SystemTime::now()
                .checked_sub(Duration::from_secs(config.max_age_days as u64 * 86_400));
            if let Some(cutoff) = cutoff {
                rolled.retain(|(path, modified)| {
                    if *modified < cutoff {
                        let _ = fs::remove_file(path);
                        false
                    } else {
                        true
                    }
                });
            }
        }

        // ── 개수 기준 (현재 파일 1개를 포함해서 센다) ──
        let keep_rolled = config.max_files.saturating_sub(1) as usize;
        if rolled.len() > keep_rolled {
            for (path, _) in rolled.iter().skip(keep_rolled) {
                let _ = fs::remove_file(path);
            }
        }
    }

    /// 한 줄을 파일에 기록하고, 필요하면 롤링을 예약한다.
    /// 롤링이 필요하면 `true`를 반환한다(락 해제 후 처리해야 하므로).
    fn write_line(&self, line: &str) -> bool {
        let Ok(mut inner) = self.inner.lock() else {
            return false;
        };

        if !inner.config.file_enabled {
            return false;
        }

        if inner.file.is_none() {
            return false;
        }

        let bytes = line.as_bytes();
        if let Some(file) = inner.file.as_mut() {
            if file.write_all(bytes).is_err() {
                return false;
            }
            let _ = file.flush();
        }
        inner.written += bytes.len() as u64;

        let limit = inner.config.max_file_size_mb as u64 * 1024 * 1024;
        limit > 0 && inner.written >= limit
    }
}

impl Log for RollingLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let (y, mo, d, h, mi, s) = local_time_parts();
        let line = format!(
            "{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02} [{:<5}] {}: {}\n",
            record.level(),
            record.target(),
            record.args()
        );

        if record.level() <= self.console_level {
            if record.level() <= Level::Warn {
                eprint!("{line}");
            } else {
                print!("{line}");
            }
        }

        if record.level() <= self.file_level && self.write_line(&line) {
            self.roll();
        }
    }

    fn flush(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            if let Some(file) = inner.file.as_mut() {
                let _ = file.flush();
            }
        }
    }
}

fn is_log_file(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("log"))
}

fn is_rolled_log_file(path: &Path) -> bool {
    if !is_log_file(path) {
        return false;
    }
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with(ROLLED_PREFIX))
}

/// 현재 지역 시각을 (년, 월, 일, 시, 분, 초)로 반환한다.
#[cfg(target_os = "windows")]
fn local_time_parts() -> (u16, u16, u16, u16, u16, u16) {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;

    let mut st: SYSTEMTIME = unsafe { std::mem::zeroed() };
    // SAFETY: GetLocalTime은 전달된 SYSTEMTIME 구조체에만 기록한다.
    unsafe {
        GetLocalTime(&mut st as *mut SYSTEMTIME);
    }
    (
        st.wYear,
        st.wMonth,
        st.wDay,
        st.wHour,
        st.wMinute,
        st.wSecond,
    )
}

/// 비Windows 폴백. 지역 시간대 정보를 얻을 수 없으므로 UTC를 사용한다.
#[cfg(not(target_os = "windows"))]
fn local_time_parts() -> (u16, u16, u16, u16, u16, u16) {
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (y, mo, d) = civil_from_days(days);
    (
        y as u16,
        mo as u16,
        d as u16,
        (rem / 3600) as u16,
        ((rem % 3600) / 60) as u16,
        (rem % 60) as u16,
    )
}

/// 1970-01-01 기준 일수 → (년, 월, 일). Howard Hinnant의 civil_from_days 알고리즘.
#[cfg(not(target_os = "windows"))]
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolled_file_detection_excludes_current_file() {
        let dir = std::env::temp_dir();
        let current = dir.join("app.log");
        let rolled = dir.join("app-20260101-000000.log");
        let unrelated = dir.join("other.txt");

        // 파일 존재 여부와 무관하게 이름 규칙만 검사하도록 임시 파일을 만든다.
        let _ = fs::write(&current, b"");
        let _ = fs::write(&rolled, b"");
        let _ = fs::write(&unrelated, b"");

        assert!(!is_rolled_log_file(&current));
        assert!(is_rolled_log_file(&rolled));
        assert!(!is_rolled_log_file(&unrelated));

        let _ = fs::remove_file(&current);
        let _ = fs::remove_file(&rolled);
        let _ = fs::remove_file(&unrelated);
    }

    #[test]
    fn local_time_parts_are_in_range() {
        let (y, mo, d, h, mi, s) = local_time_parts();
        assert!(y >= 2020, "year should be sane: {y}");
        assert!((1..=12).contains(&mo), "month out of range: {mo}");
        assert!((1..=31).contains(&d), "day out of range: {d}");
        assert!(h < 24 && mi < 60 && s < 60);
    }
}
