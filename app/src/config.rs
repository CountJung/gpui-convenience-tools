use anyhow::Result;
use gpui_component::theme::ThemeMode;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

mod clipboard;
mod openssl;
mod persistence;
pub(crate) mod process;
mod timestamp;
pub use clipboard::ClipboardConfig;
pub use openssl::OpenSslConfig;
#[cfg(test)]
use persistence::{config_path, fallback_config_path, save_config};
pub use persistence::{
    effective_config_path, load_config, open_config_file, save_theme_mode, save_theme_selection,
    update_config,
};
pub use process::{ProcessMonitorConfig, ProcessSort};
pub use timestamp::TimestampConfig;

use crate::app::TargetApp;
pub use crate::config_layout::{
    normalize_virtual_disk_attributes_width, normalize_virtual_disk_kind_width,
    normalize_virtual_disk_size_width, normalize_virtual_disk_tree_width, VirtualDiskLayoutConfig,
};

/// 앱 셸 사이드바의 기본 폭과 허용 범위.
pub const DEFAULT_SIDEBAR_WIDTH: f32 = 240.0;
const MIN_SIDEBAR_WIDTH: f32 = 200.0;
const MAX_SIDEBAR_WIDTH: f32 = 360.0;

/// 앱 전체 설정.
///
/// 저장은 항상 전체 구조체 단위로 이루어진다. 부분 갱신이 필요하면
/// [`update_config`]를 사용해 읽기-수정-쓰기를 원자적으로 처리한다.
/// 새 필드에는 반드시 `#[serde(default)]`를 붙여 기존 config.json 호환성을 유지한다.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub service_enabled: bool,
    pub targets: Vec<TargetApp>,
    /// 카카오톡 기본 채팅창의 빈 광고 자리를 회수하는 실험적 선택 모드.
    #[serde(default)]
    pub kakao_reclaim_ad_space: bool,
    #[serde(default)]
    pub light_theme_name: Option<String>,
    #[serde(default)]
    pub dark_theme_name: Option<String>,
    /// 사용자가 직접 고른 밝음/어두움 모드. None은 구버전의 초기 모드를 유지한다.
    #[serde(default)]
    pub theme_mode: Option<ThemeMode>,
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
    /// 앱 셸 사이드바 폭(px).
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: f32,
    /// 사용자가 확인하고 반복 알림을 억제한 VDI 복사 오류 키.
    #[serde(default)]
    pub virtual_disk_suppressed_issue_keys: Vec<String>,
    /// 로그 롤링 파일 설정.
    #[serde(default)]
    pub log: LogConfig,
    /// VirtualBox 디스크 탐색에서 사용자가 마지막으로 선택한 입력값.
    #[serde(default)]
    pub virtual_disk: VirtualDiskConfig,
    /// PCF 호환 암복호화의 사용자 조정값. 작업 텍스트는 저장하지 않는다.
    #[serde(default)]
    pub openssl: OpenSslConfig,
    #[serde(default)]
    pub timestamp: TimestampConfig,
    #[serde(default)]
    pub process_monitor: ProcessMonitorConfig,
    #[serde(default)]
    pub clipboard: ClipboardConfig,
}

fn default_scan_interval_secs() -> u32 {
    10
}

fn default_sidebar_width() -> f32 {
    DEFAULT_SIDEBAR_WIDTH
}

/// 저장된 사이드바 폭을 현재 UI가 지원하는 범위로 보정한다.
pub fn normalize_sidebar_width(width: f32) -> f32 {
    if width.is_finite() {
        width.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH)
    } else {
        DEFAULT_SIDEBAR_WIDTH
    }
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

/// 파일 동기화 작업이 다음 실행을 기다리는 방식.
///
/// 기존 설정 파일은 필드가 없으므로 `Interval`을 기본값으로 사용한다. 실제 감시
/// 스레드와 디바운스 정책은 D-011~D-012에서 이 값에 연결한다.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatchMode {
    /// 설정된 주기가 도래할 때 작업을 실행한다.
    #[default]
    Interval,
    /// 원본 변경 이벤트를 받아 작업을 실행한다.
    Realtime,
}

