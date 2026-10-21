use super::*;
use crate::app::tests::{click_debug_element, initialize_components, refresh, test_app_root};
use crate::process_monitor::ProcessIdentity;
use gpui::{point, px, size, Entity, Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext};
use std::path::PathBuf;

fn fixture_row(pid: u32) -> ProcessRow {
    ProcessRow {
        identity: ProcessIdentity {
            pid,
            start_time: 123,
            path: PathBuf::from("D:/fixture/worker.exe"),
            creation_stamp: Some(456),
        },
        name: format!("fixture-worker-{pid}"),
        cpu_percent: 25.0,
        memory_bytes: 1024 * 1024,
        blocked_reason: None,
    }
}

fn add_window(
    cx: &mut TestAppContext,
    root: AppRoot,
) -> (Entity<AppRoot>, &mut gpui::VisualTestContext) {
    let mut view = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let inner = cx.new(|_| root);
        view = Some(inner.clone());
        gpui_component::Root::new(inner, window, cx)
    });
    (view.unwrap(), cx)
}

#[gpui::test]
fn process_monitor_selection_confirmation_cancel_and_guarded_failure(cx: &mut TestAppContext) {
    initialize_components(cx);
    let mut root = test_app_root(ActivePanel::ProcessMonitor);
    root.process_monitor.rows = vec![fixture_row(88)];
    let (view, cx) = add_window(cx, root);
    cx.simulate_resize(size(px(1000.0), px(1000.0)));
    refresh(cx);
    assert!(cx.debug_bounds("process-confirm-kill").is_none());
    click_debug_element(cx, "process-row-88");
    view.read_with(cx, |root, _| {
        assert_eq!(
            root.process_monitor.selected.as_ref().unwrap().identity.pid,
            88
        );
        assert!(!root.process_monitor.terminating);
        assert!(root.process_monitor.worker.is_none());
    });
    assert!(cx.debug_bounds("process-selected-name").is_some());
    click_debug_element(cx, "process-cancel-kill");
    assert!(view.read_with(cx, |root, _| root.process_monitor.selected.is_none()));
    click_debug_element(cx, "process-row-88");
    click_debug_element(cx, "process-confirm-kill");
    view.read_with(cx, |root, _| {
        assert!(root.process_monitor.failed);
        assert_eq!(
            root.process_monitor.status,
            ProcessError::AccessDenied.to_string()
        );
        assert!(
            root.process_monitor.worker.is_none(),
            "GPUI tests must never contact real process termination"
        );
    });
}

#[gpui::test]
fn process_monitor_sort_interval_search_and_transient_state(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = add_window(cx, test_app_root(ActivePanel::ProcessMonitor));
    cx.simulate_resize(size(px(1000.0), px(900.0)));
    refresh(cx);
    click_debug_element(cx, "process-sort-memory");
    assert_eq!(
        view.read_with(cx, |root, _| root.process_monitor.config.sort),
        ProcessSort::Memory
    );
    click_debug_element(cx, "process-count");
    assert_eq!(
        view.read_with(cx, |root, _| root.process_monitor.config.top_count),
        100
    );
    let interval = view.read_with(cx, |root, _| {
        root.process_monitor.interval_input.clone().unwrap()
    });
    cx.update(|window, cx| interval.update(cx, |state, cx| state.focus(window, cx)));
    cx.simulate_keystrokes("ctrl-a 1 1");
    refresh(cx);
    assert_eq!(
        view.read_with(cx, |root, _| root
            .process_monitor
            .config
            .refresh_interval_secs),
        11
    );
    cx.simulate_keystrokes("ctrl-a 0");
    refresh(cx);
    view.read_with(cx, |root, _| {
        assert_eq!(root.process_monitor.config.refresh_interval_secs, 11);
        assert!(root.process_monitor.failed);
    });
    let search = view.read_with(cx, |root, _| {
        root.process_monitor.search_input.clone().unwrap()
    });
    cx.update(|window, cx| search.update(cx, |state, cx| state.focus(window, cx)));
    cx.simulate_keystrokes("f i x t u r e");
    refresh(cx);
    let config = view.read_with(cx, |root, _| {
        assert_eq!(root.process_monitor.query, "fixture");
        root.process_monitor.config.clone()
    });
    let restored = ProcessMonitorState::from_config(config);
    assert_eq!(restored.config.refresh_interval_secs, 11);
    assert_eq!(restored.config.sort, ProcessSort::Memory);
    assert!(restored.query.is_empty());
    assert!(restored.rows.is_empty());
}

