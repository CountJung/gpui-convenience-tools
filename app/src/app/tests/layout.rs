//! 사이드바·스플리터·스크롤 레이아웃 회귀 테스트.

use super::*;
use crate::platform::{SysServiceInfo, SysServiceStartType, SysServiceStatus};

fn drag_divider(
    cx: &mut gpui::VisualTestContext,
    start: gpui::Point<gpui::Pixels>,
    end: gpui::Point<gpui::Pixels>,
) {
    let direction = if end.x >= start.x { px(12.0) } else { px(-12.0) };
    cx.simulate_mouse_move(start, None, Modifiers::none());
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    refresh(cx);
    cx.simulate_mouse_move(
        point(start.x + direction, start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    refresh(cx);
    cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::none());
    refresh(cx);
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    refresh(cx);
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

fn assert_ad_cards_contain_long_target_content(cx: &mut gpui::VisualTestContext) {
    let content = cx
        .debug_bounds("content-area")
        .expect("content area should be rendered");
    let target_card = cx
        .debug_bounds("ad-block-target-card")
        .expect("ad target card should be rendered");
    let target_row = cx
        .debug_bounds("ad-block-target-row-0")
        .expect("ad target row should be rendered");
    let target_class = cx
        .debug_bounds("ad-block-target-class-0")
        .expect("ad target class should be rendered");
    let process_card = cx
        .debug_bounds("ad-block-process-card")
        .expect("ad process card should be rendered");

    let content_right = content.origin.x + content.size.width;
    let target_card_right = target_card.origin.x + target_card.size.width;
    let target_row_right = target_row.origin.x + target_row.size.width;
    let target_class_right = target_class.origin.x + target_class.size.width;
    let process_card_right = process_card.origin.x + process_card.size.width;

    assert!(
        target_card.origin.x >= content.origin.x
            && target_card_right <= content_right,
        "target card should stay inside the content container: content={content:?}, card={target_card:?}"
    );
    assert!(
        target_row.origin.x >= target_card.origin.x
            && target_row_right <= target_card_right,
        "long target content should not push its row outside the card: card={target_card:?}, row={target_row:?}"
    );
    assert!(
        target_class.origin.x >= target_row.origin.x
            && target_class_right <= target_row_right,
        "target class column should stay inside the target row: row={target_row:?}, class={target_class:?}"
    );
    assert!(
        process_card.origin.x >= content.origin.x
            && process_card_right <= content_right,
        "process card should stay inside the content container: content={content:?}, card={process_card:?}"
    );
}

fn assert_service_row_preserves_identity_width(cx: &mut gpui::VisualTestContext) {
    let row = cx
        .debug_bounds("service-row-0")
        .expect("first service row should be rendered");
    let identity = cx
        .debug_bounds("service-row-0-identity")
        .expect("service identity should be rendered");
    let controls = cx
        .debug_bounds("service-row-0-controls")
        .expect("service controls should be rendered");
    let actions = cx
        .debug_bounds("service-row-0-actions")
        .expect("service actions should be rendered");

    assert!(
        identity.size.width >= px(250.0),
        "service identity should retain readable width instead of collapsing to one glyph: \
         row={row:?}, identity={identity:?}"
    );
    assert!(
        identity.size.height >= px(32.0),
        "display name and service name should occupy two readable lines: {identity:?}"
    );
    assert!(
        actions.size.width >= px(171.0),
        "three service action buttons should retain their fixed widths: {actions:?}"
    );

    let row_right = row.origin.x + row.size.width;
    let identity_right = identity.origin.x + identity.size.width;
    let controls_right = controls.origin.x + controls.size.width;
    let actions_right = actions.origin.x + actions.size.width;
    assert!(
        identity.origin.x >= row.origin.x
            && identity_right <= row_right
            && controls.origin.x >= row.origin.x
            && controls_right <= row_right
            && actions_right <= row_right,
        "identity and controls should remain inside the service row: \
         row={row:?}, identity={identity:?}, controls={controls:?}, actions={actions:?}"
    );
    assert!(
        identity.origin.y + identity.size.height <= controls.origin.y,
        "service identity and controls should use separate rows without overlap: \
         identity={identity:?}, controls={controls:?}"
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
fn ad_block_cards_contain_long_content_at_supported_widths(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| test_app_root(ActivePanel::AdBlock));

    cx.update(|_, app| {
        view.update(app, |root, cx| {
            root.app_state.targets = vec![TargetApp {
                process_name: "KakaoTalk.exe.with-a-very-long-process-name".to_string(),
                display_name: "카카오톡 광고 대상 표시 이름이 아주 긴 경우".to_string(),
                enabled: true,
                ad_window_class: "Chrome_WidgetWin_1_With_Long_Class_Name".to_string(),
            }];
            root.running_processes = vec![
                "KakaoTalk.exe.with-a-very-long-process-name".to_string(),
                "another-process-with-a-long-name.exe".to_string(),
            ];
            cx.notify();
        });
    });

    for width in [MIN_SUPPORTED_WINDOW_WIDTH, 994.0, DEFAULT_WINDOW_WIDTH, 1280.0] {
        cx.simulate_resize(size(px(width), px(DEFAULT_WINDOW_HEIGHT)));
        refresh(cx);
        assert_ad_cards_contain_long_target_content(cx);
    }
}

#[gpui::test]
fn split_panels_keep_settings_pane_usable_at_default_width(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| test_app_root(ActivePanel::AdBlock));

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    refresh(cx);
    assert_feature_split_is_usable(cx, "ad-block-split-left-pane", "ad-block-split-right-pane");

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
fn service_rows_keep_names_readable_at_supported_window_widths(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| test_app_root(ActivePanel::Services));

    cx.update(|_, app| {
        view.update(app, |root, cx| {
            root.sys_services = vec![SysServiceInfo {
                name: "LongBackgroundServiceIdentifier".to_string(),
                display_name: "긴 Windows 백그라운드 서비스 표시 이름".to_string(),
                status: SysServiceStatus::Running,
                start_type: SysServiceStartType::Automatic,
            }];
            cx.notify();
        });
    });

    for width in [MIN_SUPPORTED_WINDOW_WIDTH, DEFAULT_WINDOW_WIDTH, 1280.0] {
        cx.simulate_resize(size(px(width), px(DEFAULT_WINDOW_HEIGHT)));
        refresh(cx);
        assert_service_row_preserves_identity_width(cx);
    }
}

#[gpui::test]
fn sidebar_divider_drag_resizes_navigation_and_content(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (_view, cx) = cx.add_window_view(|_, _| test_app_root(ActivePanel::Dashboard));

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    refresh(cx);

    let before_sidebar = cx
        .debug_bounds("sidebar-pane")
        .expect("sidebar pane should be rendered");
    let before_content = cx
        .debug_bounds("content-pane")
        .expect("content pane should be rendered");
    let start = point(
        before_content.origin.x - px(2.0),
        before_sidebar.origin.y + before_sidebar.size.height / 2.0,
    );
    let end = point(start.x + px(400.0), start.y);
    drag_divider(cx, start, end);

    let after_sidebar = cx
        .debug_bounds("sidebar-pane")
        .expect("resized sidebar pane should be rendered");
    let after_content = cx
        .debug_bounds("content-pane")
        .expect("resized content pane should be rendered");
    assert!(
        after_sidebar.size.width >= px(359.0),
        "dragging past the maximum should widen the sidebar to its upper bound: \
         before={before_sidebar:?}, \
         after={after_sidebar:?}"
    );
    assert!(
        after_sidebar.size.width <= px(360.0),
        "sidebar should respect its maximum width: {after_sidebar:?}"
    );
    assert!(
        after_content.origin.x >= before_content.origin.x + px(150.0),
        "content should move with the resized divider: before={before_content:?}, \
         after={after_content:?}"
    );

    let shrink_start = point(
        after_content.origin.x - px(2.0),
        after_sidebar.origin.y + after_sidebar.size.height / 2.0,
    );
    drag_divider(
        cx,
        shrink_start,
        point(shrink_start.x - px(400.0), shrink_start.y),
    );
    let minimum_sidebar = cx
        .debug_bounds("sidebar-pane")
        .expect("minimum sidebar pane should be rendered");
    assert!(
        minimum_sidebar.size.width >= px(200.0) && minimum_sidebar.size.width <= px(201.0),
        "dragging past the minimum should clamp the sidebar at 200px: {minimum_sidebar:?}"
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
    assert!(
        viewport.origin.y + viewport.size.height <= px(COMPACT_WINDOW_HEIGHT),
        "sidebar viewport should stay below the title bar and inside the window: {viewport:?}"
    );
    let max_scroll_height =
        cx.update(|_, app| view.read(app).sidebar_scroll_handle.max_offset().height);

    // Windows는 Win32 전용 패널까지 7개 항목이라 이 크기에서 반드시 넘친다.
    // 비Windows는 파일 동기화·로그·설정만 남아 넘치지 않을 수 있으므로, 이 단언은
    // 항목이 줄었다는 이유로 스크롤 회귀를 놓치지 않도록 Windows에서만 강제한다.
    #[cfg(target_os = "windows")]
    assert!(
        max_scroll_height > px(0.0),
        "compact window should produce sidebar overflow"
    );

    // 넘치는 구성에서만 휠로 끝까지 내려간다. 넘치지 않으면 마지막 항목은 이미 보인다.
    if max_scroll_height > px(0.0) {
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
    }

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
