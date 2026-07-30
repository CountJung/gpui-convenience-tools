use super::*;
use crate::config::{SyncJob, BUNDLED_THEMES};
use crate::platform::NativeWindowHandle;
use crate::sync::SyncFailure;
use anyhow::Result;
use gpui::{point, px, size, Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext};
use gpui_component::theme::{ActiveTheme, Theme, ThemeMode, ThemeSet};
use std::rc::Rc;

const DEFAULT_WINDOW_WIDTH: f32 = 1000.0;
const DEFAULT_WINDOW_HEIGHT: f32 = 700.0;
const MIN_SUPPORTED_WINDOW_WIDTH: f32 = 920.0;
const COMPACT_WINDOW_HEIGHT: f32 = 480.0;

struct TestPlatform;

impl Platform for TestPlatform {
    fn is_target_running(&self, _process_name: &str) -> bool {
        false
    }

    fn list_running_processes(&self) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn find_ad_window(&self, _process_name: &str) -> Result<Option<NativeWindowHandle>> {
        Ok(None)
    }

    fn hide_ad(&self, _handle: NativeWindowHandle) -> Result<()> {
        Ok(())
    }

    fn show_ad(&self, _handle: NativeWindowHandle) -> Result<()> {
        Ok(())
    }
}

fn test_app_root(active_panel: ActivePanel) -> AppRoot {
    let app_state = AppState::default();
    let (event_tx, event_rx) = unbounded_channel();

    AppRoot {
        active_panel,
        app_state: app_state.clone(),
        theme_filter_query: String::new(),
        theme_filter_active_only: false,
        theme_filter_input: None,
        running_processes: Vec::new(),
        platform: Arc::new(TestPlatform),
        event_tx,
        event_rx,
        log_scroll_handle: VirtualListScrollHandle::new(),
        scanner_state: Arc::new(Mutex::new(ScannerState {
            service_enabled: app_state.is_active,
            targets: app_state.targets,
            scan_interval_secs: 10,
        })),
        subscriptions: Vec::new(),
        scan_interval_secs: 10,
        sys_services: Vec::new(),
        service_search_query: String::new(),
        service_search_input: None,
        svc_scroll_handle: VirtualListScrollHandle::new(),
        svc_right_scroll: ScrollHandle::default(),
        pending_delete_service: None,
        service_filter: ServiceFilter::All,
        favorite_services: Vec::new(),
        sync_jobs: Vec::new(),
        selected_sync_job: None,
        sync_status: HashMap::new(),
        sync_failures: Vec::new(),
        suppressed_sync_failures: HashSet::new(),
        sync_notify_enabled: true,
        sync_name_input: None,
        sync_source_input: None,
        sync_target_input: None,
        sync_left_scroll: ScrollHandle::default(),
        sync_right_scroll: ScrollHandle::default(),
        sync_state: Arc::new(Mutex::new(SyncSharedState::default())),
        ad_left_scroll: ScrollHandle::default(),
        ad_right_scroll: ScrollHandle::default(),
        log_config: LogConfig::default(),
        sidebar_scroll_handle: ScrollHandle::default(),
        content_scroll_handle: ScrollHandle::default(),
    }
}

fn initialize_components(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::theme::normalize_component_palette(cx);
    });
}

fn refresh(cx: &mut gpui::VisualTestContext) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

fn click_debug_element(cx: &mut gpui::VisualTestContext, selector: &'static str) {
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} should be rendered"));
    cx.simulate_click(
        point(
            bounds.origin.x + bounds.size.width / 2.0,
            bounds.origin.y + bounds.size.height / 2.0,
        ),
        Modifiers::none(),
    );
    refresh(cx);
}

fn apply_theme_mode(cx: &mut gpui::VisualTestContext, mode: ThemeMode) {
    cx.update(|window, app| crate::theme::change_theme(mode, Some(window), app));
    refresh(cx);
}

