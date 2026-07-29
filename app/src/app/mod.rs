//! 앱 루트 엔티티.
//!
//! GPUI에서 `Render`를 구현하는 유일한 엔티티로, 모든 패널의 상태를 소유하고
//! 사이드바 · 타이틀바 · 콘텐츠 영역의 최상위 레이아웃을 구성한다.
//!
//! 세부 동작은 책임별 하위 모듈로 나뉜다.
//!
//! | 모듈 | 책임 |
//! | --- | --- |
//! | [`state`] | 순수 데이터 타입 |
//! | [`inputs`] | 입력 위젯 지연 생성 |
//! | [`background`] | 스캔 · 동기화 백그라운드 스레드 |
//! | [`ops`] | 광고 차단 · 서비스 · 로그 설정 조작 |
//! | [`sync_ops`] | 파일 동기화 작업 조작 |
//! | [`events`] | 백그라운드 → UI 이벤트 처리 |

mod background;
mod events;
mod inputs;
mod ops;
mod state;
mod sync_ops;

pub use state::{ActivePanel, AppState, LogEntry, SyncJobStatus, TargetApp};
use state::{PlatformEvent, ScannerState, SyncSharedState, NAV_SYSTEM, NAV_TOOLS};

use gpui::{
    div, px, AnyElement, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    ScrollHandle, StatefulInteractiveElement, Styled, Subscription, Window, WindowControlArea,
};
use gpui_component::{
    h_flex,
    input::InputState,
    notification::NotificationType,
    scroll::{Scrollbar, ScrollbarShow},
    theme::ActiveTheme,
    v_flex, VirtualListScrollHandle, TITLE_BAR_HEIGHT,
};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::config::{load_config, update_config, LogConfig, SyncJob};
use crate::platform::{NativePlatform, Platform, SysServiceInfo};
use crate::sync::SyncFailure;
use crate::window::{
    ad_block, dashboard, file_sync, log_view, service_mgr, service_view, settings, ui,
};

#[cfg(target_os = "windows")]
use crate::platform::{hide_main_window_to_tray, set_tray_service_active, set_tray_toggle_handler};

pub struct AppRoot {
    active_panel: ActivePanel,
    pub(crate) app_state: AppState,
    pub(crate) theme_filter_query: String,
    pub(crate) theme_filter_active_only: bool,
    pub(crate) theme_filter_input: Option<Entity<InputState>>,
    pub(crate) running_processes: Vec<String>,
    pub(crate) platform: Arc<dyn Platform>,
    event_tx: UnboundedSender<PlatformEvent>,
    event_rx: UnboundedReceiver<PlatformEvent>,
    pub(crate) log_scroll_handle: VirtualListScrollHandle,
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

    sidebar_scroll_handle: ScrollHandle,
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

            sidebar_scroll_handle: ScrollHandle::default(),
            content_scroll_handle: ScrollHandle::default(),
        }
    }

    pub(crate) fn app_state(&self) -> &AppState {
        &self.app_state
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
                    .active(|s| {
                        s.bg(theme.danger_active)
                            .text_color(theme.danger_foreground)
                    })
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
        let group_border = theme.sidebar_border;

        let mut item_list = v_flex().gap_1();

        for (panel, label, description) in items {
            let panel = *panel;
            let label = *label;
            let description = *description;
            let is_active = self.active_panel == panel;

            item_list = item_list.child(
                div()
                    .id(("nav-item", label.as_ptr() as usize))
                    .debug_selector(move || format!("nav-item-{label}"))
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .border_1()
                    .border_color(group_border)
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
                                    .child(label),
                            )
                            .child(
                                div()
                                    .text_color(if is_active { active_fg } else { muted_fg })
                                    .child(description),
                            ),
                    ),
            );
        }

        v_flex()
            .gap_2()
            .child(div().px_2().py_1().text_color(muted_fg).child(title))
            .child(
                div()
                    .rounded_lg()
                    .border_1()
                    .border_color(group_border)
                    .p_1()
                    .child(item_list),
            )
            .into_any_element()
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
            ActivePanel::Dashboard => dashboard::render(self, cx),
            ActivePanel::AdBlock => ad_block::render(self, window, cx),
            ActivePanel::FileSync => file_sync::render(self, window, cx),
            ActivePanel::Services => service_mgr::render(self, window, cx),
            ActivePanel::AutoStart => service_view::render(self, window, cx),
            ActivePanel::Logs => log_view::render(self, cx),
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
        let sidebar_scroll = self.sidebar_scroll_handle.clone();
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
                        div()
                            .w(px(240.0))
                            .h_full()
                            .min_h_0()
                            .flex_shrink_0()
                            .bg(sidebar)
                            .border_r_1()
                            .border_color(sidebar_border)
                            .child(crate::window::scroll_pane(
                                "sidebar-scroll",
                                &sidebar_scroll,
                                v_flex()
                                    .w_full()
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
                                        div().rounded_md().p_2().bg(theme.sidebar_accent).child(
                                            h_flex()
                                                .justify_between()
                                                .items_center()
                                                .child(
                                                    div()
                                                        .text_color(theme.sidebar_accent_foreground)
                                                        .child("광고 차단"),
                                                )
                                                .child(
                                                    ui::toggle_switch(
                                                        "global-enable-switch",
                                                        app_enabled,
                                                        cx,
                                                    )
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
                                    .child(nav_system)
                                    .into_any_element(),
                            )),
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
                            outer.child(
                                div()
                                    .id("content-area")
                                    .debug_selector(|| "content-area".to_string())
                                    .size_full()
                                    .p_4()
                                    .child(panel),
                            )
                        } else {
                            outer
                                .child(
                                    div()
                                        .id("content-area")
                                        .debug_selector(|| "content-area".to_string())
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

#[cfg(test)]
mod tests;