/// 원본의 심볼릭 링크·정션을 처리하는 방식.
///
/// 기본값은 `Skip`이다. 링크를 따라가면 원본 루트 밖으로 탈출하거나 순환 링크를
/// 만날 수 있고, 링크를 다시 만들려면 Windows 권한 정책이 개입하므로 옵션을
/// 선택 모드마다 원본 내부 추적·순환 차단·대상 충돌 보호 경계를 적용한다.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymlinkMode {
    /// 링크를 따라가지 않고 안전하게 건너뛴다.
    #[default]
    Skip,
    /// 원본 내부 링크가 가리키는 대상을 일반 파일·폴더처럼 처리한다.
    Follow,
    /// 원본 내부 링크를 대상 내부의 상대 링크로 재생성한다.
    Recreate,
}

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
    /// 작업 실행을 기다리는 방식. 구버전 설정은 주기 모드로 읽는다.
    #[serde(default)]
    pub watch_mode: WatchMode,
    /// 심볼릭 링크·정션 처리 방식. 구버전 설정은 안전한 `Skip`으로 읽는다.
    #[serde(default)]
    pub symlink_mode: SymlinkMode,
    /// 원본에서 삭제된 파일을 대상에서도 삭제할지 여부.
    #[serde(default)]
    pub mirror_deletes: bool,
    /// 숨김/시스템 속성 파일도 동기화할지 여부. 기본값은 전체 동기화이므로 true.
    #[serde(default = "default_true")]
    pub include_hidden: bool,
    /// 상대 경로 기준으로 동기화에서 제외할 패턴.
    ///
    /// 패턴 해석과 실제 건너뛰기는 D-002~D-005에서 구현한다. 필드를 먼저 설정
    /// 스키마에 포함해도 기존 config.json을 계속 읽을 수 있도록 기본값은 빈 목록이다.
    #[serde(default)]
    pub exclude_patterns: Vec<String>,

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
            watch_mode: WatchMode::Interval,
            symlink_mode: SymlinkMode::Skip,
            mirror_deletes: false,
            include_hidden: true,
            exclude_patterns: Vec::new(),
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

/// VirtualBox 디스크 탐색의 사용자 조정 가능한 마지막 입력값.
///
/// VDI를 자동으로 열거나 파티션을 자동 선택하지는 않는다. 앱 시작 시 입력창에만
/// 복원하고, 사용자가 `VDI 열기`와 파티션 선택을 명시적으로 수행해야 다시 검사한다.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct VirtualDiskConfig {
    #[serde(default)]
    pub last_vdi_path: Option<String>,
    #[serde(default)]
    pub last_partition_number: Option<u32>,
    #[serde(default)]
    pub last_guest_path: Option<String>,
    #[serde(default)]
    pub last_target_path: Option<String>,
    #[serde(default)]
    pub layout: VirtualDiskLayoutConfig,
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
            kakao_reclaim_ad_space: false,
            light_theme_name: None,
            dark_theme_name: None,
            theme_mode: None,
            scan_interval_secs: default_scan_interval_secs(),
            favorite_services: Vec::new(),
            interval_presets: default_interval_presets(),
            sync_enabled: true,
            sync_jobs: Vec::new(),
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            virtual_disk_suppressed_issue_keys: Vec::new(),
            log: LogConfig::default(),
            virtual_disk: VirtualDiskConfig::default(),
            openssl: OpenSslConfig::default(),
            timestamp: TimestampConfig::default(),
            process_monitor: ProcessMonitorConfig::default(),
            clipboard: ClipboardConfig::default(),
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
const SETTINGS_FILE_NAME: &str = "settings.json";
const LEGACY_CONFIG_FILE_NAME: &str = "config.json";

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

#[cfg(test)]
pub(crate) mod tests;