fn apply_bundled_theme(cx: &mut gpui::VisualTestContext, file_name: &str, theme_name: &str) {
    let (_, contents) = BUNDLED_THEMES
        .iter()
        .find(|(candidate, _)| *candidate == file_name)
        .unwrap_or_else(|| panic!("{file_name} should be bundled"));
    let set: ThemeSet = serde_json::from_str(contents)
        .unwrap_or_else(|err| panic!("{file_name} theme should parse: {err}"));
    let config = set
        .themes
        .into_iter()
        .find(|theme| theme.name.as_ref() == theme_name)
        .unwrap_or_else(|| panic!("{theme_name} should exist in {file_name}"));
    let mode = config.mode;

    cx.update(|window, app| {
        Theme::global_mut(app).apply_config(&Rc::new(config));
        crate::theme::change_theme(mode, Some(window), app);
    });
    refresh(cx);
}

fn assert_sync_notify_switch_toggles(
    view: &Entity<AppRoot>,
    cx: &mut gpui::VisualTestContext,
    expected_mode: ThemeMode,
    expected_theme_name: Option<&str>,
) {
    let (mode, theme_name, initial) = cx.update(|_, app| {
        let theme = app.theme();
        (
            theme.mode,
            theme.theme_name().to_string(),
            view.read(app).sync_notify_enabled,
        )
    });
    assert_eq!(mode, expected_mode, "theme mode should be applied");
    if let Some(expected_theme_name) = expected_theme_name {
        assert_eq!(
            theme_name, expected_theme_name,
            "the requested bundled theme should be active"
        );
    }
    assert!(!initial, "each theme scenario should start switched off");

    click_debug_element(cx, "sync-notify-toggle");
    let checked = cx.update(|_, app| view.read(app).sync_notify_enabled);
    assert!(checked, "clicking the rendered switch should turn it on");

    click_debug_element(cx, "sync-notify-toggle");
    let checked = cx.update(|_, app| view.read(app).sync_notify_enabled);
    assert!(
        !checked,
        "clicking the rendered switch again should turn it off"
    );
}

fn wheel_to_end(cx: &mut gpui::VisualTestContext, viewport_selector: &'static str, delta_y: f32) {
    let viewport = cx
        .debug_bounds(viewport_selector)
        .unwrap_or_else(|| panic!("{viewport_selector} should be rendered"));
    let position = point(
        viewport.origin.x + viewport.size.width / 2.0,
        viewport.origin.y + viewport.size.height / 2.0,
    );
    cx.simulate_event(ScrollWheelEvent {
        position,
        delta: ScrollDelta::Pixels(point(px(0.0), px(delta_y))),
        modifiers: Modifiers::none(),
        ..Default::default()
    });
    refresh(cx);
}

fn assert_inside_viewport(
    cx: &mut gpui::VisualTestContext,
    viewport_selector: &'static str,
    item_selector: &'static str,
) {
    let viewport = cx
        .debug_bounds(viewport_selector)
        .unwrap_or_else(|| panic!("{viewport_selector} should be rendered"));
    let item = cx
        .debug_bounds(item_selector)
        .unwrap_or_else(|| panic!("{item_selector} should be rendered"));
    let viewport_bottom = viewport.origin.y + viewport.size.height;
    let item_bottom = item.origin.y + item.size.height;
    assert!(
        item.origin.y >= viewport.origin.y && item_bottom <= viewport_bottom,
        "{item_selector} should be reachable inside {viewport_selector}: \
         viewport={viewport:?}, item={item:?}"
    );
}

