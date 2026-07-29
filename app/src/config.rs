use anyhow::Result;
use gpui_component::theme::ThemeMode;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

use crate::app::TargetApp;

/// 앱 전체 설정.
///
/// 저장은 항상 전체 구조체 단위로 이루어진다. 부분 갱신이 필요하면
/// [`update_config`]를 사용해 읽기-수정-쓰기를 원자적으로 처리한다.
/// 새 필드에는 반드시 `#[serde(default)]`를 붙여 기존 config.json 호환성을 유지한다.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub service_enabled: bool,
    pub targets: Vec<TargetApp>,
    #[serde(default)]
    pub light_theme_name: Option<String>,
    #[serde(default)]
    pub dark_theme_name: Option<String>,
    #[serde(default = "default_scan_interval_secs")]
    pub scan_interval_secs: u32,
    #[serde(default)]
    pub favorite_services: Vec<String>,
    /// 파일 동기화 작업 목록.
    #[serde(default)]
    pub sync_jobs: Vec<SyncJob>,
    /// 로그 롤링 파일 설정.
    #[serde(default)]
    pub log: LogConfig,
}

fn default_scan_interval_secs() -> u32 {
    10
}

// ─────────────────────────────────────────────
// 파일 동기화 설정
// ─────────────────────────────────────────────

/// 원본 폴더 → 대상 폴더 동기화 작업 하나.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncJob {
    /// 작업 고유 ID.
    ///
    /// 목록 인덱스는 작업 추가·삭제로 밀리기 때문에, 백그라운드 스레드의 실행 주기 추적과
    /// 실행 결과 매칭은 반드시 이 ID를 기준으로 한다. 구버전 config에는 없으므로
    /// 로드 후 [`SyncJob::ensure_id`]로 채운다.
    #[serde(default)]
    pub id: String,
    /// 사용자에게 보여줄 이름. 비어 있으면 원본 폴더명을 사용한다.
    #[serde(default)]
    pub name: String,
    pub source: String,
    pub target: String,
    /// 이 작업의 자동 동기화 사용 여부.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 감시 주기(초).
    #[serde(default = "default_sync_interval_secs")]
    pub interval_secs: u32,
    /// 원본에서 삭제된 파일을 대상에서도 삭제할지 여부.
    #[serde(default)]
    pub mirror_deletes: bool,
    /// 숨김/시스템 속성 파일도 동기화할지 여부. 기본값은 전체 동기화이므로 true.
    #[serde(default = "default_true")]
    pub include_hidden: bool,
}

impl SyncJob {
    /// ID가 비어 있으면(구버전 config에서 로드된 경우) 새로 채운다.
    pub fn ensure_id(&mut self) {
        if self.id.is_empty() {
            self.id = new_job_id();
        }
    }

    /// 목록 표시에 사용할 이름. `name`이 비어 있으면 원본 폴더명으로 대체한다.
    pub fn label(&self) -> String {
        if !self.name.trim().is_empty() {
            return self.name.clone();
        }
        PathBuf::from(&self.source)
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.to_string())
            .unwrap_or_else(|| self.source.clone())
    }
}

/// 프로세스 내에서 유일한 동기화 작업 ID를 만든다.
///
/// 외부 uuid 의존성을 들이지 않기 위해 `유닉스초-프로세스내_일련번호` 형식을 쓴다.
/// 같은 config 안에서만 유일하면 충분하다.
fn new_job_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{secs:x}-{seq:x}")
}

impl Default for SyncJob {
    fn default() -> Self {
        Self {
            id: new_job_id(),
            name: String::new(),
            source: String::new(),
            target: String::new(),
            enabled: true,
            interval_secs: default_sync_interval_secs(),
            mirror_deletes: false,
            include_hidden: true,
        }
    }
}

fn default_sync_interval_secs() -> u32 {
    60
}

fn default_true() -> bool {
    true
}

// ─────────────────────────────────────────────
// 로그 롤링 설정
// ─────────────────────────────────────────────

