use super::*;
use crate::app::{
    tests::{click_debug_element, initialize_components, refresh, test_app_root},
    ActivePanel,
};
use crate::clipboard_history::tests::{png, Fixture};
use gpui::{point, px, size, TestAppContext};

fn add_window(cx: &mut TestAppContext) -> (Entity<AppRoot>, &mut gpui::VisualTestContext) {
    let mut view = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let root = cx.new(|_| test_app_root(ActivePanel::ClipboardHistory));
        view = Some(root.clone());
        gpui_component::Root::new(root, window, cx)
    });
    let view = view.unwrap();
    cx.simulate_resize(size(px(1200.0), px(1100.0)));
    refresh(cx);
    (view, cx)
}

fn poll(view: &Entity<AppRoot>, cx: &mut gpui::VisualTestContext) {
    view.update(cx, |this, cx| {
        this.clipboard.last_poll = None;
        this.tick_clipboard(cx);
    });
    refresh(cx);
}

#[gpui::test]
fn clipboard_ui_opt_in_poll_throttle_and_type_policy(cx: &mut TestAppContext) {
    #[cfg(target_os = "windows")]
    assert!(crate::app::state::NAV_TOOLS
        .iter()
        .any(|entry| entry.0 == ActivePanel::ClipboardHistory));
    #[cfg(not(target_os = "windows"))]
    assert!(!crate::app::state::NAV_TOOLS
        .iter()
        .any(|entry| entry.0 == ActivePanel::ClipboardHistory));
    initialize_components(cx);
    let (view, cx) = add_window(cx);
    cx.write_to_clipboard(ClipboardItem::new_string("synthetic-first".into()));
    poll(&view, cx);
    view.read_with(cx, |this, _| {
        assert!(this.clipboard.history.entries.is_empty())
    });
    click_debug_element(cx, "clipboard-text-switch");
    poll(&view, cx);
    view.read_with(cx, |this, _| {
        assert!(this.clipboard.config.collect_text);
        assert!(!this.clipboard.config.collect_images);
        assert_eq!(this.clipboard.history.entries.len(), 1);
    });
    cx.write_to_clipboard(ClipboardItem::new_string("synthetic-second".into()));
    view.update(cx, |this, cx| {
        this.clipboard.last_poll = Some(Instant::now());
        this.tick_clipboard(cx);
    });
    view.read_with(cx, |this, _| {
        assert_eq!(this.clipboard.history.entries.len(), 1)
    });
    poll(&view, cx);
    view.read_with(cx, |this, _| {
        assert_eq!(this.clipboard.history.entries.len(), 2)
    });
    cx.write_to_clipboard(ClipboardItem::new_string("password=synthetic".into()));
    poll(&view, cx);
    view.read_with(cx, |this, _| {
        assert_eq!(this.clipboard.history.entries.len(), 2);
        assert!(this.clipboard.status.contains("민감"));
    });
    cx.write_to_clipboard(ClipboardItem::new_image(&Image::from_bytes(
        ImageFormat::Png,
        png(),
    )));
    poll(&view, cx);
    view.read_with(cx, |this, _| {
        assert_eq!(this.clipboard.history.entries.len(), 2)
    });
    click_debug_element(cx, "clipboard-image-switch");
    poll(&view, cx);
    view.read_with(cx, |this, _| {
        assert_eq!(this.clipboard.history.entries.len(), 3)
    });
    // 패널 비활성 상태에서도 복원된 정책으로 수집한다.
    view.update(cx, |this, _| this.active_panel = ActivePanel::Dashboard);
    cx.write_to_clipboard(ClipboardItem::new_string("background-synthetic".into()));
    poll(&view, cx);
    view.read_with(cx, |this, _| {
        assert_eq!(this.clipboard.history.entries.len(), 4)
    });
}