fn assert_ad_split_uses_available_width(cx: &mut gpui::VisualTestContext) {
    let content = cx
        .debug_bounds("content-area")
        .expect("content area should be rendered");
    let left = cx
        .debug_bounds("ad-block-split-left-pane")
        .expect("ad-block feature pane should be rendered");
    let right = cx
        .debug_bounds("ad-block-split-right-pane")
        .expect("ad-block settings pane should be rendered");

    assert!(
        left.size.width >= px(320.0),
        "feature pane should preserve its usable minimum width: {left:?}"
    );
    assert!(
        right.size.width >= px(300.0),
        "settings pane should preserve its usable minimum width: {right:?}"
    );
    assert!(
        left.size.width <= right.size.width + px(24.0)
            && right.size.width <= left.size.width + px(24.0),
        "default split should share width instead of squeezing one side: {left:?}, {right:?}"
    );

    let content_right = content.origin.x + content.size.width;
    let right_edge = right.origin.x + right.size.width;
    assert!(
        left.origin.x <= content.origin.x + px(20.0) && right_edge >= content_right - px(20.0),
        "split panes should occupy the padded content width: {content:?}, {left:?}, {right:?}"
    );
}

fn assert_feature_split_is_usable(
    cx: &mut gpui::VisualTestContext,
    left_selector: &'static str,
    right_selector: &'static str,
) {
    let content = cx
        .debug_bounds("content-area")
        .expect("content area should be rendered");
    let feature = cx
        .debug_bounds(left_selector)
        .unwrap_or_else(|| panic!("{left_selector} should be rendered"));
    let settings = cx
        .debug_bounds(right_selector)
        .unwrap_or_else(|| panic!("{right_selector} should be rendered"));

    assert!(
        settings.size.width >= px(300.0),
        "settings pane should preserve its usable minimum width: {settings:?}"
    );

    let content_right = content.origin.x + content.size.width;
    let settings_right = settings.origin.x + settings.size.width;
    assert!(
        feature.origin.x <= content.origin.x + px(20.0)
            && settings_right >= content_right - px(20.0),
        "feature split should occupy the padded content width: \
         content={content:?}, feature={feature:?}, settings={settings:?}"
    );
}

#[gpui::test]
fn ad_block_split_fills_default_and_minimum_supported_width(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (_view, cx) = cx.add_window_view(|_, _| test_app_root(ActivePanel::AdBlock));

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    refresh(cx);
    assert_ad_split_uses_available_width(cx);

    cx.simulate_resize(size(
        px(MIN_SUPPORTED_WINDOW_WIDTH),
        px(DEFAULT_WINDOW_HEIGHT),
    ));
    refresh(cx);
    assert_ad_split_uses_available_width(cx);
}

#[gpui::test]
fn every_feature_split_keeps_settings_pane_usable_at_default_width(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| test_app_root(ActivePanel::AdBlock));

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    refresh(cx);
    assert_feature_split_is_usable(cx, "ad-block-split-left-pane", "ad-block-split-right-pane");

    cx.update(|_, app| {
        view.update(app, |root, cx| {
            root.active_panel = ActivePanel::FileSync;
            cx.notify();
        });
    });
    refresh(cx);
    assert_feature_split_is_usable(
        cx,
        "file-sync-split-left-pane",
        "file-sync-split-right-pane",
    );

    cx.update(|_, app| {
        view.update(app, |root, cx| {
            root.active_panel = ActivePanel::Services;
            cx.notify();
        });
    });
    refresh(cx);
    assert_feature_split_is_usable(
        cx,
        "service-mgr-split-left-pane",
        "service-mgr-split-right-pane",
    );
}

