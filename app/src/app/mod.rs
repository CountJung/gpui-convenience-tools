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
mod interval;
mod ops;
mod state;
mod sync_ops;
mod ui_state;
mod virtual_disk_copy;
mod virtual_disk_ops;

pub(crate) use interval::{IntervalPicker, IntervalTarget};
pub(crate) use virtual_disk_copy::{
    init_virtual_disk_keymap, CopySelected, EnterSelected, ParentDirectory, Refresh, SelectAll,
    VIRTUAL_DISK_KEY_CONTEXT,
};

pub use state::{ActivePanel, AppState, LogEntry, TargetApp};
use state::{PlatformEvent, ScannerState, SyncSharedState, NAV_SYSTEM, NAV_TOOLS};
pub use ui_state::ServiceFilter;
use ui_state::{AdBlockState, ServiceState, SyncState};
use virtual_disk_ops::VirtualDiskSession;

use gpui::{
    div, px, AnyElement, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render,
    Pixels, ScrollHandle, StatefulInteractiveElement, Styled, Subscription, Timer, Window,
    WindowControlArea,
};
use gpui_component::{
    h_flex,
    input::InputState,
    notification::NotificationType,
    resizable::{h_resizable, resizable_panel},
    scroll::{Scrollbar, ScrollbarShow},
    theme::ActiveTheme,
    v_flex, PixelsExt, VirtualListScrollHandle, TITLE_BAR_HEIGHT,
};
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::config::{
    default_interval_presets, load_config, normalize_interval_presets, normalize_sidebar_width,
    update_config, LogConfig, DEFAULT_SIDEBAR_WIDTH,
};
use crate::platform::{NativePlatform, Platform};
use crate::window::{
    ad_block, dashboard, file_sync, log_view, service_mgr, service_view, settings, ui,
    virtual_disk,
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
    pub(crate) sidebar_width: f32,
    /// 스캔 주기·감시 주기가 공유하는 주기 선택 상태.
    pub(crate) interval_picker: IntervalPicker,

    // ── 기능별 패널 상태 ──
    pub(crate) services: ServiceState,
    pub(crate) sync: SyncState,
    pub(crate) ad_block: AdBlockState,

    // ── VirtualBox 오프라인 디스크 탐색 ──
    pub(crate) virtual_disk: VirtualDiskSession,
    pub(crate) virtual_disk_page_scroll: ScrollHandle,

    // ── 로그 설정 ──
    pub(crate) log_config: LogConfig,

    sidebar_scroll_handle: ScrollHandle,
    content_scroll_handle: ScrollHandle,
}

/// 검증 하네스가 패널 전환 입력 없이 특정 화면을 열 수 있도록 하는 실행 전용 선택값이다.
/// 일반 사용자 설정에는 저장하지 않으며, 값이 없거나 알 수 없으면 대시보드로 시작한다.
fn initial_panel_from_validation_env() -> ActivePanel {
    match std::env::var("GPUI_CONVENIENCE_TOOLS_INITIAL_PANEL")
        .ok()
        .as_deref()
    {
        Some("file_sync") => ActivePanel::FileSync,
        Some("auto_start") => ActivePanel::AutoStart,
        _ => ActivePanel::Dashboard,
    }
}

impl AppRoot {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let platform: Arc<dyn Platform> = Arc::new(NativePlatform::new());
        let mut app_state = AppState::default();
        let mut initial_scan_interval_secs: u32 = 10;
        let mut sync_jobs = Vec::new();
        let mut favorite_services = Vec::new();
        let mut log_config = LogConfig::default();
        let mut initial_interval_presets = default_interval_presets();
        let mut sync_enabled = true;
        let mut sidebar_width = DEFAULT_SIDEBAR_WIDTH;
        let mut virtual_disk_suppressed_issue_keys = BTreeSet::new();
        let initial_sync_history = crate::sync_history::load_recent(20).unwrap_or_else(|err| {
            log::warn!("동기화 이력 불러오기 실패: {err:#}");
            Vec::new()
        });