#[gpui::test]
fn clipboard_ui_search_select_copy_pin_delete_and_clear(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = add_window(cx);
    view.update(cx, |this, cx| {
        this.clipboard.config.collect_text = true;
        this.capture_clipboard_candidate(Candidate::Text("alpha-fixture".into()), cx);
        this.capture_clipboard_candidate(Candidate::Text("beta-fixture".into()), cx);
    });
    refresh(cx);
    click_debug_element(cx, "clipboard-select-2");
    click_debug_element(cx, "clipboard-copy-selected");
    assert_eq!(
        cx.read_from_clipboard().unwrap().text().as_deref(),
        Some("beta-fixture")
    );
    click_debug_element(cx, "clipboard-pin-2");
    view.read_with(cx, |this, _| {
        assert!(this.clipboard.history.entries[0].pinned)
    });
    let search = view.read_with(cx, |this, _| this.clipboard.search.clone().unwrap());
    cx.update(|window, cx| search.update(cx, |input, cx| input.set_value("ALPHA", window, cx)));
    refresh(cx);
    view.read_with(cx, |this, _| {
        let ids: Vec<_> = this
            .clipboard
            .visible_entries()
            .map(|entry| entry.id)
            .collect();
        assert_eq!(ids, vec![1]);
        assert!(this.clipboard.selected.is_none());
    });
    // GPUI 0.2.2는 사라진 selector의 bounds를 지우지 않으므로 새 결과 수의 렌더를 확인한다.
    assert!(cx.debug_bounds("clipboard-result-count-1").is_some());
    click_debug_element(cx, "clipboard-delete-1");
    view.read_with(cx, |this, _| {
        assert_eq!(this.clipboard.history.entries.len(), 1)
    });
    click_debug_element(cx, "clipboard-clear");
    view.read_with(cx, |this, _| {
        assert_eq!(this.clipboard.history.entries.len(), 1)
    });
    click_debug_element(cx, "clipboard-clear");
    view.read_with(cx, |this, _| {
        assert!(this.clipboard.history.entries.is_empty())
    });
    // 초기화는 테스트 mock 클립보드조차 지우지 않는다.
    assert_eq!(
        cx.read_from_clipboard().unwrap().text().as_deref(),
        Some("beta-fixture")
    );
}

#[gpui::test]
fn clipboard_ui_png_recopy_and_missing_file_failure(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = add_window(cx);
    view.update(cx, |this, cx| {
        this.clipboard.config.collect_images = true;
        this.capture_clipboard_candidate(Candidate::Png(png()), cx);
        this.select_clipboard_entry(1, cx);
        assert!(this.clipboard.selected_image.is_some());
        this.copy_clipboard_entry(1, cx);
    });
    let item = cx.read_from_clipboard().unwrap();
    assert!(
        matches!(item.entries().first(), Some(ClipboardEntry::Image(image)) if image.format == ImageFormat::Png)
    );
    view.update(cx, |this, cx| {
        this.clipboard.image_cache.clear();
        this.copy_clipboard_entry(1, cx);
    });
    view.read_with(cx, |this, _| {
        assert!(this.clipboard.failed);
        assert!(this.clipboard.status.contains("없습니다"));
    });
}

#[gpui::test]
fn clipboard_ui_policy_validation_retention_and_restart_store(cx: &mut TestAppContext) {
    initialize_components(cx);
    let fixture = Fixture::new();
    let (view, cx) = add_window(cx);
    let (limits, exclusions) = view.read_with(cx, |this, _| {
        (
            this.clipboard.limits.clone().unwrap(),
            this.clipboard.exclusions.clone().unwrap(),
        )
    });
    cx.update(|window, cx| limits.update(cx, |input, cx| input.set_value("0 9999 1", window, cx)));
    refresh(cx);
    click_debug_element(cx, "clipboard-policy-apply");
    view.read_with(cx, |this, _| {
        assert!(this.clipboard.failed);
        assert_eq!(this.clipboard.config.max_entries, 100);
    });
    cx.update(|window, cx| {
        limits.update(cx, |input, cx| input.set_value("2 1 64", window, cx));
        exclusions.update(cx, |input, cx| {
            input.set_value("synthetic-excluded", window, cx)
        });
    });
    refresh(cx);
    click_debug_element(cx, "clipboard-policy-apply");
    view.update(cx, |this, cx| {
        this.clipboard.store = Some(HistoryStore::new(&fixture.path));
        this.clipboard.config.collect_text = true;
        this.capture_clipboard_candidate(Candidate::Text("synthetic-excluded".into()), cx);
        assert!(this.clipboard.history.entries.is_empty());
        this.capture_clipboard_candidate(Candidate::Text("restart-fixture".into()), cx);
        assert!(!this.clipboard.failed);
    });
    let config = view.read_with(cx, |this, _| this.clipboard.config.clone());
    let restored = HistoryStore::new(&fixture.path)
        .load(&config, now())
        .unwrap();
    assert_eq!(restored.entries.len(), 1);
    assert_eq!(restored.entries[0].preview(), "restart-fixture");
    assert_eq!(config.max_entries, 2);
    assert_eq!(config.max_age_hours, 1);
    assert_eq!(config.max_bytes, 64 * 1024);
    view.update(cx, |this, cx| {
        this.clipboard.history.entries[0].created_at = now().saturating_sub(3600);
        this.clipboard.last_poll = None;
        this.clipboard.config.collect_text = false;
        this.tick_clipboard(cx);
    });
    refresh(cx);
    assert!(HistoryStore::new(&fixture.path)
        .load(&config, now())
        .unwrap()
        .entries
        .is_empty());
}

