//! `AppRoot`가 다루는 순수 데이터 타입.
//!
//! 렌더링·I/O 로직 없이 상태 표현만 담는다.

use serde::{Deserialize, Serialize};

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
pub(crate) const NAV_TOOLS: [(ActivePanel, &str, &str); 3] = [
    (ActivePanel::AdBlock, "웹뷰 광고 차단", "카카오톡 등 WebView 광고 숨김"),
    (ActivePanel::FileSync, "파일 동기화", "폴더 → 폴더 주기적 복사"),
    (ActivePanel::Services, "Windows 서비스", "서비스 시작·중지·삭제"),
];

pub(crate) const NAV_SYSTEM: [(ActivePanel, &str, &str); 3] = [
    (ActivePanel::AutoStart, "자동 시작", "로그온 시 자동 실행 등록"),
    (ActivePanel::Logs, "로그", "앱 활동 기록"),
    (ActivePanel::Settings, "설정", "테마 · 로그 보관"),
];