        if let Ok(Some(cfg)) = load_config() {
            sync_enabled = cfg.sync_enabled;
            sidebar_width = normalize_sidebar_width(cfg.sidebar_width);
            virtual_disk_suppressed_issue_keys.extend(
                cfg.virtual_disk_suppressed_issue_keys
                    .into_iter()
                    .filter(|key| !key.trim().is_empty()),
            );
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
            // 구버전 config에는 프리셋이 없어 기본값이 들어온다. 손상된 값은 여기서 거른다.
            initial_interval_presets = cfg.interval_presets;
            normalize_interval_presets(&mut initial_interval_presets);
            if initial_interval_presets.is_empty() {
                initial_interval_presets = default_interval_presets();
            }
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
        // 끊긴 지점은 작업에 저장돼 있다. 실행 중에는 공유 상태가 정본이므로 여기로 옮긴다.
        let cursors = sync_jobs
            .iter()
            .filter_map(|job| {
                job.resume_cursor
                    .clone()
                    .map(|cursor| (job.id.clone(), cursor))
            })
            .collect();
        let sync_shared = Arc::new(Mutex::new(SyncSharedState {
            jobs: sync_jobs.clone(),
            auto_enabled: sync_enabled,
            cursors,
            ..SyncSharedState::default()
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
        Self::spawn_sync_loop(event_tx.clone(), Arc::clone(&sync_shared));

        let running_processes = platform.list_running_processes().unwrap_or_default();
        let selected_sync_job = (!sync_jobs.is_empty()).then_some(0);

        let root = Self {
            active_panel: initial_panel_from_validation_env(),
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
            sidebar_width,
            interval_picker: IntervalPicker {
                presets: initial_interval_presets,
                ..IntervalPicker::default()
            },

            services: ServiceState {
                favorites: favorite_services,
                ..ServiceState::default()
            },
            sync: SyncState {
                enabled: sync_enabled,
                jobs: sync_jobs,
                selected_job: selected_sync_job,
                status: Default::default(),
                history: initial_sync_history,
                failures: Vec::new(),
                running: None,
                suppressed_failures: Default::default(),
                notify_enabled: true,
                name_input: None,
                source_input: None,
                target_input: None,
                exclude_input: None,
                page_scroll: ScrollHandle::default(),
                shared: sync_shared,
                #[cfg(test)]
                external_side_effects_enabled: true,
            },
            ad_block: AdBlockState::default(),

            virtual_disk: VirtualDiskSession {
                suppressed_issue_keys: virtual_disk_suppressed_issue_keys,
                ..VirtualDiskSession::default()
            },
            virtual_disk_page_scroll: ScrollHandle::default(),

            log_config,

            sidebar_scroll_handle: ScrollHandle::default(),
            content_scroll_handle: ScrollHandle::default(),
        };

        Self::start_event_refresh_loop(cx);

        root
    }

    /// 백그라운드 채널에 새 이벤트가 있을 때 GPUI 렌더 루프를 깨운다.
    fn start_event_refresh_loop(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| loop {
            Timer::after(Duration::from_millis(200)).await;
            let Some(this) = this.upgrade() else {
                break;
            };
            if this
                .update(cx, |this, cx| {
                    if !this.event_rx.is_empty() {
                        cx.notify();
                    }
                })
                .is_err()
            {
                break;
            }
        })
        .detach();
    }

    pub(crate) fn app_state(&self) -> &AppState {
        &self.app_state
    }

    /// 사이드바 상단의 전역 스위치 한 줄.
    ///
    /// 앱을 켜두기만 해도 스스로 도는 기능은 패널에 들어가지 않고도 끌 수 있어야 하므로
    /// 여기에 모은다. 광고 차단과 파일 동기화가 같은 모양을 쓴다.
    fn render_sidebar_switch(
        &self,
        id: &'static str,
        label: &'static str,
        enabled: bool,
        on_toggle: impl Fn(&mut Self, bool, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme();
        let accent = theme.sidebar_accent;
        let accent_foreground = theme.sidebar_accent_foreground;

        div()
            .rounded_md()
            .p_2()
            .bg(accent)
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_color(accent_foreground).child(label))
                    // `Switch`는 debug_selector를 직접 받지 않으므로 감싼 div가 대신 단다.
                    .child(
                        div()
                            .debug_selector(move || id.to_string())
                            .child(ui::toggle_switch(id, enabled, cx).on_click(cx.listener(
                                move |this, checked: &bool, window, cx| {
                                    on_toggle(this, *checked, window, cx);
                                },
                            ))),
                    ),
            )
            .into_any_element()
    }

    /// 광고 차단 전역 스위치. Win32 창 조작 전용이라 다른 OS에는 두지 않는다.
    #[cfg(target_os = "windows")]
    fn render_ad_block_toggle(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        Some(self.render_sidebar_switch(
            "global-enable-switch",
            "광고 차단",
            self.app_state.is_active,
            |this, checked, window, cx| this.set_service_enabled(checked, window, cx),
            cx,
        ))
    }

    #[cfg(not(target_os = "windows"))]
    fn render_ad_block_toggle(&self, _cx: &mut Context<Self>) -> Option<AnyElement> {
        None
    }

    /// 파일 동기화 자동 실행 전역 스위치. 모든 플랫폼에 있다.
    fn render_file_sync_toggle(&self, cx: &mut Context<Self>) -> AnyElement {
        self.render_sidebar_switch(
            "global-sync-switch",
            "파일 동기화",
            self.sync.enabled,
            |this, checked, window, cx| this.set_sync_enabled(checked, window, cx),
            cx,
        )
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
                if self.services.items.is_empty() {
                    self.refresh_sys_services();
                }
            }
            ActivePanel::FileSync => {
                if self.sync.selected_job.is_none() && !self.sync.jobs.is_empty() {
                    self.select_sync_job(0, window, cx);
                }
            }
            ActivePanel::VirtualDisk => {}
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
            ActivePanel::VirtualDisk => virtual_disk::render(self, window, cx),
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
        let ad_block_toggle = self.render_ad_block_toggle(cx);
        let file_sync_toggle = self.render_file_sync_toggle(cx);

        let theme = cx.theme();
        let background = theme.background;
        let sidebar = theme.sidebar;
        let sidebar_border = theme.sidebar_border;
        let sidebar_fg = theme.sidebar_foreground;
        let border = theme.border;

        // 스플리터·가상 리스트를 쓰는 패널은 높이를 스스로 채우므로
        // 바깥에서 스크롤을 걸지 않는다.
        let fills_height = matches!(
            self.active_panel,
            ActivePanel::Logs
                | ActivePanel::Services
                | ActivePanel::AdBlock
                | ActivePanel::FileSync
                | ActivePanel::VirtualDisk
        );
        let sidebar_scroll = self.sidebar_scroll_handle.clone();
        let content_scroll = self.content_scroll_handle.clone();
        let initial_sidebar_width = px(normalize_sidebar_width(self.sidebar_width));
        let persist_sidebar_width = !cfg!(test);

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
                div()
                    .flex_1()
                    .min_h_0()
                    .child(
                        h_resizable("app-shell-split")
                            .on_resize(move |state, _window, cx| {
                                if !persist_sidebar_width {
                                    return;
                                }
                                let Some(width) = state
                                    .read(cx)
                                    .sizes()
                                    .first()
                                    .map(|size| size.as_f32())
                                else {
                                    return;
                                };
                                let width = normalize_sidebar_width(width);
                                if let Err(err) =
                                    update_config(|config| config.sidebar_width = width)
                                {
                                    log::warn!("사이드바 폭 저장 실패: {err}");
                                }
                            })
                            .child(
                                resizable_panel()
                                    .size(initial_sidebar_width)
                                    .size_range(px(200.0)..px(360.0))
                                    .child(
                                div()
                                    .debug_selector(|| "sidebar-pane".to_string())
                                    .size_full()
                                    .min_h_0()
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
                                            .children(ad_block_toggle)
                                            .child(file_sync_toggle)
                                            .child(nav_overview)
                                            .child(nav_tools)
                                            .child(nav_system)
                                            .into_any_element(),
                                    )),
                            ),
                    )
                        .child(
                            resizable_panel()
                                .size_range(px(520.0)..Pixels::MAX)
                                .child({
                                    let outer = div()
                                        .debug_selector(|| "content-pane".to_string())
                                        .size_full()
                                        .min_w_0()
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
                                                .min_w_0()
                                                .p_4()
                                                .overflow_x_hidden()
                                                .child(panel),
                                        )
                                    } else {
                                        outer
                                            .child(
                                                div()
                                                    .id("content-area")
                                                    .debug_selector(|| "content-area".to_string())
                                                    .size_full()
                                                    .min_w_0()
                                                    .p_4()
                                                    .overflow_x_hidden()
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
                        ),
                    ),
            )
    }
}

#[cfg(test)]
mod tests;