#[gpui::test]
fn clipboard_ui_corruption_blocks_capture_and_requires_clear_confirmation(cx: &mut TestAppContext) {
    initialize_components(cx);
    let fixture = Fixture::new();
    let store = HistoryStore::new(&fixture.path);
    store.save(&History::default(), None).unwrap();
    let index = fixture.path.join("clipboard-history/index.json");
    std::fs::write(&index, b"broken-fixture").unwrap();
    let (view, cx) = add_window(cx);
    view.update(cx, |this, cx| {
        this.clipboard.store = Some(store);
        this.clipboard.loaded = false;
        this.clipboard.config.collect_text = true;
        this.clipboard.last_poll = None;
        this.tick_clipboard(cx);
        assert!(this.clipboard.busy);
    });
    refresh(cx);
    view.read_with(cx, |this, _| {
        assert!(this.clipboard.blocked);
        assert!(this.clipboard.failed);
    });
    assert_eq!(std::fs::read(&index).unwrap(), b"broken-fixture");
    refresh(cx);
    click_debug_element(cx, "clipboard-clear");
    assert_eq!(std::fs::read(&index).unwrap(), b"broken-fixture");
    click_debug_element(cx, "clipboard-clear");
    view.read_with(cx, |this, _| assert!(!this.clipboard.blocked));
    assert!(HistoryStore::new(&fixture.path)
        .load(&ClipboardConfig::default(), now())
        .is_ok());
}

#[gpui::test]
fn clipboard_ui_widths_and_scrolling_preserve_controls(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = add_window(cx);
    for (width, height) in [(920.0, 480.0), (1200.0, 800.0), (1500.0, 900.0)] {
        cx.simulate_resize(size(px(width), px(height)));
        view.update(cx, |this, cx| {
            this.clipboard.scroll.set_offset(point(px(0.0), px(0.0)));
            cx.notify();
        });
        refresh(cx);
        let viewport = cx.debug_bounds("clipboard-page").unwrap();
        for selector in [
            "clipboard-limits",
            "clipboard-exclusions",
            "clipboard-search",
            "clipboard-policy-apply",
        ] {
            let bounds = cx.debug_bounds(selector).unwrap();
            assert!(bounds.size.width > px(100.0));
            assert!(bounds.origin.x >= viewport.origin.x);
            assert!(
                bounds.origin.x + bounds.size.width
                    <= viewport.origin.x + viewport.size.width + px(1.0)
            );
        }
        cx.simulate_event(gpui::ScrollWheelEvent {
            position: point(
                viewport.origin.x + viewport.size.width / 2.0,
                viewport.origin.y + viewport.size.height / 2.0,
            ),
            delta: gpui::ScrollDelta::Pixels(point(px(0.0), px(-3000.0))),
            modifiers: gpui::Modifiers::none(),
            ..Default::default()
        });
        refresh(cx);
        let clear = cx.debug_bounds("clipboard-policy-apply").unwrap();
        assert!(
            clear.origin.y + clear.size.height
                <= viewport.origin.y + viewport.size.height + px(1.0)
        );
    }
}

#[gpui::test]
fn clipboard_ui_sensitive_marker_and_unavailable_probe_fail_closed(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = add_window(cx);
    view.update(cx, |this, _| this.clipboard.config.collect_text = true);
    cx.write_to_clipboard(ClipboardItem::new_string(
        "marker-protected-synthetic".into(),
    ));
    {
        let _probe = history::windows::test_probe(history::windows::Probe::Sensitive);
        poll(&view, cx);
        view.read_with(cx, |this, _| {
            assert!(this.clipboard.history.entries.is_empty());
            assert!(this.clipboard.status.contains("민감"));
        });
    }
    {
        let _probe = history::windows::test_probe(history::windows::Probe::Unavailable);
        poll(&view, cx);
        view.read_with(cx, |this, _| {
            assert!(this.clipboard.history.entries.is_empty());
            assert!(this.clipboard.status.contains("확인 불가"));
        });
    }
}