#[gpui::test]
fn process_monitor_preferences_save_and_reopen_without_search_or_snapshot(cx: &mut TestAppContext) {
    let _isolation = crate::config::tests::IsolatedDataDir::new("process-monitor-gpui");
    initialize_components(cx);
    let (view, cx) = add_window(cx, test_app_root(ActivePanel::ProcessMonitor));
    cx.simulate_resize(size(px(1000.0), px(900.0)));
    refresh(cx);
    // Enable only the isolated settings writes after lazy inputs are initialized.
    // This test root has no OS timer and never requests a scan or termination.
    view.update(cx, |root, _| root.sync.external_side_effects_enabled = true);
    click_debug_element(cx, "process-sort-memory");
    click_debug_element(cx, "process-count");
    let interval = view.read_with(cx, |root, _| {
        root.process_monitor.interval_input.clone().unwrap()
    });
    cx.update(|window, cx| interval.update(cx, |state, cx| state.focus(window, cx)));
    cx.simulate_keystrokes("ctrl-a 1 7");
    refresh(cx);
    view.update(cx, |root, _| {
        root.sync.external_side_effects_enabled = false;
        root.process_monitor.query = "private-transient-query".into();
        root.process_monitor.rows = vec![fixture_row(88)];
        assert!(root.process_monitor.worker.is_none());
        assert!(!root.process_monitor.save_error);
    });
    let restored = crate::config::load_config().unwrap().unwrap();
    assert_eq!(restored.process_monitor.sort, ProcessSort::Memory);
    assert_eq!(restored.process_monitor.refresh_interval_secs, 17);
    assert_eq!(restored.process_monitor.top_count, 100);
    let serialized = serde_json::to_string(&restored).unwrap();
    assert!(!serialized.contains("private-transient-query"));
    assert!(!serialized.contains("fixture-worker"));
    let reopened = ProcessMonitorState::from_config(restored.process_monitor);
    assert!(reopened.query.is_empty());
    assert!(reopened.rows.is_empty());
    assert!(reopened.selected.is_none());
    assert_eq!(reopened.config.refresh_interval_secs, 17);
}

#[gpui::test]
fn process_monitor_background_results_and_identity_changes_cancel_confirmation(
    cx: &mut TestAppContext,
) {
    initialize_components(cx);
    let (view, cx) = add_window(cx, test_app_root(ActivePanel::ProcessMonitor));
    refresh(cx);
    view.update(cx, |root, cx| {
        root.process_monitor.selected = Some(fixture_row(88));
        root.process_monitor.busy = true;
        let mut replacement = fixture_row(88);
        replacement.identity.start_time += 1;
        root.process_monitor.worker =
            Some(MonitorWorker::with_test_reply(WorkerReply::Snapshot(vec![
                replacement,
            ])));
        root.tick_process_monitor(cx);
        assert!(root.process_monitor.selected.is_none());
        assert!(!root.process_monitor.busy);
        assert!(root.process_monitor.failed);
        root.process_monitor.worker = Some(MonitorWorker::with_test_reply(
            WorkerReply::Terminated(Err(ProcessError::Protected)),
        ));
        root.tick_process_monitor(cx);
        assert!(root.process_monitor.failed);
        assert!(!root.process_monitor.terminating);
        let previous_identity = root.process_monitor.rows[0].identity.clone();
        root.process_monitor.selected = Some(fixture_row(88));
        root.process_monitor.busy = true;
        root.process_monitor.worker = Some(MonitorWorker::with_test_reply(
            WorkerReply::ScanFailed(ProcessError::ScanFailed),
        ));
        root.tick_process_monitor(cx);
        assert_eq!(root.process_monitor.rows[0].identity, previous_identity);
        assert!(root.process_monitor.selected.is_none());
        assert!(root.process_monitor.failed);
        assert!(!root.process_monitor.busy);
        assert!(root.process_monitor.status.contains("오래된 정보"));
        root.process_monitor.worker = None;
        root.active_panel = ActivePanel::Dashboard;
        root.tick_process_monitor(cx);
        assert!(root.process_monitor.worker.is_none());
    });
}

