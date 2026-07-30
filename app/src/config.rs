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
    /// 주기 드롭다운에 나열할 초 단위 프리셋.
    ///
    /// 광고 차단 스캔 주기와 파일 동기화 감시 주기가 **같은 목록을 공유**한다.
    /// 사용자가 한쪽에서 추가한 주기를 다른 쪽에서 다시 만들 이유가 없기 때문이다.
    #[serde(default = "default_interval_presets")]
    pub interval_presets: Vec<u32>,
    /// 파일 동기화 자동 실행 전역 스위치.
    ///
    /// 개별 작업의 `enabled`와는 별개다. 이 스위치를 끄면 어떤 작업도 주기가 도래해도
    /// 스스로 돌지 않는다(수동 '지금 동기화'는 그대로 동작한다). 앱을 켜두기만 해도
    /// 동기화가 도는 기능이므로 사이드바에서 바로 끌 수 있어야 한다.
    #[serde(default = "default_true")]
    pub sync_enabled: bool,
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

/// 기본 주기 프리셋: 10초 · 30초 · 1분.
pub fn default_interval_presets() -> Vec<u32> {
    vec![10, 30, 60]
}

/// 저장하려는 작업 스냅샷에 디스크가 갖고 있던 진행 상황을 되살린다.
///
/// `last_run_unix`·`resume_cursor`는 동기화 엔진만 갱신하는데, UI는 자기 메모리 스냅샷을
/// 통째로 저장한다. 그대로 두면 사용자가 설정을 만질 때마다 방금 기록한 진행 위치가
/// 앱 시작 시점 값으로 되돌아가 "이어서 동기화"가 무력해진다.
pub fn carry_over_engine_progress(stored: &[SyncJob], jobs: &mut [SyncJob]) {
    for job in jobs.iter_mut() {
        let Some(previous) = stored.iter().find(|candidate| candidate.id == job.id) else {
            continue;
        };
        job.last_run_unix = previous.last_run_unix;
        job.resume_cursor = previous.resume_cursor.clone();
    }
}

/// 프리셋 목록을 정규화한다 — 중복 제거 후 오름차순 정렬.
///
/// 드롭다운 순서를 사용자가 추가한 순서에 맡기면 목록이 금세 뒤죽박죽이 된다.
pub fn normalize_interval_presets(presets: &mut Vec<u32>) {
    presets.retain(|secs| *secs > 0);
    presets.sort_unstable();
    presets.dedup();
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

    // ── 아래 두 필드는 동기화 엔진 계층이 소유한다 ──
    //
    // 앱을 껐다 켜도 "처음부터 다시" 돌지 않게 하려고 실행 진행 상황을 작업 자체에 남긴다.
    // **UI는 이 값을 쓰지 않는다.** 사용자가 설정을 저장할 때마다 UI 스냅샷으로 덮어쓰면
    // 백그라운드가 방금 기록한 진행 상황이 날아가므로, 저장 경로에서 디스크 값을 보존한다.
    /// 마지막 실행을 끝낸 시각(유닉스 초).
    ///
    /// 없으면 "한 번도 실행하지 않음"이므로 시작 직후 곧바로 실행한다.
    #[serde(default)]
    pub last_run_unix: Option<u64>,
    /// 중단된 순회를 이어서 시작할 지점(원본 기준 상대 경로).
    ///
    /// 실행이 정상적으로 끝나면 비운다. 값이 남아 있다는 것은 중지 또는 앱 종료로
    /// 순회가 끊겼다는 뜻이다.
    #[serde(default)]
    pub resume_cursor: Option<String>,
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
            last_run_unix: None,
            resume_cursor: None,
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
            interval_presets: default_interval_presets(),
            sync_enabled: true,
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

/// 데이터 루트를 사용자 프로필 밖으로 돌리는 환경 변수. 시각 검증 하네스 전용이다.
pub const DATA_DIR_ENV: &str = "GPUI_CONVENIENCE_TOOLS_DATA_DIR";

/// `%APPDATA%/gpui-convenience-tools` (Windows 기준) 데이터 루트.
///
/// [`DATA_DIR_ENV`]가 비어 있지 않게 설정돼 있으면 그 경로를 대신 쓴다.
/// `dirs::config_dir()`는 Windows에서 `SHGetKnownFolderPath`를 호출하므로 `APPDATA`
/// 환경 변수를 바꿔도 반영되지 않는다. 시각 검증이 사용자의 실제 `config.json`·로그를
/// 건드리지 않고 격리 실행하려면 이 전용 변수가 필요하다.
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os(DATA_DIR_ENV) {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
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
        // 스위치가 없던 config는 켜진 상태로 읽혀야 기존 동작이 유지된다.
        assert!(config.sync_enabled);
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

    /// UI 스냅샷을 저장해도 엔진이 기록한 실행 진행 상황은 살아남아야 한다.
    #[test]
    fn saving_ui_snapshot_keeps_engine_progress() {
        let stored = vec![SyncJob {
            id: "job-1".to_string(),
            source: r"D:\a".to_string(),
            last_run_unix: Some(1_700_000_000),
            resume_cursor: Some(r"nested\file.txt".to_string()),
            ..SyncJob::default()
        }];

        // UI는 진행 상황을 모른 채(=None) 이름만 바꿔서 저장한다.
        let mut incoming = vec![SyncJob {
            id: "job-1".to_string(),
            name: "이름 변경".to_string(),
            source: r"D:\a".to_string(),
            ..SyncJob::default()
        }];
        carry_over_engine_progress(&stored, &mut incoming);

        assert_eq!(incoming[0].name, "이름 변경", "사용자 수정은 그대로 반영된다");
        assert_eq!(incoming[0].last_run_unix, Some(1_700_000_000));
        assert_eq!(
            incoming[0].resume_cursor.as_deref(),
            Some(r"nested\file.txt"),
            "이어서 시작할 지점이 사라지면 재시작마다 처음부터 다시 돈다"
        );

        // 저장본에 없던 새 작업은 건드리지 않는다.
        let mut fresh = vec![SyncJob {
            id: "job-2".to_string(),
            ..SyncJob::default()
        }];
        carry_over_engine_progress(&stored, &mut fresh);
        assert!(fresh[0].resume_cursor.is_none());
        assert!(fresh[0].last_run_unix.is_none());
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