#[gpui::test]
fn clipboard_ui_settings_autosave_preserve_other_fields_and_rehydrate(cx: &mut TestAppContext) {
    let _isolation = crate::config::tests::IsolatedDataDir::new("clipboard-settings-ui");
    crate::config::update_config(|config| config.scan_interval_secs = 73).unwrap();
    initialize_components(cx);
    let (view, cx) = add_window(cx);
    view.update(cx, |this, _| this.sync.external_side_effects_enabled = true);
    click_debug_element(cx, "clipboard-text-switch");
    click_debug_element(cx, "clipboard-image-switch");
    view.update(cx, |this, cx| {
        this.clipboard
            .scroll
            .set_offset(point(px(0.0), px(-3000.0)));
        cx.notify();
    });
    refresh(cx);
    click_debug_element(cx, "clipboard-sensitive-switch");
    let restored = crate::config::load_config().unwrap().unwrap();
    assert!(restored.clipboard.collect_text);
    assert!(restored.clipboard.collect_images);
    assert!(!restored.clipboard.exclude_sensitive);
    assert_eq!(restored.scan_interval_secs, 73);
    view.update(cx, |this, _| {
        this.save_current_config().unwrap();
    });
    let config_path = crate::config::effective_config_path();
    let saved = std::fs::read(&config_path).unwrap();
    std::fs::write(&config_path, b"{broken").unwrap();
    view.update(cx, |this, cx| this.persist_clipboard_config(cx));
    view.read_with(cx, |this, _| assert!(this.clipboard.save_error));
    assert_eq!(std::fs::read(&config_path).unwrap(), b"{broken");
    std::fs::write(&config_path, saved).unwrap();
    view.update(cx, |this, cx| {
        this.save_current_config().unwrap();
        assert!(!this.clipboard.save_error);
        this.sync.external_side_effects_enabled = false;
        this.clipboard = ClipboardState::from_config(restored.clipboard);
        cx.notify();
    });
    refresh(cx);
    view.read_with(cx, |this, cx| {
        assert!(this.clipboard.config.collect_text);
        assert!(this.clipboard.config.collect_images);
        assert!(!this.clipboard.config.exclude_sensitive);
        assert!(this.clipboard.query.is_empty());
        assert!(this
            .clipboard
            .search
            .as_ref()
            .unwrap()
            .read(cx)
            .value()
            .is_empty());
    });
}

#[gpui::test]
fn clipboard_ui_background_single_flight_keeps_ui_available(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = add_window(cx);
    cx.write_to_clipboard(ClipboardItem::new_string("worker-first-fixture".into()));
    view.update(cx, |this, cx| {
        this.clipboard.config.collect_text = true;
        this.clipboard.last_poll = None;
        this.tick_clipboard(cx);
        assert!(this.clipboard.busy);
        // UI에 결과가 즉시 쓰이지 않는다. 처리 중 재호출도 새 작업을 만들지 않는다.
        assert!(this.clipboard.history.entries.is_empty());
        cx.write_to_clipboard(ClipboardItem::new_string("worker-second-fixture".into()));
        this.clipboard.last_poll = None;
        this.tick_clipboard(cx);
        this.clear_clipboard_history(cx);
        assert!(!this.clipboard.confirm_clear);
        assert!(this.clipboard.busy);
        // 수집 OFF는 긴 PNG/파일 작업 완료를 기다리지 않고 즉시 반영한다.
        this.set_clipboard_collection(false, false, cx);
        assert!(!this.clipboard.config.collect_text);
        this.active_panel = ActivePanel::Dashboard;
    });
    refresh(cx);
    view.read_with(cx, |this, _| {
        assert!(!this.clipboard.busy);
        assert_eq!(this.clipboard.history.entries.len(), 1);
        assert_eq!(
            this.clipboard.history.entries[0].preview(),
            "worker-first-fixture"
        );
        assert_eq!(this.active_panel, ActivePanel::Dashboard);
    });
    poll(&view, cx);
    view.read_with(cx, |this, _| {
        assert_eq!(this.clipboard.history.entries.len(), 1)
    });
}

