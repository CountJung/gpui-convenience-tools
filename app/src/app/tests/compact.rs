//! UX-008: exercise the actual minimum client size and maximum navigation width.
use super::*;
use gpui::VisualTestContext;

pub(crate) fn compact_root(panel: ActivePanel) -> AppRoot {
    let mut root = test_app_root(panel);
    root.sidebar_width = 360.;
    root
}

pub(crate) fn resize_compact(cx: &mut VisualTestContext) {
    cx.simulate_resize(size(
        px(MIN_SUPPORTED_WINDOW_WIDTH),
        px(COMPACT_WINDOW_HEIGHT),
    ));
    refresh(cx);
    let content = cx.debug_bounds("content-pane").unwrap();
    let start = point(content.left() - px(2.), content.center().y);
    layout::drag_divider(cx, start, point(start.x + px(400.), start.y));
    let sidebar = cx.debug_bounds("sidebar-pane").unwrap();
    assert!(
        (px(359.)..=px(361.)).contains(&sidebar.size.width),
        "sidebar: {sidebar:?}"
    );
}

/// Bring a control fully into view using wheel events, never by changing its scroll handle.
pub(crate) fn reach_compact_control(
    cx: &mut VisualTestContext,
    viewport: &'static str,
    item: &'static str,
) {
    for _ in 0..36 {
        let page = cx
            .debug_bounds(viewport)
            .unwrap_or_else(|| panic!("no {viewport}"));
        let target = cx.debug_bounds(item).unwrap_or_else(|| panic!("no {item}"));
        assert!(
            target.size.width > px(0.)
                && target.size.height > px(0.)
                && target.left() >= page.left() - px(1.)
                && target.right() <= page.right() + px(1.),
            "{item} horizontally outside {viewport}: {target:?} vs {page:?}"
        );
        let delta = if target.top() < page.top() - px(1.) {
            page.top() - target.top() + px(1.)
        } else if target.bottom() > page.bottom() + px(1.) {
            page.bottom() - target.bottom() - px(1.)
        } else {
            return;
        };
        cx.simulate_event(ScrollWheelEvent {
            position: page.center(),
            delta: ScrollDelta::Pixels(point(px(0.), delta)),
            modifiers: Modifiers::none(),
            ..Default::default()
        });
        refresh(cx);
    }
    panic!(
        "wheel could not bring {item} into {viewport}: {:?} vs {:?}",
        cx.debug_bounds(item),
        cx.debug_bounds(viewport)
    );
}

#[gpui::test]
fn compact_dashboard_tiles_activity_and_last_menu(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = compact_root(ActivePanel::Dashboard);
        root.app_state.blocked_count = 123_456;
        for i in 0..6 {
            root.app_state.log_entries.push(LogEntry {
                level: "INFO".into(),
                message: format!("작업 {i}: {}", "합성 활동 결과 ".repeat(12)),
            });
        }
        root
    });
    resize_compact(cx);
    let mut previous: Option<gpui::Bounds<gpui::Pixels>> = None;
    for (tile, label, value) in [
        #[cfg(windows)]
        ("dashboard-누적 차단", "누적 차단", "123456"),
        #[cfg(windows)]
        ("dashboard-활성 타겟", "활성 타겟", "1 / 1"),
        ("dashboard-자동 동기화", "자동 동기화", "0 / 0"),
        ("dashboard-동기화 실패", "동기화 실패", "0"),
    ] {
        reach_compact_control(cx, "content-area", tile);
        let bounds = cx.debug_bounds(tile).unwrap();
        let required = cx.update(|window, _| {
            [label, value]
                .map(|text| {
                    let style = window.text_style();
                    let run = gpui::TextRun {
                        len: text.len(),
                        font: style.font(),
                        color: style.color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    window
                        .text_system()
                        .shape_line(text.into(), window.rem_size(), &[run], None)
                        .width
                })
                .into_iter()
                .max()
                .unwrap()
                + window.rem_size() * 1.5
                + px(2.)
        });
        assert!(
            bounds.size.width + px(1.) >= required,
            "{tile}: {bounds:?} < {required:?}"
        );
        if let Some(prev) = previous {
            assert!(bounds.left() >= prev.right() || bounds.top() >= prev.bottom());
        }
        previous = Some(bounds);
    }
    reach_compact_control(cx, "content-area", "dashboard-last-activity");
    reach_compact_control(cx, "sidebar-scroll", "nav-item-설정");
    click_debug_element(cx, "nav-item-설정");
    assert_eq!(
        view.read_with(cx, |root, _| root.active_panel),
        ActivePanel::Settings
    );
    reach_compact_control(cx, "sidebar-scroll", "nav-item-설정");
}