#[gpui::test]
fn process_monitor_width_columns_and_overflow_scroll(cx: &mut TestAppContext) {
    initialize_components(cx);
    let mut root = test_app_root(ActivePanel::ProcessMonitor);
    root.process_monitor.rows = (100..140)
        .map(|pid| {
            let mut row = fixture_row(pid);
            row.name = "very-long-process-name-".repeat(30);
            row.identity.path = PathBuf::from("D:/long-fixture-folder/".repeat(30));
            row
        })
        .collect();
    let (view, cx) = add_window(cx, root);
    for width in [920.0, 1000.0, 1500.0] {
        cx.simulate_resize(size(px(width), px(480.0)));
        view.update(cx, |root, cx| {
            root.process_monitor
                .scroll
                .set_offset(point(px(0.0), px(0.0)));
            cx.notify();
        });
        refresh(cx);
        let viewport = cx.debug_bounds("process-page").unwrap();
        let interval = cx.debug_bounds("process-interval-row").unwrap();
        let helper = cx.debug_bounds("process-interval-help").unwrap();
        let save_status = cx.debug_bounds("process-save-status").unwrap();
        let status = cx.debug_bounds("process-status").unwrap();
        assert!(
            helper.size.width >= px(400.0),
            "helper must have a readable line width at {width}"
        );
        assert!(helper.size.width >= viewport.size.width - px(40.0));
        assert!(helper.size.height > px(0.0));
        assert!(helper.origin.y >= interval.origin.y + interval.size.height);
        assert!(save_status.origin.y >= helper.origin.y + helper.size.height);
        assert!(status.origin.y >= save_status.origin.y + save_status.size.height);
        let name = cx.debug_bounds("process-name-100").unwrap();
        assert!(name.size.width >= px(160.0));
        let row = cx.debug_bounds("process-row-100").unwrap();
        assert!(row.origin.x + row.size.width <= viewport.origin.x + viewport.size.width + px(1.0));
        for (header, cell) in [
            ("process-cpu-header", "process-cpu-100"),
            ("process-memory-header", "process-memory-100"),
        ] {
            let header = cx.debug_bounds(header).unwrap();
            let cell = cx.debug_bounds(cell).unwrap();
            assert_eq!(header.size.width, cell.size.width);
            assert_eq!(header.origin.x, cell.origin.x);
        }
        assert!(cx.debug_bounds("process-pid-100").unwrap().size.width >= px(90.0));
        let position = point(
            viewport.origin.x + viewport.size.width / 2.0,
            viewport.origin.y + viewport.size.height / 2.0,
        );
        cx.simulate_event(ScrollWheelEvent {
            position,
            delta: ScrollDelta::Pixels(point(px(0.0), px(-20000.0))),
            modifiers: Modifiers::none(),
            ..Default::default()
        });
        refresh(cx);
        let end = cx.debug_bounds("process-list-end").unwrap();
        assert!(end.origin.y >= viewport.origin.y - px(1.0));
        assert!(
            end.origin.y + end.size.height <= viewport.origin.y + viewport.size.height + px(1.0)
        );
        assert!(
            view.read_with(cx, |root, _| root.process_monitor.scroll.offset().y
                < px(0.0))
        );
    }
}
