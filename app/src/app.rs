use gpui::{
    div, px, size, AnyElement, AppContext, Context, Entity, InteractiveElement, IntoElement,
    ParentElement, PathPromptOptions, Render, ScrollHandle, StatefulInteractiveElement, Styled,
    Subscription, Window, WindowControlArea,
};
use gpui_component::{
    h_flex,
    input::{InputEvent, InputState},
    notification::{Notification, NotificationType},
    scroll::{Scrollbar, ScrollbarShow},
    switch::Switch,
    theme::ActiveTheme,
    v_flex, v_virtual_list, VirtualListScrollHandle, WindowExt, TITLE_BAR_HEIGHT,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    future::pending,
    ops::Range,
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::config::{load_config, update_config, LogConfig, SyncJob};
use crate::platform::{NativePlatform, NativeWindowHandle, Platform, SysServiceInfo};
use crate::sync::{run_sync_job, SyncFailure, SyncOutcome};
use crate::window::{ad_block, file_sync, service_mgr, service_view, settings};

#[cfg(target_os = "windows")]
use crate::platform::{
    hide_main_window_to_tray, set_tray_service_active, set_tray_toggle_handler,
};

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
enum PlatformEvent {
    AdBlocked,
    TargetStatusChanged(bool),
    ServiceToggled(bool),
    TargetToggled { index: usize, enabled: bool },
    TargetRemoved { index: usize },
    SyncFinished { id: String, label: String, outcome: SyncOutcome },
}

#[derive(Clone, Debug)]
struct ScannerState {
    service_enabled: bool,
    targets: Vec<TargetApp>,
    scan_interval_secs: u32,
}

/// 동기화 스레드와 UI가 공유하는 상태.
#[derive(Debug, Default)]
struct SyncSharedState {
    jobs: Vec<SyncJob>,
    /// 사용자가 '지금 동기화'로 요청한 작업 ID 큐.
    run_now: Vec<String>,
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
const NAV_TOOLS: [(ActivePanel, &str, &str); 3] = [
    (ActivePanel::AdBlock, "웹뷰 광고 차단", "카카오톡 등 WebView 광고 숨김"),
    (ActivePanel::FileSync, "파일 동기화", "폴더 → 폴더 주기적 복사"),
    (ActivePanel::Services, "Windows 서비스", "서비스 시작·중지·삭제"),
];

const NAV_SYSTEM: [(ActivePanel, &str, &str); 3] = [
    (ActivePanel::AutoStart, "자동 시작", "로그온 시 자동 실행 등록"),
    (ActivePanel::Logs, "로그", "앱 활동 기록"),
    (ActivePanel::Settings, "설정", "테마 · 로그 보관"),
];

pub struct AppRoot {
    active_panel: ActivePanel,
    app_state: AppState,
    pub(crate) theme_filter_query: String,
    pub(crate) theme_filter_active_only: bool,
    pub(crate) theme_filter_input: Option<Entity<InputState>>,
    pub(crate) running_processes: Vec<String>,
    pub(crate) platform: Arc<dyn Platform>,
    event_tx: UnboundedSender<PlatformEvent>,
    event_rx: UnboundedReceiver<PlatformEvent>,
    log_scroll_handle: VirtualListScrollHandle,
    scanner_state: Arc<Mutex<ScannerState>>,
    subscriptions: Vec<Subscription>,
    pub(crate) scan_interval_secs: u32,

    // ── 서비스 관리 ──
    pub(crate) sys_services: Vec<SysServiceInfo>,
    pub(crate) service_search_query: String,
    pub(crate) service_search_input: Option<Entity<InputState>>,
    pub(crate) svc_scroll_handle: VirtualListScrollHandle,
    pub(crate) svc_right_scroll: ScrollHandle,
    pub(crate) pending_delete_service: Option<String>,
    pub(crate) service_filter: ServiceFilter,
    pub(crate) favorite_services: Vec<String>,

    // ── 파일 동기화 ──
    pub(crate) sync_jobs: Vec<SyncJob>,
    pub(crate) selected_sync_job: Option<usize>,
    /// 작업 ID → 최근 실행 결과. 인덱스 대신 ID로 매칭해 삭제 시 어긋나지 않게 한다.
    pub(crate) sync_status: HashMap<String, SyncJobStatus>,
    pub(crate) sync_failures: Vec<SyncFailure>,
    pub(crate) suppressed_sync_failures: HashSet<String>,
    pub(crate) sync_notify_enabled: bool,
    pub(crate) sync_name_input: Option<Entity<InputState>>,
    pub(crate) sync_source_input: Option<Entity<InputState>>,
    pub(crate) sync_target_input: Option<Entity<InputState>>,
    pub(crate) sync_left_scroll: ScrollHandle,
    pub(crate) sync_right_scroll: ScrollHandle,
    sync_state: Arc<Mutex<SyncSharedState>>,

    // ── 광고 차단 패널 ──
    pub(crate) ad_left_scroll: ScrollHandle,
    pub(crate) ad_right_scroll: ScrollHandle,

    // ── 로그 설정 ──
    pub(crate) log_config: LogConfig,

    content_scroll_handle: ScrollHandle,
}

/// 서비스 목록 상태 필터.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ServiceFilter {
    #[default]
    All,
    Running,
    Stopped,
    Favorites,
}

impl AppRoot {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        let platform: Arc<dyn Platform> = Arc::new(NativePlatform::new());
        let mut app_state = AppState::default();
        let mut initial_scan_interval_secs: u32 = 10;
        let mut sync_jobs = Vec::new();
        let mut favorite_services = Vec::new();
        let mut log_config = LogConfig::default();

        if let Ok(Some(cfg)) = load_config() {
            app_state.is_active = cfg.service_enabled;
            if !cfg.targets.is_empty() {
                app_state.targets = cfg.targets;
            }
            initial_scan_interval_secs = cfg.scan_interval_secs.max(1);
            sync_jobs = cfg.sync_jobs;
            // 구버전 config에는 작업 ID가 없으므로 여기서 채운다.
            for job in sync_jobs.iter_mut() {
                job.ensure_id();
            }
            favorite_services = cfg.favorite_services;
            log_config = cfg.log;
            app_state.log_entries.push(LogEntry {
                level: "INFO".to_string(),
                message: "설정 파일을 불러왔습니다.".to_string(),
            });
        }

        let initial_running = app_state
            .targets
            .iter()
            .filter(|t| t.enabled)
            .any(|t| platform.is_target_running(&t.process_name));
        app_state.is_target_running = initial_running;

        let (event_tx, event_rx) = unbounded_channel();
        let scanner_state = Arc::new(Mutex::new(ScannerState {
            service_enabled: app_state.is_active,
            targets: app_state.targets.clone(),
            scan_interval_secs: initial_scan_interval_secs,
        }));
        let sync_state = Arc::new(Mutex::new(SyncSharedState {
            jobs: sync_jobs.clone(),
            run_now: Vec::new(),
        }));

        #[cfg(target_os = "windows")]
        {
            set_tray_service_active(app_state.is_active);

            let event_tx_for_tray = event_tx.clone();
            let scanner_state_for_tray = Arc::clone(&scanner_state);
            set_tray_toggle_handler(Arc::new(move |enabled| {
                if let Ok(mut state) = scanner_state_for_tray.lock() {
                    state.service_enabled = enabled;
                }

                if let Err(err) = update_config(|cfg| cfg.service_enabled = enabled) {
                    log::error!("트레이 토글 설정 저장 실패: {err}");
                }

                let _ = event_tx_for_tray.send(PlatformEvent::ServiceToggled(enabled));
            }));
        }

        Self::spawn_platform_loop(
            Arc::clone(&platform),
            event_tx.clone(),
            Arc::clone(&scanner_state),
        );
        Self::spawn_sync_loop(event_tx.clone(), Arc::clone(&sync_state));

        let running_processes = platform.list_running_processes().unwrap_or_default();
        let selected_sync_job = (!sync_jobs.is_empty()).then_some(0);

        Self {
            active_panel: ActivePanel::Dashboard,
            app_state,
            theme_filter_query: String::new(),
            theme_filter_active_only: false,
            theme_filter_input: None,
            running_processes,
            platform,
            event_tx,
            event_rx,
            log_scroll_handle: VirtualListScrollHandle::new(),
            scanner_state,
            subscriptions: Vec::new(),
            scan_interval_secs: initial_scan_interval_secs,

            sys_services: Vec::new(),
            service_search_query: String::new(),
            service_search_input: None,
            svc_scroll_handle: VirtualListScrollHandle::new(),
            svc_right_scroll: ScrollHandle::default(),
            pending_delete_service: None,
            service_filter: ServiceFilter::All,
            favorite_services,

            sync_jobs,
            selected_sync_job,
            sync_status: HashMap::new(),
            sync_failures: Vec::new(),
            suppressed_sync_failures: HashSet::new(),
            sync_notify_enabled: true,
            sync_name_input: None,
            sync_source_input: None,
            sync_target_input: None,
            sync_left_scroll: ScrollHandle::default(),
            sync_right_scroll: ScrollHandle::default(),
            sync_state,

            ad_left_scroll: ScrollHandle::default(),
            ad_right_scroll: ScrollHandle::default(),

            log_config,

            content_scroll_handle: ScrollHandle::default(),
        }
    }

    pub(crate) fn app_state(&self) -> &AppState {
        &self.app_state
    }

    // ─────────────────────────────────────────────
    // 입력 위젯 준비
    // ─────────────────────────────────────────────

    pub(crate) fn ensure_theme_filter_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.theme_filter_input.is_some() {
            return;
        }

        let initial_query = self.theme_filter_query.clone();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("테마 이름으로 검색")
                .default_value(initial_query)
        });

        let subscription = cx.subscribe(
            &input,
            |this: &mut Self, input: Entity<InputState>, ev: &InputEvent, cx| {
                if let InputEvent::Change = ev {
                    this.theme_filter_query = input.read(cx).value().to_string();
                    cx.notify();
                }
            },
        );

        self.theme_filter_input = Some(input);
        self.subscriptions.push(subscription);
    }

    pub(crate) fn set_theme_filter_query(
        &mut self,
        query: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.theme_filter_query = query.clone();

        if let Some(input) = self.theme_filter_input.as_ref() {
            input.update(cx, |state, cx| {
                state.set_value(query, window, cx);
            });
        }
    }

    pub(crate) fn ensure_service_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.service_search_input.is_some() {
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("서비스 이름 검색"));
        let subscription = cx.subscribe(
            &input,
            |this: &mut Self, input: Entity<InputState>, ev: &InputEvent, cx| {
                if let InputEvent::Change = ev {
                    this.service_search_query = input.read(cx).value().to_string();
                    cx.notify();
                }
            },
        );
        self.service_search_input = Some(input);
        self.subscriptions.push(subscription);
    }

    /// 동기화 작업 편집용 입력 3종을 준비한다.
    pub(crate) fn ensure_sync_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sync_name_input.is_some() {
            return;
        }

        let job = self
            .selected_sync_job
            .and_then(|ix| self.sync_jobs.get(ix))
            .cloned()
            .unwrap_or_default();

        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("작업 이름 (비우면 원본 폴더명)")
                .default_value(job.name.clone())
        });
        let source = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(r"예: D:\작업\원본")
                .default_value(job.source.clone())
        });
        let target = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(r"예: E:\백업\대상")
                .default_value(job.target.clone())
        });

        // 이름은 입력 즉시 반영한다(경로는 '경로 적용'으로 명시 저장).
        let name_sub = cx.subscribe(
            &name,
            |this: &mut Self, input: Entity<InputState>, ev: &InputEvent, cx| {
                if let InputEvent::Change = ev {
                    let value = input.read(cx).value().to_string();
                    if let Some(ix) = this.selected_sync_job {
                        if let Some(job) = this.sync_jobs.get_mut(ix) {
                            job.name = value;
                        }
                        this.persist_sync_jobs();
                        cx.notify();
                    }
                }
            },
        );

        self.sync_name_input = Some(name);
        self.sync_source_input = Some(source);
        self.sync_target_input = Some(target);
        self.subscriptions.push(name_sub);
    }

    // ─────────────────────────────────────────────
    // 백그라운드 루프
    // ─────────────────────────────────────────────

    fn spawn_platform_loop(
        platform: Arc<dyn Platform>,
        event_tx: UnboundedSender<PlatformEvent>,
        scanner_state: Arc<Mutex<ScannerState>>,
    ) {
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build();

            let Ok(runtime) = runtime else {
                return;
            };

            runtime.block_on(async move {
                tokio::spawn(async move {
                    let mut last_running: Option<bool> = None;
                    let mut last_hidden: Option<NativeWindowHandle> = None;

                    loop {
                        let snapshot = scanner_state
                            .lock()
                            .ok()
                            .map(|s| (s.service_enabled, s.targets.clone(), s.scan_interval_secs));

                        let Some((service_enabled, targets, interval_secs)) = snapshot else {
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            continue;
                        };

                        let sleep_duration = Duration::from_secs(interval_secs.max(1) as u64);

                        if !service_enabled {
                            if last_running != Some(false) {
                                let _ = event_tx.send(PlatformEvent::TargetStatusChanged(false));
                                last_running = Some(false);
                            }
                            tokio::time::sleep(sleep_duration).await;
                            continue;
                        }

                        let mut any_running = false;
                        let mut detected_handle: Option<NativeWindowHandle> = None;

                        for target in targets.iter().filter(|t| t.enabled) {
                            if !platform.is_target_running(&target.process_name) {
                                continue;
                            }

                            any_running = true;

                            if let Ok(Some(hwnd)) = platform.find_ad_window(&target.process_name) {
                                detected_handle = Some(hwnd);
                                break;
                            }
                        }

                        if let Some(hwnd) = detected_handle {
                            let _ = platform.hide_ad(hwnd);
                            if last_hidden != Some(hwnd) {
                                let _ = event_tx.send(PlatformEvent::AdBlocked);
                            }
                            last_hidden = Some(hwnd);
                        } else if let Some(hwnd) = last_hidden {
                            let _ = platform.show_ad(hwnd);
                            last_hidden = None;
                        }

                        if last_running != Some(any_running) {
                            let _ = event_tx.send(PlatformEvent::TargetStatusChanged(any_running));
                            last_running = Some(any_running);
                        }

                        tokio::time::sleep(sleep_duration).await;
                    }
                });

                pending::<()>().await;
            });
        });
    }

    /// 동기화 전용 백그라운드 스레드.
    ///
    /// 1초마다 깨어나 (1) 수동 실행 요청과 (2) 주기가 도래한 자동 작업을 처리한다.
    /// 파일 I/O는 블로킹이므로 광고 스캔 루프와 스레드를 분리했다.
    fn spawn_sync_loop(
        event_tx: UnboundedSender<PlatformEvent>,
        sync_state: Arc<Mutex<SyncSharedState>>,
    ) {
        std::thread::spawn(move || {
            // 작업 인덱스는 추가·삭제로 밀리므로 실행 주기는 반드시 ID로 추적한다.
            let mut last_run: HashMap<String, Instant> = HashMap::new();

            loop {
                std::thread::sleep(Duration::from_secs(1));

                let (jobs, manual) = {
                    let Ok(mut state) = sync_state.lock() else {
                        continue;
                    };
                    let manual = std::mem::take(&mut state.run_now);
                    (state.jobs.clone(), manual)
                };

                // 삭제된 작업의 기록은 정리한다.
                last_run.retain(|id, _| jobs.iter().any(|job| &job.id == id));

                for job in jobs.iter() {
                    let manual_requested = manual.contains(&job.id);

                    let due = match last_run.get(&job.id) {
                        Some(prev) => {
                            prev.elapsed() >= Duration::from_secs(job.interval_secs.max(1) as u64)
                        }
                        None => true,
                    };

                    if !manual_requested && (!job.enabled || !due) {
                        continue;
                    }

                    let outcome = run_sync_job(job);
                    last_run.insert(job.id.clone(), Instant::now());

                    let label = job.label();
                    if outcome.has_failures() {
                        log::warn!("동기화 '{label}' 완료(실패 포함): {}", outcome.summary());
                        for failure in &outcome.failures {
                            log::warn!("  · {} — {}", failure.path, failure.reason);
                        }
                    } else if outcome.copied > 0 || outcome.deleted > 0 {
                        log::info!("동기화 '{label}' 완료: {}", outcome.summary());
                    }

                    let _ = event_tx.send(PlatformEvent::SyncFinished {
                        id: job.id.clone(),
                        label,
                        outcome,
                    });
                }
            }
        });
    }

    // ─────────────────────────────────────────────
    // 광고 차단 상태 조작
    // ─────────────────────────────────────────────

    fn refresh_target_status(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let running = self
            .app_state
            .targets
            .iter()
            .filter(|t| t.enabled)
            .any(|t| self.platform.is_target_running(&t.process_name));

        let _ = self
            .event_tx
            .send(PlatformEvent::TargetStatusChanged(running));
        self.process_pending_events(window, cx);
    }

    fn sync_scanner_state(&self) {
        if let Ok(mut state) = self.scanner_state.lock() {
            state.service_enabled = self.app_state.is_active;
            state.targets = self.app_state.targets.clone();
            state.scan_interval_secs = self.scan_interval_secs;
        }
    }

    fn persist_config(&self) {
        let is_active = self.app_state.is_active;
        let targets = self.app_state.targets.clone();
        let interval = self.scan_interval_secs;

        if let Err(err) = update_config(move |cfg| {
            cfg.service_enabled = is_active;
            cfg.targets = targets;
            cfg.scan_interval_secs = interval;
        }) {
            log::error!("설정 저장 실패: {err}");
        }
    }

    pub fn set_scan_interval(&mut self, secs: u32, cx: &mut Context<Self>) {
        let secs = secs.max(1);
        self.scan_interval_secs = secs;
        self.sync_scanner_state();
        self.persist_config();
        self.push_log("INFO", format!("스캔 주기를 {secs}초로 변경했습니다."));
        cx.notify();
    }

    pub(crate) fn set_service_enabled(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = self.event_tx.send(PlatformEvent::ServiceToggled(enabled));
        self.process_pending_events(window, cx);
        cx.notify();
    }

    pub(crate) fn set_target_enabled(
        &mut self,
        index: usize,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = self
            .event_tx
            .send(PlatformEvent::TargetToggled { index, enabled });
        self.process_pending_events(window, cx);
        cx.notify();
    }

    pub(crate) fn remove_target(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = self.event_tx.send(PlatformEvent::TargetRemoved { index });
        self.process_pending_events(window, cx);
        cx.notify();
    }

    pub(crate) fn refresh_running_processes(&mut self) {
        match self.platform.list_running_processes() {
            Ok(processes) => self.running_processes = processes,
            Err(err) => log::error!("실행 중인 프로세스 조회 실패: {err}"),
        }
    }

    pub(crate) fn add_target_process(
        &mut self,
        process_name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .app_state
            .targets
            .iter()
            .any(|target| target.process_name.eq_ignore_ascii_case(process_name))
        {
            self.notify_toast("이미 등록된 타겟입니다", NotificationType::Info, window, cx);
            return;
        }

        let display_name = process_name
            .strip_suffix(".exe")
            .unwrap_or(process_name)
            .to_string();

        self.app_state.targets.push(TargetApp {
            process_name: process_name.to_string(),
            display_name,
            enabled: true,
            ad_window_class: "auto:webview".to_string(),
        });

        self.sync_scanner_state();
        self.persist_config();
        self.refresh_target_status(window, cx);
        self.refresh_running_processes();
        self.push_log("INFO", format!("타겟을 추가했습니다: {process_name}"));
        self.notify_toast("타겟을 추가했습니다", NotificationType::Success, window, cx);
        cx.notify();
    }

    // ─────────────────────────────────────────────
    // 서비스 관리
    // ─────────────────────────────────────────────

    pub(crate) fn refresh_sys_services(&mut self) {
        match self.platform.list_sys_services() {
            Ok(services) => self.sys_services = services,
            Err(err) => log::error!("서비스 목록 조회 실패: {err}"),
        }
    }

    pub(crate) fn toggle_favorite_service(&mut self, name: &str) {
        if let Some(pos) = self.favorite_services.iter().position(|n| n == name) {
            self.favorite_services.remove(pos);
        } else {
            self.favorite_services.push(name.to_string());
        }

        let favorites = self.favorite_services.clone();
        if let Err(err) = update_config(move |cfg| cfg.favorite_services = favorites) {
            log::error!("즐겨찾기 저장 실패: {err}");
        }
    }

    pub(crate) fn is_favorite_service(&self, name: &str) -> bool {
        self.favorite_services.iter().any(|n| n == name)
    }

    /// 서비스 뷰에서 발생하는 동작 결과를 로그 패널에 기록한다.
    pub fn push_service_log(&mut self, message: &str, _window: &mut Window, cx: &mut Context<Self>) {
        self.push_log("INFO", message.to_string());
        cx.notify();
    }

    // ─────────────────────────────────────────────
    // 파일 동기화
    // ─────────────────────────────────────────────

    fn persist_sync_jobs(&mut self) {
        let jobs = self.sync_jobs.clone();

        if let Ok(mut state) = self.sync_state.lock() {
            state.jobs = jobs.clone();
        }

        if let Err(err) = update_config(move |cfg| cfg.sync_jobs = jobs) {
            log::error!("동기화 작업 저장 실패: {err}");
        }
    }

    pub(crate) fn add_sync_job(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_jobs.push(SyncJob::default());
        let index = self.sync_jobs.len() - 1;
        self.persist_sync_jobs();
        self.select_sync_job(index, window, cx);
        self.push_log("INFO", "동기화 작업을 추가했습니다.".to_string());
        cx.notify();
    }

    pub(crate) fn remove_sync_job(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.sync_jobs.len() {
            return;
        }

        let label = self.sync_jobs[index].label();
        let removed = self.sync_jobs.remove(index);
        self.sync_status.remove(&removed.id);

        self.selected_sync_job = if self.sync_jobs.is_empty() {
            None
        } else {
            Some(index.min(self.sync_jobs.len() - 1))
        };

        self.persist_sync_jobs();
        if let Some(next) = self.selected_sync_job {
            self.load_sync_inputs(next, window, cx);
        }
        self.push_log("INFO", format!("동기화 작업을 삭제했습니다: {label}"));
        cx.notify();
    }

    pub(crate) fn select_sync_job(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.sync_jobs.len() {
            return;
        }
        self.selected_sync_job = Some(index);
        self.load_sync_inputs(index, window, cx);
        cx.notify();
    }

    /// 선택한 작업의 값을 입력 위젯에 채운다.
    fn load_sync_inputs(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(job) = self.sync_jobs.get(index).cloned() else {
            return;
        };

        if let Some(input) = self.sync_name_input.as_ref() {
            input.update(cx, |state, cx| state.set_value(job.name.clone(), window, cx));
        }
        if let Some(input) = self.sync_source_input.as_ref() {
            input.update(cx, |state, cx| {
                state.set_value(job.source.clone(), window, cx)
            });
        }
        if let Some(input) = self.sync_target_input.as_ref() {
            input.update(cx, |state, cx| {
                state.set_value(job.target.clone(), window, cx)
            });
        }
    }

    /// 선택한 작업을 수정하고 저장한다.
    pub(crate) fn update_selected_sync_job(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut SyncJob),
    ) {
        let Some(index) = self.selected_sync_job else {
            return;
        };
        let Some(job) = self.sync_jobs.get_mut(index) else {
            return;
        };
        edit(job);
        self.persist_sync_jobs();
        cx.notify();
    }

    /// 입력창의 경로 텍스트를 선택한 작업에 반영한다.
    pub(crate) fn apply_sync_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.selected_sync_job else {
            return;
        };

        let source = self
            .sync_source_input
            .as_ref()
            .map(|i| i.read(cx).value().to_string())
            .unwrap_or_default();
        let target = self
            .sync_target_input
            .as_ref()
            .map(|i| i.read(cx).value().to_string())
            .unwrap_or_default();

        if let Some(job) = self.sync_jobs.get_mut(index) {
            job.source = source.trim().to_string();
            job.target = target.trim().to_string();
        }

        self.persist_sync_jobs();
        self.push_log("INFO", "동기화 경로를 저장했습니다.".to_string());
        self.notify_toast("경로를 저장했습니다", NotificationType::Success, window, cx);
        cx.notify();
    }

    /// 네이티브 폴더 선택 대화상자를 띄워 경로를 채운다.
    pub(crate) fn pick_sync_folder(
        &mut self,
        is_source: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_sync_job.is_none() {
            return;
        }

        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(if is_source {
                "원본 폴더 선택".into()
            } else {
                "대상 폴더 선택".into()
            }),
        });

        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let picked = path.display().to_string();

            let _ = this.update_in(cx, |this, window, cx| {
                let Some(index) = this.selected_sync_job else {
                    return;
                };

                if let Some(job) = this.sync_jobs.get_mut(index) {
                    if is_source {
                        job.source = picked.clone();
                    } else {
                        job.target = picked.clone();
                    }
                }

                let input = if is_source {
                    this.sync_source_input.clone()
                } else {
                    this.sync_target_input.clone()
                };
                if let Some(input) = input {
                    input.update(cx, |state, cx| state.set_value(picked.clone(), window, cx));
                }

                this.persist_sync_jobs();
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn request_sync_job(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.sync_jobs.get(index).map(|job| job.id.clone()) else {
            return;
        };

        if let Ok(mut state) = self.sync_state.lock() {
            if !state.run_now.contains(&id) {
                state.run_now.push(id);
            }
        }
        self.notify_toast("동기화를 시작했습니다", NotificationType::Info, window, cx);
        cx.notify();
    }

    pub(crate) fn request_sync_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.sync_jobs.len();
        if count == 0 {
            self.notify_toast(
                "등록된 동기화 작업이 없습니다",
                NotificationType::Warning,
                window,
                cx,
            );
            return;
        }

        if let Ok(mut state) = self.sync_state.lock() {
            state.run_now = self.sync_jobs.iter().map(|job| job.id.clone()).collect();
        }
        self.notify_toast(
            "전체 동기화를 시작했습니다",
            NotificationType::Info,
            window,
            cx,
        );
        cx.notify();
    }

    pub(crate) fn toggle_sync_failure_suppression(&mut self, key: &str) {
        if !self.suppressed_sync_failures.remove(key) {
            self.suppressed_sync_failures.insert(key.to_string());
        }
    }

    // ─────────────────────────────────────────────
    // 로그 설정
    // ─────────────────────────────────────────────

    pub(crate) fn update_log_config(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut LogConfig),
    ) {
        edit(&mut self.log_config);
        crate::logging::update_config(self.log_config.clone());

        let log_config = self.log_config.clone();
        if let Err(err) = update_config(move |cfg| cfg.log = log_config) {
            log::error!("로그 설정 저장 실패: {err}");
        }

        self.push_log("INFO", "로그 설정을 변경했습니다.".to_string());
        cx.notify();
    }

    // ─────────────────────────────────────────────
    // 공통 유틸
    // ─────────────────────────────────────────────

    fn push_log(&mut self, level: &str, message: String) {
        self.app_state.log_entries.push(LogEntry {
            level: level.to_string(),
            message,
        });
        // UI 로그 패널이 무한정 자라지 않도록 상한을 둔다(파일 로그는 별도 보존).
        const MAX_UI_LOG_ENTRIES: usize = 2000;
        if self.app_state.log_entries.len() > MAX_UI_LOG_ENTRIES {
            let excess = self.app_state.log_entries.len() - MAX_UI_LOG_ENTRIES;
            self.app_state.log_entries.drain(0..excess);
        }
        self.log_scroll_handle.scroll_to_bottom();
    }

    fn notify_toast(
        &self,
        message: &'static str,
        kind: NotificationType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.push_notification(
            Notification::new()
                .message(message)
                .with_type(kind)
                .title("gpui-convenience-tools"),
            cx,
        );
    }

    fn process_pending_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                PlatformEvent::AdBlocked => {
                    self.app_state.blocked_count += 1;
                    self.push_log("SUCCESS", "광고 창을 감지하여 숨겼습니다.".to_string());
                    self.notify_toast("광고를 차단했습니다", NotificationType::Success, window, cx);
                }
                PlatformEvent::TargetStatusChanged(is_running) => {
                    self.app_state.is_target_running = is_running;
                    self.push_log(
                        "INFO",
                        if is_running {
                            "타겟 상태: 실행 중".to_string()
                        } else {
                            "타겟 상태: 미실행".to_string()
                        },
                    );
                }
                PlatformEvent::ServiceToggled(enabled) => {
                    self.app_state.is_active = enabled;
                    self.sync_scanner_state();
                    self.persist_config();
                    #[cfg(target_os = "windows")]
                    set_tray_service_active(enabled);
                    self.push_log(
                        "INFO",
                        if enabled {
                            "광고 차단을 켰습니다.".to_string()
                        } else {
                            "광고 차단을 껐습니다.".to_string()
                        },
                    );
                    self.notify_toast(
                        if enabled {
                            "광고 차단을 켰습니다"
                        } else {
                            "광고 차단을 껐습니다"
                        },
                        NotificationType::Info,
                        window,
                        cx,
                    );
                }
                PlatformEvent::TargetToggled { index, enabled } => {
                    let message = if let Some(target) = self.app_state.targets.get_mut(index) {
                        target.enabled = enabled;
                        format!("{} 활성 상태 변경: {}", target.process_name, enabled)
                    } else {
                        continue;
                    };

                    self.sync_scanner_state();
                    self.persist_config();
                    self.push_log("INFO", message);
                }
                PlatformEvent::TargetRemoved { index } => {
                    if index < self.app_state.targets.len() {
                        let name = self.app_state.targets.remove(index).process_name;
                        self.sync_scanner_state();
                        self.persist_config();
                        self.push_log("INFO", format!("타겟을 삭제했습니다: {name}"));
                    }
                }
                PlatformEvent::SyncFinished { id, label, outcome } => {
                    self.handle_sync_finished(id, label, outcome, window, cx);
                }
            }
        }
    }

    fn handle_sync_finished(
        &mut self,
        id: String,
        label: String,
        outcome: SyncOutcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 실행 중 작업이 삭제됐을 수 있으므로 아직 존재할 때만 상태를 반영한다.
        if !self.sync_jobs.iter().any(|job| job.id == id) {
            return;
        }

        self.sync_status.insert(
            id,
            SyncJobStatus {
                last_run: Some(crate::logging::now_hms()),
                summary: outcome.summary(),
                failed: outcome.has_failures(),
            },
        );

        if outcome.copied > 0 || outcome.deleted > 0 {
            self.push_log("INFO", format!("[{label}] {}", outcome.summary()));
        }

        // ── 실패 처리: 로그에는 항상 남기고, 토스트는 억제되지 않은 새 항목만 ──
        let mut unsuppressed_new = 0usize;
        for failure in &outcome.failures {
            let key = failure.key();
            let already_known = self.sync_failures.iter().any(|f| f.key() == key);

            if !already_known {
                self.app_state.log_entries.push(LogEntry {
                    level: "ERROR".to_string(),
                    message: format!("[{label}] {} — {}", failure.path, failure.reason),
                });
                self.sync_failures.push(failure.clone());
            }

            if !already_known && !self.suppressed_sync_failures.contains(&key) {
                unsuppressed_new += 1;
            }
        }

        if outcome.truncated_failures > 0 {
            self.push_log(
                "WARN",
                format!(
                    "[{label}] 실패 항목이 많아 {}건은 목록에서 생략했습니다.",
                    outcome.truncated_failures
                ),
            );
        }

        // 기록 상한
        const MAX_TRACKED_FAILURES: usize = 300;
        if self.sync_failures.len() > MAX_TRACKED_FAILURES {
            let excess = self.sync_failures.len() - MAX_TRACKED_FAILURES;
            self.sync_failures.drain(0..excess);
        }
        self.log_scroll_handle.scroll_to_bottom();

        if unsuppressed_new > 0 && self.sync_notify_enabled {
            window.push_notification(
                Notification::new()
                    .message(format!(
                        "'{label}' 동기화 중 {unsuppressed_new}건을 복사하지 못했습니다. 파일 동기화 패널에서 사유를 확인하세요."
                    ))
                    .with_type(NotificationType::Warning)
                    .title("동기화 실패"),
                cx,
            );
        }

        cx.notify();
    }

    // ─────────────────────────────────────────────
    // 렌더링
    // ─────────────────────────────────────────────

    fn render_dashboard_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();

        let (svc_label, svc_bg, svc_fg) = if self.app_state.is_active {
            ("광고 차단: 동작 중", theme.success, theme.success_foreground)
        } else {
            ("광고 차단: 중지됨", theme.warning, theme.warning_foreground)
        };

        let (tgt_label, tgt_bg, tgt_fg) = if self.app_state.is_target_running {
            ("타겟: 실행 중", theme.success, theme.success_foreground)
        } else {
            ("타겟: 미실행", theme.muted, theme.muted_foreground)
        };

        let active_targets = self.app_state.targets.iter().filter(|t| t.enabled).count();
        let auto_sync_jobs = self.sync_jobs.iter().filter(|j| j.enabled).count();
        let failed_syncs = self.sync_status.values().filter(|s| s.failed).count();

        let recent: Vec<_> = self.app_state.log_entries.iter().rev().take(6).collect();
        let mut activity = v_flex().gap_1();
        if recent.is_empty() {
            activity = activity.child(div().text_color(theme.muted_foreground).child("활동 없음"));
        } else {
            for entry in &recent {
                let level_color = match entry.level.as_str() {
                    "SUCCESS" => theme.success,
                    "WARN" => theme.warning,
                    "ERROR" => theme.danger,
                    _ => theme.info,
                };
                activity = activity.child(
                    h_flex()
                        .gap_2()
                        .child(
                            div()
                                .w(px(64.0))
                                .text_color(level_color)
                                .child(entry.level.clone()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_color(theme.muted_foreground)
                                .child(entry.message.clone()),
                        ),
                );
            }
        }

        v_flex()
            .w_full()
            .gap_3()
            .child(div().text_color(theme.foreground).child("대시보드"))
            .child(
                div()
                    .rounded_lg()
                    .p_4()
                    .bg(theme.secondary)
                    .border_1()
                    .border_color(theme.border)
                    .child(
                        v_flex()
                            .gap_3()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .rounded_md()
                                            .px_3()
                                            .py_1()
                                            .bg(svc_bg)
                                            .text_color(svc_fg)
                                            .child(svc_label),
                                    )
                                    .child(
                                        div()
                                            .rounded_md()
                                            .px_3()
                                            .py_1()
                                            .bg(tgt_bg)
                                            .text_color(tgt_fg)
                                            .child(tgt_label),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_3()
                                    .child(self.stat_tile(
                                        "누적 차단",
                                        &self.app_state.blocked_count.to_string(),
                                        cx,
                                    ))
                                    .child(self.stat_tile(
                                        "활성 타겟",
                                        &format!("{active_targets} / {}", self.app_state.targets.len()),
                                        cx,
                                    ))
                                    .child(self.stat_tile(
                                        "자동 동기화",
                                        &format!("{auto_sync_jobs} / {}", self.sync_jobs.len()),
                                        cx,
                                    ))
                                    .child(self.stat_tile(
                                        "동기화 실패",
                                        &failed_syncs.to_string(),
                                        cx,
                                    )),
                            ),
                    ),
            )
            .child(
                div()
                    .rounded_lg()
                    .p_4()
                    .bg(theme.secondary)
                    .border_1()
                    .border_color(theme.border)
                    .child(
                        v_flex()
                            .gap_2()
                            .child(div().text_color(theme.foreground).child("최근 활동"))
                            .child(activity),
                    ),
            )
            .into_any_element()
    }

    fn stat_tile(&self, label: &'static str, value: &str, cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();
        div()
            .flex_1()
            .rounded_md()
            .px_3()
            .py_3()
            .bg(theme.list)
            .border_1()
            .border_color(theme.border)
            .child(
                v_flex()
                    .gap_1()
                    .child(div().text_color(theme.muted_foreground).child(label))
                    .child(div().text_color(theme.foreground).child(value.to_string())),
            )
            .into_any_element()
    }

    fn render_log_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();

        if self.app_state.log_entries.is_empty() {
            return v_flex()
                .size_full()
                .gap_3()
                .child(div().text_color(theme.foreground).child("로그"))
                .child(
                    div()
                        .rounded_lg()
                        .p_4()
                        .bg(theme.secondary)
                        .border_1()
                        .border_color(theme.border)
                        .text_color(theme.muted_foreground)
                        .child("기록된 로그가 없습니다."),
                )
                .into_any_element();
        }

        let item_sizes = Rc::new(
            self.app_state
                .log_entries
                .iter()
                .map(|_| size(px(0.), px(30.0)))
                .collect::<Vec<_>>(),
        );

        let scroll = self.log_scroll_handle.clone();
        let log_path = crate::logging::current_log_file();
        let (file_count, total_bytes) = crate::logging::log_dir_stats();

        v_flex()
            .size_full()
            .gap_3()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_color(theme.foreground).child("로그"))
                    .child(
                        div().text_color(theme.muted_foreground).child(format!(
                            "파일 {file_count}개 · {:.1} MB · {}",
                            total_bytes as f64 / (1024.0 * 1024.0),
                            log_path.display()
                        )),
                    ),
            )
            .child(
                div()
                    .rounded_lg()
                    .p_2()
                    .bg(theme.secondary)
                    .border_1()
                    .border_color(theme.border)
                    .size_full()
                    .child(
                        v_virtual_list(
                            cx.entity(),
                            "event-log-virtual-list",
                            item_sizes,
                            move |this, visible_range: Range<usize>, _window, cx| {
                                visible_range
                                    .map(|ix| {
                                        let Some(entry) = this.app_state.log_entries.get(ix) else {
                                            return div();
                                        };

                                        let level_color = match entry.level.as_str() {
                                            "SUCCESS" => cx.theme().success,
                                            "WARN" => cx.theme().warning,
                                            "ERROR" => cx.theme().danger,
                                            _ => cx.theme().info,
                                        };

                                        div()
                                            .h(px(28.0))
                                            .px_2()
                                            .py_1()
                                            .border_b_1()
                                            .border_color(cx.theme().border)
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .w(px(72.0))
                                                            .text_color(level_color)
                                                            .child(entry.level.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .min_w_0()
                                                            .text_color(cx.theme().foreground)
                                                            .child(entry.message.clone()),
                                                    ),
                                            )
                                    })
                                    .collect::<Vec<_>>()
                            },
                        )
                        .track_scroll(&scroll),
                    ),
            )
            .into_any_element()
    }

    fn render_window_controls(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let is_maximized = window.is_maximized();

        h_flex()
            .items_center()
            .h_full()
            .child(
                div()
                    .id("title-min")
                    .w(px(40.0))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .text_color(theme.foreground)
                    .hover(|s| s.bg(theme.secondary_hover))
                    .active(|s| s.bg(theme.secondary_active))
                    .on_click(cx.listener(|_, _, window, _| {
                        window.minimize_window();
                    }))
                    .child("-")
                    .window_control_area(WindowControlArea::Min),
            )
            .child(
                div()
                    .id("title-max")
                    .w(px(40.0))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .text_color(theme.foreground)
                    .hover(|s| s.bg(theme.secondary_hover))
                    .active(|s| s.bg(theme.secondary_active))
                    .on_click(cx.listener(|_, _, window, _| {
                        window.zoom_window();
                    }))
                    .child(if is_maximized { "❐" } else { "□" })
                    .window_control_area(WindowControlArea::Max),
            )
            .child(
                div()
                    .id("title-close")
                    .w(px(40.0))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .text_color(theme.foreground)
                    .hover(|s| s.bg(theme.danger).text_color(theme.danger_foreground))
                    .active(|s| s.bg(theme.danger_active).text_color(theme.danger_foreground))
                    .on_click(cx.listener(|this, _, window, cx| {
                        #[cfg(target_os = "windows")]
                        {
                            if let Err(err) = hide_main_window_to_tray() {
                                log::error!("트레이로 숨기기 실패: {err}");
                                window.remove_window();
                            } else {
                                this.push_log("INFO", "창을 트레이로 보냈습니다.".to_string());
                                this.notify_toast(
                                    "트레이에서 계속 실행됩니다",
                                    NotificationType::Info,
                                    window,
                                    cx,
                                );
                                cx.notify();
                            }
                        }

                        #[cfg(not(target_os = "windows"))]
                        {
                            let _ = this;
                            window.remove_window();
                        }
                    }))
                    .child("×"),
            )
            .into_any_element()
    }

    /// 사이드바 그룹 하나를 그린다.
    fn render_nav_group(
        &self,
        title: &'static str,
        items: &'static [(ActivePanel, &'static str, &'static str)],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let sidebar = theme.sidebar;
        let sidebar_fg = theme.sidebar_foreground;
        let active_bg = theme.sidebar_primary;
        let active_fg = theme.sidebar_primary_foreground;
        let muted_fg = theme.muted_foreground;

        let mut group = v_flex().gap_1().child(
            div()
                .px_2()
                .py_1()
                .text_color(muted_fg)
                .child(title),
        );

        for (panel, label, description) in items {
            let panel = *panel;
            let is_active = self.active_panel == panel;

            group = group.child(
                div()
                    .id(("nav-item", label.as_ptr() as usize))
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .cursor_pointer()
                    .bg(if is_active { active_bg } else { sidebar })
                    .hover(|s| s.bg(theme.sidebar_accent))
                    .on_click(cx.listener(move |this, _ev, window, cx| {
                        this.activate_panel(panel, window, cx);
                    }))
                    .child(
                        v_flex()
                            .child(
                                div()
                                    .text_color(if is_active { active_fg } else { sidebar_fg })
                                    .child(*label),
                            )
                            .child(
                                div()
                                    .text_color(if is_active { active_fg } else { muted_fg })
                                    .child(*description),
                            ),
                    ),
            );
        }

        group.into_any_element()
    }

    fn activate_panel(&mut self, panel: ActivePanel, window: &mut Window, cx: &mut Context<Self>) {
        match panel {
            ActivePanel::Dashboard | ActivePanel::AdBlock => {
                self.refresh_target_status(window, cx);
                if panel == ActivePanel::AdBlock && self.running_processes.is_empty() {
                    self.refresh_running_processes();
                }
            }
            ActivePanel::Services => {
                if self.sys_services.is_empty() {
                    self.refresh_sys_services();
                }
            }
            ActivePanel::FileSync => {
                if self.selected_sync_job.is_none() && !self.sync_jobs.is_empty() {
                    self.select_sync_job(0, window, cx);
                }
            }
            _ => {}
        }

        self.active_panel = panel;
        cx.notify();
    }
}

