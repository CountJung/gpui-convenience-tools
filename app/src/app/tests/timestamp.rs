use super::*;
use gpui::AppContext;

#[gpui::test]
fn timestamp_buttons_convert_both_directions_and_report_invalid_input(cx: &mut TestAppContext) {
    initialize_components(cx);
    let mut root_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let entity = cx.new(|_| compact_root(ActivePanel::Timestamp));
        root_entity = Some(entity.clone());
        gpui_component::Root::new(entity, window, cx)
    });
    let root = root_entity.unwrap();
    resize_compact(cx);
    let reach = |cx: &mut gpui::VisualTestContext, item: &'static str| {
        reach_compact_control(cx, "timestamp-page", item);
    };
    reach(cx, "timestamp-epoch");
    click_debug_element(cx, "timestamp-epoch");
    // The reverse action must replace this noncanonical spelling, not pass on stale input.
    cx.simulate_keystrokes("ctrl-a 0 1");
    refresh(cx);
    reach(cx, "timestamp-to-date");
    click_debug_element(cx, "timestamp-to-date");
    reach(cx, "timestamp-date");
    cx.update(|_, cx| {
        assert_eq!(
            root.read(cx)
                .timestamp
                .date
                .as_ref()
                .unwrap()
                .read(cx)
                .value(),
            "1970. 1. 1. 오전 9:00:01"
        );
        assert!(!root.read(cx).timestamp.failed);
    });
    reach(cx, "timestamp-to-epoch");
    click_debug_element(cx, "timestamp-to-epoch");
    reach(cx, "timestamp-epoch");
    cx.update(|_, cx| {
        let input = root.read(cx).timestamp.epoch.clone().unwrap();
        assert_eq!(input.read(cx).value(), "1");
    });
    click_debug_element(cx, "timestamp-epoch");
    cx.simulate_keystrokes("ctrl-a b a d");
    refresh(cx);
    reach(cx, "timestamp-to-date");
    click_debug_element(cx, "timestamp-to-date");
    reach(cx, "timestamp-status");
    cx.update(|_, cx| {
        assert!(root.read(cx).timestamp.failed);
        assert!(root.read(cx).timestamp.status.contains("십진수"));
    });
}

#[gpui::test]
fn timestamp_zone_autosaves_restores_and_inputs_fit_supported_sizes(cx: &mut TestAppContext) {
    let _dir = crate::config::tests::IsolatedDataDir::new("timestamp-ui");
    initialize_components(cx);
    let mut root_entity = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let mut root = test_app_root(ActivePanel::Timestamp);
        root.sync.external_side_effects_enabled = true;
        let entity = cx.new(|_| root);
        root_entity = Some(entity.clone());
        gpui_component::Root::new(entity, window, cx)
    });
    let root = root_entity.unwrap();
    refresh(cx);
    click_debug_element(cx, "timestamp-zone");
    cx.update(|_, cx| assert!(root.read(cx).timestamp.config.utc));
    assert!(crate::config::load_config().unwrap().unwrap().timestamp.utc);
    for (width, height) in [(1000.0, 700.0), (920.0, 480.0), (1600.0, 900.0)] {
        cx.simulate_resize(size(px(width), px(height)));
        refresh(cx);
        let page = cx.debug_bounds("timestamp-page").unwrap();
        let input = cx.debug_bounds("timestamp-epoch").unwrap();
        assert!(input.size.width > px(400.0));
        assert!(input.origin.x >= page.origin.x);
        assert!(input.right() <= page.right());
    }
    let config = crate::config::load_config().unwrap().unwrap();
    cx.update(|_, cx| {
        root.update(cx, |root, _| {
            root.timestamp =
                crate::app::timestamp_ops::TimestampState::from_config(config.timestamp)
        })
    });
    refresh(cx);
    click_debug_element(cx, "timestamp-to-date");
    cx.update(|_, cx| {
        assert_eq!(
            root.read(cx)
                .timestamp
                .date
                .as_ref()
                .unwrap()
                .read(cx)
                .value(),
            "2026. 10. 20. 오전 1:57:56.171"
        )
    });
}
