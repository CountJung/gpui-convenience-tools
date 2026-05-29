use gpui::{
    div, px, size, AnyElement, AppContext, Context, Entity, InteractiveElement, IntoElement,
    ParentElement, Render, StatefulInteractiveElement, Styled, Subscription, Window,
    WindowControlArea,
};
use gpui_component::{
    h_flex,
    input::{InputEvent, InputState},
    notification::{Notification, NotificationType},
    scroll::ScrollableElement,
    switch::Switch,
    theme::ActiveTheme,
    v_flex, v_virtual_list, VirtualListScrollHandle, WindowExt, TITLE_BAR_HEIGHT,
};
use serde::{Deserialize, Serialize};
use std::{
    future::pending,
    ops::Range,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::config::{load_config, save_config, AppConfig};
use crate::platform::{NativePlatform, NativeWindowHandle, Platform};
use crate::window::{service_view, settings};

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
                message: "App initialized".to_string(),
            }],
        }
    }
}

#[derive(Clone, Debug)]
enum PlatformEvent {
    AdBlocked,
    TargetStatusChanged(bool),
    ServiceToggled(bool),
    TargetToggled { index: usize, enabled: bool },
}

#[derive(Clone, Debug)]
struct ScannerState {
    service_enabled: bool,
    targets: Vec<TargetApp>,
    scan_interval_secs: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivePanel {
    Dashboard,
    TargetList,
    Settings,
    Logs,
    ServiceView,
}

pub struct AppRoot {
    active_panel: ActivePanel,
    app_state: AppState,
    pub(crate) theme_filter_query: String,
    pub(crate) theme_filter_active_only: bool,
    pub(crate) theme_filter_input: Option<Entity<InputState>>,
    running_processes: Vec<String>,
    platform: Arc<dyn Platform>,
    event_tx: UnboundedSender<PlatformEvent>,
    event_rx: UnboundedReceiver<PlatformEvent>,
    log_scroll_handle: VirtualListScrollHandle,
    scanner_state: Arc<Mutex<ScannerState>>,
    subscriptions: Vec<Subscription>,
    pub(crate) scan_interval_secs: u32,
}

impl AppRoot {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        let platform: Arc<dyn Platform> = Arc::new(NativePlatform::new());
        let mut app_state = AppState::default();
        let mut initial_scan_interval_secs: u32 = 10;