/// 롤링 파일 로그 설정.
///
/// 세 가지 보존 기준이 함께 적용된다(모두 초과분 삭제).
/// - `max_files`: 보관할 롤링 파일 최대 개수
/// - `max_age_days`: 보관할 최대 일수
/// - `max_file_size_mb`: 파일 하나가 이 크기를 넘으면 롤링
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogConfig {
    /// 파일 로그 기록 사용 여부.
    #[serde(default = "default_true")]
    pub file_enabled: bool,
    /// 보관할 롤링 파일 최대 개수(현재 파일 포함).
    #[serde(default = "default_max_files")]
    pub max_files: u32,
    /// 보관할 최대 일수. 0이면 날짜 기준 삭제를 하지 않는다.
    #[serde(default = "default_max_age_days")]
    pub max_age_days: u32,
    /// 롤링 임계 파일 크기(MB). 0이면 크기 기준 롤링을 하지 않는다.
    #[serde(default = "default_max_file_size_mb")]
    pub max_file_size_mb: u32,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            file_enabled: true,
            max_files: default_max_files(),
            max_age_days: default_max_age_days(),
            max_file_size_mb: default_max_file_size_mb(),
        }
    }
}

fn default_max_files() -> u32 {
    10
}

fn default_max_age_days() -> u32 {
    30
}

fn default_max_file_size_mb() -> u32 {
    5
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            service_enabled: true,
            targets: vec![TargetApp {
                process_name: "KakaoTalk.exe".to_string(),
                display_name: "KakaoTalk".to_string(),
                enabled: true,
                ad_window_class: "Chrome_WidgetWin_1".to_string(),
            }],
            light_theme_name: None,
            dark_theme_name: None,
            scan_interval_secs: default_scan_interval_secs(),
            favorite_services: Vec::new(),
            sync_jobs: Vec::new(),
            log: LogConfig::default(),
        }
    }
}

pub(crate) const BUNDLED_THEMES: [(&str, &str); 21] = [
    (
        "adventure.json",
        include_str!("../assets/themes/adventure.json"),
    ),
    ("alduin.json", include_str!("../assets/themes/alduin.json")),
    (
        "asciinema.json",
        include_str!("../assets/themes/asciinema.json"),
    ),
    ("ayu.json", include_str!("../assets/themes/ayu.json")),
    (
        "tokyonight.json",
        include_str!("../assets/themes/tokyonight.json"),
    ),
    (
        "catppuccin.json",
        include_str!("../assets/themes/catppuccin.json"),
    ),
    (
        "gruvbox.json",
        include_str!("../assets/themes/gruvbox.json"),
    ),
    (
        "everforest.json",
        include_str!("../assets/themes/everforest.json"),
    ),
    (
        "flexoki.json",
        include_str!("../assets/themes/flexoki.json"),
    ),
    (
        "fahrenheit.json",
        include_str!("../assets/themes/fahrenheit.json"),
    ),
    ("harper.json", include_str!("../assets/themes/harper.json")),
    ("hybrid.json", include_str!("../assets/themes/hybrid.json")),
    (
        "jellybeans.json",
        include_str!("../assets/themes/jellybeans.json"),
    ),
    ("kibble.json", include_str!("../assets/themes/kibble.json")),
    (
        "macos-classic.json",
        include_str!("../assets/themes/macos-classic.json"),
    ),
    ("matrix.json", include_str!("../assets/themes/matrix.json")),
    (
        "mellifluous.json",
        include_str!("../assets/themes/mellifluous.json"),
    ),
    (
        "molokai.json",
        include_str!("../assets/themes/molokai.json"),
    ),
    (
        "solarized.json",
        include_str!("../assets/themes/solarized.json"),
    ),
    (
        "spaceduck.json",
        include_str!("../assets/themes/spaceduck.json"),
    ),
    (
        "twilight.json",
        include_str!("../assets/themes/twilight.json"),
    ),
];

/// `%APPDATA%/gpui-convenience-tools` (Windows 기준) 데이터 루트.
pub fn data_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gpui-convenience-tools")
}

pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

pub fn themes_path() -> PathBuf {
    data_dir().join("themes")
}

/// 롤링 로그 파일이 쌓이는 디렉터리.
pub fn logs_path() -> PathBuf {
    data_dir().join("logs")
}

pub fn ensure_bundled_themes() -> Result<PathBuf> {
    let path = themes_path();
    fs::create_dir_all(&path)?;

    for (name, contents) in BUNDLED_THEMES {
        let target = path.join(name);
        if !target.exists() {
            fs::write(target, contents)?;
        }
    }

    Ok(path)
}

pub fn save_config(config: &AppConfig) -> Result<()> {
    let path = config_path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(config)?;
    fs::write(path, json)?;
    Ok(())
}

pub fn load_config() -> Result<Option<AppConfig>> {
    let path = config_path();

    if !path.exists() {
        return Ok(None);
    }

    let data = fs::read_to_string(path)?;
    let config = serde_json::from_str::<AppConfig>(&data)?;
    Ok(Some(config))
}

/// 저장된 설정을 읽어 수정한 뒤 다시 저장한다.
///
/// 설정 필드가 늘어날 때마다 저장 지점마다 복사 로직을 추가하는 실수를 막기 위한
/// 단일 경로다. 새 저장 로직은 반드시 이 함수를 사용한다.
pub fn update_config(edit: impl FnOnce(&mut AppConfig)) -> Result<AppConfig> {
    let mut config = load_config()?.unwrap_or_default();
    edit(&mut config);
    save_config(&config)?;
    Ok(config)
}

pub fn save_theme_selection(mode: ThemeMode, theme_name: &str) -> Result<()> {
    update_config(|config| match mode {
        ThemeMode::Light => config.light_theme_name = Some(theme_name.to_string()),
        ThemeMode::Dark => config.dark_theme_name = Some(theme_name.to_string()),
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 구버전 config.json(신규 필드 없음)이 그대로 로드되어야 한다.
    #[test]
    fn legacy_config_without_new_fields_still_parses() {
        let legacy = r#"{
            "service_enabled": true,
            "targets": [],
            "scan_interval_secs": 15
        }"#;

        let config: AppConfig = serde_json::from_str(legacy).expect("구버전 config 파싱");
        assert_eq!(config.scan_interval_secs, 15);
        assert!(config.sync_jobs.is_empty());
        // 신규 필드는 기본값으로 채워진다.
        assert!(config.log.file_enabled);
        assert_eq!(config.log.max_files, default_max_files());
    }

    /// 구버전 SyncJob에는 id가 없으므로 ensure_id로 채워야 한다.
    #[test]
    fn sync_job_without_id_gets_one() {
        let legacy = r#"{"source":"C:\\a","target":"C:\\b"}"#;
        let mut job: SyncJob = serde_json::from_str(legacy).expect("구버전 SyncJob 파싱");

        assert!(job.id.is_empty());
        job.ensure_id();
        assert!(!job.id.is_empty());

        // 이미 ID가 있으면 유지한다.
        let existing = job.id.clone();
        job.ensure_id();
        assert_eq!(job.id, existing);
    }

    /// 새로 만든 작업들은 서로 다른 ID를 갖는다.
    #[test]
    fn new_sync_jobs_have_unique_ids() {
        let a = SyncJob::default();
        let b = SyncJob::default();
        assert_ne!(a.id, b.id);
    }

    /// `label()`은 이름이 비었을 때 원본 폴더명으로 대체된다.
    #[test]
    fn label_falls_back_to_source_folder_name() {
        let job = SyncJob {
            source: r"D:\작업\원본폴더".to_string(),
            ..SyncJob::default()
        };
        assert_eq!(job.label(), "원본폴더");

        let named = SyncJob {
            name: "내 백업".to_string(),
            source: r"D:\작업\원본폴더".to_string(),
            ..SyncJob::default()
        };
        assert_eq!(named.label(), "내 백업");
    }
}