#[gpui::test]
fn sidebar_wheel_scroll_reaches_last_navigation_item(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| test_app_root(ActivePanel::Dashboard));

    cx.simulate_resize(size(
        px(MIN_SUPPORTED_WINDOW_WIDTH),
        px(COMPACT_WINDOW_HEIGHT),
    ));
    refresh(cx);

    let viewport = cx
        .debug_bounds("sidebar-scroll")
        .expect("sidebar viewport should be rendered");
    let max_scroll_height =
        cx.update(|_, app| view.read(app).sidebar_scroll_handle.max_offset().height);
    assert!(
        max_scroll_height > px(0.0),
        "compact window should produce sidebar overflow"
    );

    let wheel_position = point(
        viewport.origin.x + viewport.size.width / 2.0,
        viewport.origin.y + viewport.size.height / 2.0,
    );
    cx.simulate_event(ScrollWheelEvent {
        position: wheel_position,
        delta: ScrollDelta::Pixels(point(px(0.0), px(-1000.0))),
        modifiers: Modifiers::none(),
        ..Default::default()
    });
    refresh(cx);

    let scroll_offset_y = cx.update(|_, app| view.read(app).sidebar_scroll_handle.offset().y);
    assert!(
        scroll_offset_y < px(0.0),
        "wheel input should move the sidebar scroll offset"
    );

    let settings = cx
        .debug_bounds("nav-item-설정")
        .expect("last navigation item should remain rendered");
    let viewport_bottom = viewport.origin.y + viewport.size.height;
    let settings_bottom = settings.origin.y + settings.size.height;
    assert!(
        settings.origin.y >= viewport.origin.y && settings_bottom <= viewport_bottom,
        "the last navigation item should be reachable in the compact viewport: \
         viewport={viewport:?}, settings={settings:?}"
    );
}

#[gpui::test]
fn rendered_switch_toggles_in_light_dark_and_missing_switch_token_theme(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = test_app_root(ActivePanel::FileSync);
        root.sync_notify_enabled = false;
        root
    });

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    apply_theme_mode(cx, ThemeMode::Light);
    assert_sync_notify_switch_toggles(&view, cx, ThemeMode::Light, None);

    apply_theme_mode(cx, ThemeMode::Dark);
    assert_sync_notify_switch_toggles(&view, cx, ThemeMode::Dark, None);

    // Alduin omits the switch palette tokens that caused the original visibility regression.
    apply_bundled_theme(cx, "alduin.json", "Alduin");
    assert_sync_notify_switch_toggles(&view, cx, ThemeMode::Dark, Some("Alduin"));
}

#[gpui::test]
fn file_sync_both_panes_scroll_to_their_last_controls_at_compact_height(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = test_app_root(ActivePanel::FileSync);
        root.sync_jobs = (0..12)
            .map(|index| SyncJob {
                name: format!("검증 작업 {index}"),
                source: format!(r"D:\validation\source-{index}"),
                target: format!(r"D:\validation\target-{index}"),
                ..SyncJob::default()
            })
            .collect();
        root.selected_sync_job = Some(0);
        root.sync_failures = (0..12)
            .map(|index| SyncFailure {
                path: format!("failure-{index}.txt"),
                reason: "검증용 공유 위반".to_string(),
            })
            .collect();
        root
    });

    cx.simulate_resize(size(
        px(MIN_SUPPORTED_WINDOW_WIDTH),
        px(COMPACT_WINDOW_HEIGHT),
    ));
    refresh(cx);

    let (left_max, right_max) = cx.update(|_, app| {
        let root = view.read(app);
        (
            root.sync_left_scroll.max_offset().height,
            root.sync_right_scroll.max_offset().height,
        )
    });
    assert!(
        left_max > px(0.0),
        "compact File Sync left pane should overflow"
    );
    assert!(
        right_max > px(0.0),
        "compact File Sync settings pane should overflow"
    );

    wheel_to_end(cx, "file-sync-left", -5000.0);
    let left_offset = cx.update(|_, app| view.read(app).sync_left_scroll.offset().y);
    assert!(
        left_offset < px(0.0),
        "wheel input should move the left pane scroll offset"
    );
    assert_inside_viewport(cx, "file-sync-left", "sync-failure-row-11");

    wheel_to_end(cx, "file-sync-right", -5000.0);
    let right_offset = cx.update(|_, app| view.read(app).sync_right_scroll.offset().y);
    assert!(
        right_offset < px(0.0),
        "wheel input should move the right pane scroll offset"
    );
    assert_inside_viewport(cx, "file-sync-right", "sync-opt-mirror-row");
}