        if let Ok(Some(cfg)) = load_config() {
            app_state.is_active = cfg.service_enabled;
            if !cfg.targets.is_empty() {
                app_state.targets = cfg.targets;
            }
            initial_scan_interval_secs = cfg.scan_interval_secs.max(1);
            app_state.log_entries.push(LogEntry {
                level: "INFO".to_string(),
                message: "Loaded config file.".to_string(),
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

        #[cfg(target_os = "windows")]
        {
            set_tray_service_active(app_state.is_active);

            let event_tx_for_tray = event_tx.clone();
            let scanner_state_for_tray = Arc::clone(&scanner_state);
            set_tray_toggle_handler(Arc::new(move |enabled| {
                let targets = if let Ok(mut state) = scanner_state_for_tray.lock() {
                    state.service_enabled = enabled;
                    state.targets.clone()
                } else {
                    Vec::new()
                };

                if !targets.is_empty() {
                    let existing = load_config().ok().flatten();
                    let cfg = AppConfig {
                        service_enabled: enabled,
                        targets,
                        light_theme_name: existing
                            .as_ref()
                            .and_then(|cfg| cfg.light_theme_name.clone()),
                        dark_theme_name: existing
                            .as_ref()
                            .and_then(|cfg| cfg.dark_theme_name.clone()),
                        scan_interval_secs: existing
                            .as_ref()
                            .map(|cfg| cfg.scan_interval_secs)
                            .unwrap_or(10),
                    };
                    if let Err(err) = save_config(&cfg) {
                        log::error!("failed to save config from tray toggle: {err}");
                    }
                }

                let _ = event_tx_for_tray.send(PlatformEvent::ServiceToggled(enabled));
            }));
        }

        Self::spawn_platform_loop(
            Arc::clone(&platform),
            event_tx.clone(),
            Arc::clone(&scanner_state),
        );

        let running_processes = platform.list_running_processes().unwrap_or_default();

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
        }
    }

    pub(crate) fn ensure_theme_filter_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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

    fn refresh_target_status(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let running = self
            .app_state
            .targets
            .iter()
            .filter(|t| t.enabled)
            .any(|t| self.platform.is_target_running(&t.process_name));

        let _ = self.event_tx.send(PlatformEvent::TargetStatusChanged(running));
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
        let existing = load_config().ok().flatten();
        let cfg = AppConfig {
            service_enabled: self.app_state.is_active,
            targets: self.app_state.targets.clone(),
            light_theme_name: existing
                .as_ref()
                .and_then(|cfg| cfg.light_theme_name.clone()),
            dark_theme_name: existing
                .as_ref()
                .and_then(|cfg| cfg.dark_theme_name.clone()),
            scan_interval_secs: self.scan_interval_secs,
        };

        if let Err(err) = save_config(&cfg) {
            log::error!("failed to save config: {err}");
        }
    }

    pub fn set_scan_interval(&mut self, secs: u32, cx: &mut Context<Self>) {
        let secs = secs.max(1);
        self.scan_interval_secs = secs;
        self.sync_scanner_state();
        self.persist_config();
        self.app_state.log_entries.push(LogEntry {
            level: "INFO".to_string(),
            message: format!("Scan interval set to {secs}s"),
        });
        self.log_scroll_handle.scroll_to_bottom();
        cx.notify();
    }

    fn refresh_running_processes(&mut self) {
        match self.platform.list_running_processes() {
            Ok(processes) => {
                self.running_processes = processes;
            }
            Err(err) => {
                log::error!("failed to list running processes: {err}");
            }
        }
    }

    /// 서비스 뷰에서 발생하는 동작 결과를 로그 패널에 기록한다.
    pub fn push_service_log(&mut self, message: &str, _window: &mut Window, cx: &mut Context<Self>) {
        self.app_state.log_entries.push(LogEntry {
            level: "INFO".to_string(),
            message: message.to_string(),
        });
        self.log_scroll_handle.scroll_to_bottom();
        cx.notify();
    }

    fn add_target_process(&mut self, process_name: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .app_state
            .targets
            .iter()
            .any(|target| target.process_name.eq_ignore_ascii_case(process_name))
        {
            window.push_notification(
                Notification::new()
                    .message("Target already exists")
                    .with_type(NotificationType::Info)
                    .title("gpui-convenience-tools"),
                cx,
            );
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

        self.app_state.log_entries.push(LogEntry {
            level: "INFO".to_string(),
            message: format!("Added target from running process: {process_name}"),
        });

        window.push_notification(
            Notification::new()
                .message("Target added from running process")
                .with_type(NotificationType::Success)
                .title("gpui-convenience-tools"),
            cx,
        );
    }

    fn process_pending_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                PlatformEvent::AdBlocked => {
                    self.app_state.blocked_count += 1;
                    self.app_state.log_entries.push(LogEntry {
                        level: "SUCCESS".to_string(),
                        message: "Ad window detected and hidden.".to_string(),
                    });
                    window.push_notification(
                        Notification::new()
                            .message("Ad blocked")
                            .with_type(NotificationType::Success)
                            .title("gpui-convenience-tools"),
                        cx,
                    );
                    self.log_scroll_handle.scroll_to_bottom();
                }
                PlatformEvent::TargetStatusChanged(is_running) => {
                    self.app_state.is_target_running = is_running;
                    self.app_state.log_entries.push(LogEntry {
                        level: "INFO".to_string(),
                        message: if is_running {
                            "Target status: running".to_string()
                        } else {
                            "Target status: not running".to_string()
                        },
                    });
                    self.log_scroll_handle.scroll_to_bottom();
                }
                PlatformEvent::ServiceToggled(enabled) => {
                    self.app_state.is_active = enabled;
                    self.sync_scanner_state();
                    self.persist_config();
                    #[cfg(target_os = "windows")]
                    set_tray_service_active(enabled);
                    self.app_state.log_entries.push(LogEntry {
                        level: "INFO".to_string(),
                        message: if enabled {
                            "Service state: enabled".to_string()
                        } else {
                            "Service state: disabled".to_string()
                        },
                    });
                    window.push_notification(
                        Notification::new()
                            .message(if enabled {
                                "Service has been enabled"
                            } else {
                                "Service has been disabled"
                            })
                            .with_type(NotificationType::Info)
                            .title("gpui-convenience-tools"),
                        cx,
                    );
                    self.log_scroll_handle.scroll_to_bottom();
                }
                PlatformEvent::TargetToggled { index, enabled } => {
                    let message = if let Some(target) = self.app_state.targets.get_mut(index) {
                        target.enabled = enabled;
                        format!("{} toggle changed: {}", target.process_name, enabled)
                    } else {
                        continue;
                    };

                    self.sync_scanner_state();
                    self.persist_config();
                    self.app_state.log_entries.push(LogEntry {
                        level: "INFO".to_string(),
                        message,
                    });
                    window.push_notification(
                        Notification::new()
                            .message("Target app setting updated")
                            .with_type(NotificationType::Info)
                            .title("gpui-convenience-tools"),
                        cx,
                    );
                    self.log_scroll_handle.scroll_to_bottom();
                }
            }
        }
    }

