use super::*;
use crate::config::tests::IsolatedDataDir;

#[gpui::test]
fn log_detail_compact_rows_scroll_to_end_and_copy_separate_values(cx: &mut TestAppContext) {
    let _dir = IsolatedDataDir::new("log-detail-compact");
    initialize_components(cx);
    let original = format!(
        "{}\r\n\n{}A\rB\u{2028}C\u{2029}끝",
        "long-name-".repeat(80),
        "다음 줄\n".repeat(40)
    );
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = test_app_root(ActivePanel::Logs);
        root.sidebar_width = 400.0;
        root.app_state.log_entries[0].message = original.clone();
        root.app_state.log_entries.push(LogEntry {
            level: "WARN".into(),
            message: "next".into(),
        });
        root
    });
    cx.simulate_resize(size(
        px(MIN_SUPPORTED_WINDOW_WIDTH),
        px(COMPACT_WINDOW_HEIGHT),
    ));
    refresh(cx);
    let heading = cx.debug_bounds("log-list-heading").unwrap();
    let stats = cx.debug_bounds("log-list-stats").unwrap();
    assert!(stats.left() > heading.right() || stats.top() > heading.bottom());
    let first = cx.debug_bounds("log-row-0").unwrap();
    let second = cx.debug_bounds("log-row-1").unwrap();
    let preview = cx.debug_bounds("log-preview-0").unwrap();
    assert_eq!(first.size.height, px(30.0));
    assert!(preview.top() >= first.top() && preview.bottom() <= first.bottom());
    assert!(first.bottom() <= second.top());
    click_debug_element(cx, "log-row-0");

    let viewport = cx.debug_bounds("log-detail-body").unwrap();
    assert_inside_viewport(cx, "log-detail-body", "log-detail-path");
    let before = cx.debug_bounds("log-detail-message").unwrap();
    assert!(before.bottom() > viewport.bottom());
    wheel_to_end(cx, "log-detail-body", -10000.0);
    let after = cx.debug_bounds("log-detail-message").unwrap();
    assert!(after.bottom() <= viewport.bottom() + px(1.0));
    assert!(after.bottom() > viewport.top());
    assert!(after.left() >= viewport.left() && after.right() <= viewport.right() + px(1.0));

    click_debug_element(cx, "log-copy-message");
    assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), original);
    click_debug_element(cx, "log-copy-path");
    assert_eq!(
        cx.read_from_clipboard().unwrap().text().unwrap(),
        crate::logging::current_log_file().display().to_string()
    );
    click_debug_element(cx, "log-close-detail");
    assert!(view.read_with(cx, |root, _| root.app_state.selected_log_entry.is_none()));
    let card = cx.debug_bounds("log-list-card").unwrap();
    let short_row = cx.debug_bounds("log-row-1").unwrap();
    let right_edge = card.right() - px(16.0);
    assert!(short_row.right() >= right_edge);
    cx.simulate_click(point(right_edge, short_row.center().y), Modifiers::none());
    refresh(cx);
    click_debug_element(cx, "log-copy-message");
    assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), "next");
}

#[gpui::test]
fn log_detail_keeps_clicked_snapshot_after_retention_and_empty_refresh(cx: &mut TestAppContext) {
    let _dir = IsolatedDataDir::new("log-detail-refresh");
    initialize_components(cx);
    let original = "before refresh\r\nlast char: 끝";
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = test_app_root(ActivePanel::Logs);
        root.app_state.log_entries[0].message = original.into();
        root
    });
    refresh(cx);
    click_debug_element(cx, "log-row-0");
    cx.update(|_, cx| {
        view.update(cx, |root, cx| {
            for n in 0..2000 {
                root.push_log("INFO", format!("replacement {n}"));
            }
            assert_eq!(root.app_state.log_entries[0].message, "replacement 0");
            cx.notify();
        })
    });
    refresh(cx);
    click_debug_element(cx, "log-copy-message");
    assert_eq!(cx.read_from_clipboard().unwrap().text().unwrap(), original);
    cx.update(|_, cx| {
        view.update(cx, |root, cx| {
            root.app_state.log_entries.clear();
            cx.notify();
        })
    });
    refresh(cx);
    click_debug_element(cx, "log-close-detail");
    assert!(view.read_with(cx, |root, _| root.app_state.selected_log_entry.is_none()));
    assert_inside_viewport(cx, "content-area", "log-list-empty");
}
