//! `AppRoot`가 다루는 순수 데이터 타입.
//!
//! 렌더링·I/O 로직 없이 상태 표현만 담는다.

use serde::{Deserialize, Serialize};
use std::sync::{atomic::AtomicBool, Arc};

use crate::config::SyncJob;
use crate::sync::SyncOutcome;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TargetApp {
    pub process_name: String,
    pub display_name: String,
    pub enabled: bool,
    pub ad_window_class: String,
}

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct AppState {
    pub is_active: bool,
    pub is_target_running: bool,
    pub targets: Vec<TargetApp>,
    pub blocked_count: u32,
    pub log_entries: Vec<LogEntry>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            is_active: true,
            is_target_running: false,
            targets: vec![TargetApp {
                process_name: "KakaoTalk.exe".to_string(),
                display_name: "KakaoTalk".to_string(),
                enabled: true,
                ad_window_class: "Chrome_WidgetWin_1".to_string(),
            }],
            blocked_count: 0,
            log_entries: vec![LogEntry {
                level: "INFO".to_string(),
                message: "앱을 시작했습니다.".to_string(),
            }],
        }
    }
}

/// 백그라운드 → UI 방향으로만 흐르는 이벤트.
///
/// UI 이벤트 핸들러도 상태를 직접 고치지 않고 이 채널을 경유해 일관성을 유지한다.
#[derive(Debug)]
pub(crate) enum PlatformEvent {
    AdBlocked,
    TargetStatusChanged(bool),
    ServiceToggled(bool),
    TargetToggled { index: usize, enabled: bool },
    TargetRemoved { index: usize },
    SyncStarted {
        id: String,
        label: String,
    },
    /// 실행 중 진행 상황. 백그라운드가 빈도를 제한해 보내므로 파일마다 오지는 않는다.
    SyncProgress {
        id: String,
        current_path: String,
        copied: usize,
        skipped: usize,
        failed: usize,
    },
    SyncFinished { id: String, label: String, outcome: SyncOutcome },
}

#[derive(Clone, Debug)]
pub(crate) struct ScannerState {
    pub(crate) service_enabled: bool,
    pub(crate) targets: Vec<TargetApp>,
    pub(crate) scan_interval_secs: u32,
}

/// 동기화 스레드와 UI가 공유하는 상태.
#[derive(Debug, Default)]
pub(crate) struct SyncSharedState {
    pub(crate) jobs: Vec<SyncJob>,
    /// 사용자가 '지금 동기화'로 요청한 작업 ID 큐.
    pub(crate) run_now: Vec<String>,
    /// 실행 중인 작업의 중지 요청 플래그.
    ///
    /// 동기화 스레드는 실행 중 뮤텍스를 잡고 있지 않으므로 중지 신호는 뮤텍스가 아니라
    /// 이 원자 플래그로 전달한다. 엔진이 파일 단위로 확인한다.
    pub(crate) cancel: Arc<AtomicBool>,
}

/// 지금 실행 중인 동기화 작업의 진행 상황.
#[derive(Clone, Debug)]
pub struct SyncRunning {
    pub id: String,
    pub label: String,
    /// 마지막으로 보고된 처리 중 파일(원본 기준 상대 경로).
    pub current_path: String,
    pub copied: usize,
    pub skipped: usize,
    pub failed: usize,
    /// 중지를 요청했지만 아직 끝나지 않은 상태.
    pub stopping: bool,
}

impl SyncRunning {
    /// 하단 상태 표시줄에 쓸 진행 요약.
    pub fn counters(&self) -> String {
        format!(
            "복사 {} · 건너뜀 {} · 실패 {}",
            self.copied, self.skipped, self.failed
        )
    }

    /// 하단 상태 표시줄에 넣을 현재 파일 경로.
    ///
    /// 길이 제한은 GPUI `truncate()`에 맡기지 않는다. flex 자식으로 들어가면 폭이
    /// 0으로 잡혀 말줄임표만 남기 때문이다. 대신 여기서 잘라 두고, 어떤 파일인지
    /// 알아볼 수 있도록 앞쪽을 버리고 파일명 쪽을 남긴다.
    pub fn display_path(&self) -> String {
        const MAX_CHARS: usize = 72;

        if self.current_path.is_empty() {
            return "폴더를 검사하는 중…".to_string();
        }

        let chars: Vec<char> = self.current_path.chars().collect();
        if chars.len() <= MAX_CHARS {
            return self.current_path.clone();
        }

        let tail: String = chars[chars.len() - (MAX_CHARS - 1)..].iter().collect();
        format!("…{tail}")
    }
}

/// 동기화 작업 하나의 최근 실행 결과.
#[derive(Clone, Debug, Default)]
pub struct SyncJobStatus {
    pub last_run: Option<String>,
    pub summary: String,
    pub failed: bool,
}

impl SyncJobStatus {
    /// 목록에 한 줄로 표시할 문자열.
    pub fn line(&self) -> String {
        match &self.last_run {
            Some(time) => format!("최근 실행 {time} — {}", self.summary),
            None if !self.summary.is_empty() => self.summary.clone(),
            None => "아직 실행되지 않았습니다.".to_string(),
        }
    }
}

/// 비Windows에서는 Win32 전용 패널 variant가 만들어지지 않는다(아래 `NAV_*` 참고).
/// 렌더 match는 모든 variant를 그대로 다루므로 그 플랫폼에서만 미사용 경고를 푼다.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivePanel {
    Dashboard,
    AdBlock,
    FileSync,
    Services,
    AutoStart,
    Logs,
    Settings,
}

/// 사이드바 항목 정의: (패널, 표시 이름, 보조 설명)
///
/// 웹뷰 광고 차단·Windows 서비스·자동 시작은 Win32와 SCM에 직접 의존해 다른 OS에
/// 대응물이 없다. **그 플랫폼에서는 메뉴 자체를 노출하지 않는다** — 눌러 봐야 미지원
/// 안내만 나오는 항목을 남겨 두면 앱이 고장 난 것처럼 보인다.
#[cfg(target_os = "windows")]
pub(crate) const NAV_TOOLS: [(ActivePanel, &str, &str); 3] = [
    (ActivePanel::AdBlock, "웹뷰 광고 차단", "카카오톡 등 WebView 광고 숨김"),
    (ActivePanel::FileSync, "파일 동기화", "폴더 → 폴더 주기적 복사"),
    (ActivePanel::Services, "Windows 서비스", "서비스 시작·중지·삭제"),
];

#[cfg(not(target_os = "windows"))]
pub(crate) const NAV_TOOLS: [(ActivePanel, &str, &str); 1] = [(
    ActivePanel::FileSync,
    "파일 동기화",
    "폴더 → 폴더 주기적 복사",
)];

#[cfg(target_os = "windows")]
pub(crate) const NAV_SYSTEM: [(ActivePanel, &str, &str); 3] = [
    (ActivePanel::AutoStart, "자동 시작", "로그온 시 자동 실행 등록"),
    (ActivePanel::Logs, "로그", "앱 활동 기록"),
    (ActivePanel::Settings, "설정", "테마 · 로그 보관"),
];

#[cfg(not(target_os = "windows"))]
pub(crate) const NAV_SYSTEM: [(ActivePanel, &str, &str); 2] = [
    (ActivePanel::Logs, "로그", "앱 활동 기록"),
    (ActivePanel::Settings, "설정", "테마 · 로그 보관"),
];