    fn render_dashboard_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let status_text = if !self.app_state.is_active {
            "Service: disabled"
        } else if self.app_state.is_target_running {
            "Service: enabled / target running"
        } else {
            "Service: enabled / target not running"
        };

        let status_bg = if !self.app_state.is_active {
            theme.warning
        } else if self.app_state.is_target_running {
            theme.success
        } else {
            theme.info
        };

        let status_fg = if !self.app_state.is_active {
            theme.warning_foreground
        } else if self.app_state.is_target_running {
            theme.success_foreground
        } else {
            theme.info_foreground
        };

        v_flex()
            .size_full()
            .gap_3()
            .child(div().text_color(theme.foreground).child("Dashboard"))
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
                            .child(
                                div()
                                    .rounded_md()
                                    .px_3()
                                    .py_2()
                                    .bg(status_bg)
                                    .text_color(status_fg)
                                    .child(status_text),
                            )
                            .child(
                                div().text_color(theme.foreground).child(format!(
                                    "Total blocks: {}",
                                    self.app_state.blocked_count
                                )),
                            )
                            .child(
                                div().text_color(theme.foreground).child(format!(
                                    "Targets: {}",
                                    self.app_state.targets.len()
                                )),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_target_list_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();

        let target_rows = self
            .app_state
            .targets
            .iter()
            .enumerate()
            .fold(v_flex().gap_2(), |list, (ix, target)| {
                list.child(
                    div()
                        .rounded_md()
                        .px_3()
                        .py_2()
                        .bg(theme.list)
                        .border_1()
                        .border_color(theme.border)
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .child(
                                    v_flex()
                                        .gap_1()
                                        .child(
                                            div().text_color(theme.foreground).child(format!(
                                                "{} ({})",
                                                target.display_name, target.process_name
                                            )),
                                        )
                                        .child(
                                            div()
                                                .text_color(theme.muted_foreground)
                                                .child(format!("class: {}", target.ad_window_class)),
                                        ),
                                )
                                .child(
                                    Switch::new(("target-switch", ix))
                                        .checked(target.enabled)
                                        .on_click(cx.listener(move |this, checked: &bool, window, cx| {
                                            let _ = this.event_tx.send(PlatformEvent::TargetToggled {
                                                index: ix,
                                                enabled: *checked,
                                            });
                                            this.process_pending_events(window, cx);
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
            });

        let mut running_process_chips = h_flex().gap_2().flex_wrap();
        for (ix, process_name) in self.running_processes.iter().enumerate() {
            let exists = self
                .app_state
                .targets
                .iter()
                .any(|target| target.process_name.eq_ignore_ascii_case(process_name));

            if exists {
                running_process_chips = running_process_chips.child(
                    div()
                        .rounded_md()
                        .px_2()
                        .py_1()
                        .bg(theme.muted)
                        .text_color(theme.muted_foreground)
                        .border_1()
                        .border_color(theme.border)
                        .child(format!("{process_name} (added)")),
                );
            } else {
                running_process_chips = running_process_chips.child(
                    div()
                        .rounded_md()
                        .px_2()
                        .py_1()
                        .cursor_pointer()
                        .bg(theme.secondary)
                        .text_color(theme.foreground)
                        .border_1()
                        .border_color(theme.border)
                        .hover(|s| s.bg(theme.secondary_hover))
                        .id(("running-process-chip", ix))
                        .on_click(cx.listener({
                            let process_name = process_name.clone();
                            move |this, _event, window, cx| {
                                this.add_target_process(&process_name, window, cx);
                                cx.notify();
                            }
                        }))
                        .child(format!("+ {process_name}")),
                );
            }
        }

        v_flex()
            .size_full()
            .gap_3()
            .child(div().text_color(theme.foreground).child("Targets"))
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
                            .child(div().text_color(theme.foreground).child("Running Processes"))
                            .child(
                                div()
                                    .rounded_md()
                                    .px_3()
                                    .py_2()
                                    .cursor_pointer()
                                    .bg(theme.list)
                                    .border_1()
                                    .border_color(theme.border)
                                    .hover(|s| s.bg(theme.secondary_hover))
                                    .id("refresh-running-processes")
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.refresh_running_processes();
                                        cx.notify();
                                    }))
                                    .child("Refresh running process list"),
                            )
                            .child(
                                div()
                                    .text_color(theme.muted_foreground)
                                    .child("Click a process to add it as a target."),
                            )
                            .child(running_process_chips),
                    ),
            )
            .child(
                div()
                    .rounded_lg()
                    .p_4()
                    .bg(theme.secondary)
                    .border_1()
                    .border_color(theme.border)
                    .child(target_rows),
            )
            .into_any_element()
    }

