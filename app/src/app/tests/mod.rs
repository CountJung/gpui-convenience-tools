//! GPUI 회귀 테스트의 공용 픽스처와 헬퍼.
//!
//! 시나리오별 테스트는 하위 모듈이 소유한다.

mod file_sync;
mod interval;
mod layout;
mod theme;

use super::*;
use crate::config::{SyncJob, BUNDLED_THEMES};
use crate::platform::NativeWindowHandle;
use crate::sync::{SyncFailure, SyncOutcome};
use anyhow::Result;
use gpui::{
    point, px, size, Modifiers, MouseButton, ScrollDelta, ScrollWheelEvent, TestAppContext,
};
use gpui_component::theme::{ActiveTheme, Theme, ThemeMode, ThemeSet};
use std::sync::atomic::Ordering as AtomicOrdering;
use std::{cell::Cell, rc::Rc};

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
        interval_picker: IntervalPicker {
            presets: crate::config::default_interval_presets(),
            ..IntervalPicker::default()
        },
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
        sync_running: None,
        suppressed_sync_failures: HashSet::new(),
        sync_notify_enabled: true,
        sync_name_input: None,
        sync_source_input: None,
        sync_target_input: None,
        sync_page_scroll: ScrollHandle::default(),
        sync_state: Arc::new(Mutex::new(SyncSharedState::default())),
        external_side_effects_enabled: false,
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