#[gpui::test]
fn clipboard_ui_expired_recopy_persists_prune_and_removes_owned_png(cx: &mut TestAppContext) {
    initialize_components(cx);
    let fixture = Fixture::new();
    let (view, cx) = add_window(cx);
    let store = HistoryStore::new(&fixture.path);
    let mut image_path = None;
    view.update(cx, |this, cx| {
        this.clipboard.store = Some(store.clone());
        this.clipboard.config.collect_images = true;
        this.capture_clipboard_candidate(Candidate::Png(png()), cx);
        this.clipboard.config.collect_images = false;
        let entry = &mut this.clipboard.history.entries[0];
        entry.created_at = 1;
        if let Content::Png { digest, .. } = &entry.content {
            image_path = Some(
                fixture
                    .path
                    .join("clipboard-history")
                    .join(format!("{digest}.png")),
            );
        }
        store.save(&this.clipboard.history, None).unwrap();
        this.copy_clipboard_entry(1, cx);
        assert!(this.clipboard.busy);
        this.tick_clipboard(cx);
    });
    refresh(cx);
    poll(&view, cx);
    assert!(!image_path.unwrap().exists());
    assert!(store
        .load(&ClipboardConfig::default(), now())
        .unwrap()
        .entries
        .is_empty());
    let json = std::fs::read(fixture.path.join("clipboard-history/index.json")).unwrap();
    let index: serde_json::Value = serde_json::from_slice(&json).unwrap();
    assert!(index["history"]["entries"].as_array().unwrap().is_empty());
    view.read_with(cx, |this, _| {
        assert!(this.clipboard.history.entries.is_empty());
        assert!(!this.clipboard.busy);
    });
    assert!(cx.read_from_clipboard().is_none());
}

#[gpui::test]
fn clipboard_ui_recopy_then_clear_baselines_current_sequence(cx: &mut TestAppContext) {
    let _probe = history::windows::test_probe(history::windows::Probe::Allowed(10));
    initialize_components(cx);
    let (view, cx) = add_window(cx);
    view.update(cx, |this, _| this.clipboard.config.collect_text = true);
    cx.write_to_clipboard(ClipboardItem::new_string("clear-A-fixture".into()));
    poll(&view, cx);
    history::windows::test_sequence(11);
    cx.write_to_clipboard(ClipboardItem::new_string("clear-B-fixture".into()));
    poll(&view, cx);
    history::windows::test_sequence(12);
    view.update(cx, |this, cx| {
        this.copy_clipboard_entry(1, cx);
        this.clear_clipboard_history(cx);
        this.clear_clipboard_history(cx);
        assert_eq!(this.clipboard.last_sequence, Some(12));
    });
    poll(&view, cx);
    view.read_with(cx, |this, _| {
        assert!(this.clipboard.history.entries.is_empty())
    });
    assert_eq!(
        cx.read_from_clipboard().unwrap().text().as_deref(),
        Some("clear-A-fixture")
    );
    // 다른 sequence라도 같은 A는 baseline fingerprint와 같아 다시 담지 않는다.
    history::windows::test_sequence(13);
    poll(&view, cx);
    view.read_with(cx, |this, _| {
        assert!(this.clipboard.history.entries.is_empty())
    });
    history::windows::test_sequence(14);
    cx.write_to_clipboard(ClipboardItem::new_string("clear-C-fixture".into()));
    poll(&view, cx);
    view.read_with(cx, |this, _| {
        assert_eq!(this.clipboard.history.entries.len(), 1);
        assert_eq!(
            this.clipboard.history.entries[0].preview(),
            "clear-C-fixture"
        );
    });
}

#[gpui::test]
fn clipboard_ui_first_history_row_visible_at_default_window(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = add_window(cx);
    view.update(cx, |this, cx| {
        this.clipboard.config.collect_text = true;
        this.capture_clipboard_candidate(Candidate::Text("visible-history-fixture".into()), cx);
        this.clipboard.config.collect_text = false;
        this.clipboard.scroll.set_offset(point(px(0.0), px(0.0)));
        cx.notify();
    });
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    refresh(cx);
    let viewport = cx.debug_bounds("clipboard-page").unwrap();
    let row = cx.debug_bounds("clipboard-entry-1").unwrap();
    assert!(row.origin.y >= viewport.origin.y);
    assert!(row.origin.y + row.size.height <= viewport.origin.y + viewport.size.height);
    assert!(row.origin.x >= viewport.origin.x);
    assert!(row.origin.x + row.size.width <= viewport.origin.x + viewport.size.width + px(1.0));
}