    fn render_log_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();

        if self.app_state.log_entries.is_empty() {
            return v_flex()
                .size_full()
                .gap_3()
                .child(div().text_color(theme.foreground).child("Logs"))
                .child(
                    div()
                        .rounded_lg()
                        .p_4()
                        .bg(theme.secondary)
                        .border_1()
                        .border_color(theme.border)
                        .text_color(theme.muted_foreground)
                        .child("No logs yet."),
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

        v_flex()
            .size_full()
            .gap_3()
            .child(div().text_color(theme.foreground).child("Logs"))
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
                                log::error!("failed to hide window to tray: {err}");
                                window.remove_window();
                            } else {
                                this.app_state.log_entries.push(LogEntry {
                                    level: "INFO".to_string(),
                                    message: "Window sent to tray.".to_string(),
                                });
                                window.push_notification(
                                    Notification::new()
                                        .message("App continues to run in tray.")
                                        .with_type(NotificationType::Info)
                                        .title("gpui-convenience-tools"),
                                    cx,
                                );
                                cx.notify();
                            }
                        }

                        #[cfg(not(target_os = "windows"))]
                        {
                            window.remove_window();
                        }
                    }))
                    .child("×"),
            )
            .into_any_element()
    }
}

impl Render for AppRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.process_pending_events(window, cx);

        let window_controls = self.render_window_controls(window, cx);

        let panel: AnyElement = match self.active_panel {
            ActivePanel::Dashboard => self.render_dashboard_panel(cx),
            ActivePanel::TargetList => self.render_target_list_panel(cx),
            ActivePanel::Settings => settings::render(self, window, cx),
            ActivePanel::Logs => self.render_log_panel(cx),
            ActivePanel::ServiceView => service_view::render(self, window, cx),
        };

        let theme = cx.theme();
        let background = theme.background;
        let sidebar = theme.sidebar;
        let sidebar_border = theme.sidebar_border;
        let sidebar_fg = theme.sidebar_foreground;
        let sidebar_active_bg = theme.sidebar_primary;
        let sidebar_active_fg = theme.sidebar_primary_foreground;
        let border = theme.border;

        let dashboard_active = self.active_panel == ActivePanel::Dashboard;
        let target_active = self.active_panel == ActivePanel::TargetList;
        let settings_active = self.active_panel == ActivePanel::Settings;
        let logs_active = self.active_panel == ActivePanel::Logs;
        let service_active = self.active_panel == ActivePanel::ServiceView;

        let app_enabled = self.app_state.is_active;

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
                                .child(div().text_color(theme.foreground).child("gpui-convenience-tools")),
                    )
                            .child(window_controls),
            )
            .child(
                h_flex()
                    .size_full()
                    .child(
                        v_flex()
                            .w(px(220.0))
                            .h_full()
                            .bg(sidebar)
                            .border_r_1()
                            .border_color(sidebar_border)
                            .p_3()
                            .gap_2()
                            .child(
                                div()
                                    .px_2()
                                    .py_3()
                                    .text_color(sidebar_fg)
                                    .child("gpui-convenience-tools"),
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
                                                    .child("Active"),
                                            )
                                            .child(
                                                Switch::new("global-enable-switch")
                                                    .checked(app_enabled)
                                                    .on_click(cx.listener(|this, checked: &bool, window, cx| {
                                                        let _ = this.event_tx.send(PlatformEvent::ServiceToggled(*checked));
                                                        this.process_pending_events(window, cx);
                                                        cx.notify();
                                                    })),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .bg(if dashboard_active { sidebar_active_bg } else { sidebar })
                                    .text_color(if dashboard_active { sidebar_active_fg } else { sidebar_fg })
                                    .id("nav-dashboard")
                                    .on_click(cx.listener(|this, _event, window, cx| {
                                        this.refresh_target_status(window, cx);
                                        this.active_panel = ActivePanel::Dashboard;
                                        cx.notify();
                                    }))
                                    .child("Dashboard"),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .bg(if target_active { sidebar_active_bg } else { sidebar })
                                    .text_color(if target_active { sidebar_active_fg } else { sidebar_fg })
                                    .id("nav-targets")
                                    .on_click(cx.listener(|this, _event, window, cx| {
                                        this.refresh_target_status(window, cx);
                                        this.active_panel = ActivePanel::TargetList;
                                        cx.notify();
                                    }))
                                    .child("Targets"),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .bg(if settings_active { sidebar_active_bg } else { sidebar })
                                    .text_color(if settings_active { sidebar_active_fg } else { sidebar_fg })
                                    .id("nav-settings")
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.active_panel = ActivePanel::Settings;
                                        cx.notify();
                                    }))
                                    .child("Settings"),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .bg(if logs_active { sidebar_active_bg } else { sidebar })
                                    .text_color(if logs_active { sidebar_active_fg } else { sidebar_fg })
                                    .id("nav-logs")
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.active_panel = ActivePanel::Logs;
                                        cx.notify();
                                    }))
                                    .child("Logs"),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .rounded_md()
                                    .cursor_pointer()
                                    .bg(if service_active { sidebar_active_bg } else { sidebar })
                                    .text_color(if service_active { sidebar_active_fg } else { sidebar_fg })
                                    .id("nav-service")
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.active_panel = ActivePanel::ServiceView;
                                        cx.notify();
                                    }))
                                    .child("Service"),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .p_4()
                            .border_l_1()
                            .border_color(border)
                            .child(panel)
                            .overflow_y_scrollbar(),
                    ),
            )
    }
}