impl Render for AppRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.process_pending_events(window, cx);

        let window_controls = self.render_window_controls(window, cx);

        let panel: AnyElement = match self.active_panel {
            ActivePanel::Dashboard => self.render_dashboard_panel(cx),
            ActivePanel::AdBlock => ad_block::render(self, window, cx),
            ActivePanel::FileSync => file_sync::render(self, window, cx),
            ActivePanel::Services => service_mgr::render(self, window, cx),
            ActivePanel::AutoStart => service_view::render(self, window, cx),
            ActivePanel::Logs => self.render_log_panel(cx),
            ActivePanel::Settings => settings::render(self, window, cx),
        };

        let nav_overview = self.render_nav_group(
            "개요",
            &[(ActivePanel::Dashboard, "대시보드", "전체 상태 요약")],
            cx,
        );
        let nav_tools = self.render_nav_group("편의 기능", &NAV_TOOLS, cx);
        let nav_system = self.render_nav_group("시스템", &NAV_SYSTEM, cx);

        let theme = cx.theme();
        let background = theme.background;
        let sidebar = theme.sidebar;
        let sidebar_border = theme.sidebar_border;
        let sidebar_fg = theme.sidebar_foreground;
        let border = theme.border;
        let app_enabled = self.app_state.is_active;

        // 스플리터·가상 리스트를 쓰는 패널은 높이를 스스로 채우므로
        // 바깥에서 스크롤을 걸지 않는다.
        let fills_height = matches!(
            self.active_panel,
            ActivePanel::Logs
                | ActivePanel::Services
                | ActivePanel::AdBlock
                | ActivePanel::FileSync
        );
        let content_scroll = self.content_scroll_handle.clone();

        v_flex()
            .size_full()
            .bg(background)
            .child(
                h_flex()
                    .h(TITLE_BAR_HEIGHT)
                    .border_b_1()
                    .border_color(theme.title_bar_border)
                    .bg(theme.title_bar)
                    .child(
                        h_flex()
                            .id("custom-title-drag")
                            .h_full()
                            .flex_1()
                            .items_center()
                            .px_3()
                            .window_control_area(WindowControlArea::Drag)
                            .child(
                                div()
                                    .text_color(theme.foreground)
                                    .child("gpui-convenience-tools"),
                            ),
                    )
                    .child(window_controls),
            )
            .child(
                h_flex()
                    .size_full()
                    .min_h_0()
                    .child(
                        v_flex()
                            .w(px(240.0))
                            .h_full()
                            .bg(sidebar)
                            .border_r_1()
                            .border_color(sidebar_border)
                            .p_3()
                            .gap_3()
                            .child(
                                div()
                                    .px_2()
                                    .py_2()
                                    .text_color(sidebar_fg)
                                    .child("GPUI 편의 도구"),
                            )
                            .child(
                                div()
                                    .rounded_md()
                                    .p_2()
                                    .bg(theme.sidebar_accent)
                                    .child(
                                        h_flex()
                                            .justify_between()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_color(theme.sidebar_accent_foreground)
                                                    .child("광고 차단"),
                                            )
                                            .child(
                                                Switch::new("global-enable-switch")
                                                    .checked(app_enabled)
                                                    .on_click(cx.listener(
                                                        |this, checked: &bool, window, cx| {
                                                            this.set_service_enabled(
                                                                *checked, window, cx,
                                                            );
                                                        },
                                                    )),
                                            ),
                                    ),
                            )
                            .child(nav_overview)
                            .child(nav_tools)
                            .child(nav_system),
                    )
                    .child({
                        let outer = div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .min_h_0()
                            .relative()
                            .border_l_1()
                            .border_color(border);

                        if fills_height {
                            outer.child(div().id("content-area").size_full().p_4().child(panel))
                        } else {
                            outer
                                .child(
                                    div()
                                        .id("content-area")
                                        .size_full()
                                        .p_4()
                                        .overflow_y_scroll()
                                        .track_scroll(&content_scroll)
                                        .child(panel),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .left_0()
                                        .right_0()
                                        .bottom_0()
                                        .child(
                                            Scrollbar::vertical(&content_scroll)
                                                .scrollbar_show(ScrollbarShow::Always),
                                        ),
                                )
                        }
                    }),
            )
    }
}
