use super::*;

fn reveal(cx: &mut gpui::VisualTestContext, pane: &'static str, id: &'static str) {
    let viewport = cx.debug_bounds(pane).unwrap();
    let bounds = cx.debug_bounds(id).unwrap();
    if bounds.bottom() > viewport.bottom() || bounds.top() < viewport.top() {
        wheel_to_end(cx, pane, f32::from(viewport.center().y - bounds.center().y));
    }
    assert_inside_viewport(cx, pane, id);
}

#[gpui::test]
fn ad_block_compact_switches_panels_and_keeps_controls_reachable(cx: &mut TestAppContext) {
    initialize_components(cx);
    const PROCESS: &str = "UX004-Synthetic-Very-Long-Process-Name-For-Checking-The-Last-Characters-In-A-Tooltip-001.exe";
    const CLASS: &str =
        "UX004_Synthetic_Long_Window_Class_With_No_Spaces_And_A_Unique_Trailing_Identifier_002";
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = test_app_root(ActivePanel::AdBlock);
        root.sidebar_width = 360.;
        root.app_state.targets = vec![TargetApp {
            process_name: PROCESS.into(),
            display_name: "광고 대상".repeat(20),
            ad_window_class: CLASS.into(),
            enabled: false,
        }];
        root
    });
    cx.simulate_resize(size(px(920.), px(480.)));
    refresh(cx);
    let sidebar = cx.debug_bounds("sidebar-pane").unwrap();
    let divider = point(sidebar.right() - px(2.), sidebar.center().y);
    drag_divider(cx, divider, point(px(358.), divider.y));
    assert!(view.read_with(cx, |root, _| root.ad_block.compact
        && !root.ad_block.settings_open));
    let shell = cx.debug_bounds("content-area").unwrap();
    let list = cx.debug_bounds("ad-block-left").unwrap();
    assert!(list.size.width >= shell.size.width - px(40.));
    reveal(cx, "ad-block-left", "ad-target-toggle-0");
    click_debug_element(cx, "ad-target-toggle-0");
    assert!(view.read_with(cx, |root, _| root.app_state.targets[0].enabled));

    for (label, original) in [
        ("ad-block-target-process-0", PROCESS),
        ("ad-block-target-class-0", CLASS),
    ] {
        reveal(cx, "ad-block-left", label);
        let position = cx.debug_bounds(label).unwrap().center();
        cx.simulate_mouse_move(position, None, Modifiers::none());
        cx.executor()
            .advance_clock(std::time::Duration::from_secs(1));
        refresh(cx);
        let tip = cx.debug_bounds(original).expect("전체 원문을 담은 툴팁");
        assert!(
            tip.left() >= px(0.) && tip.right() <= px(920.) && tip.bottom() <= px(480.),
            "{tip:?}"
        );
    }

    click_debug_element(cx, "ad-settings-toggle");
    assert!(view.read_with(cx, |root, _| root.ad_block.settings_open));
    for (button, amount) in [("interval-cancel", "91"), ("interval-add", "92")] {
        let before = view.read_with(cx, |root, _| {
            (
                root.scan_interval_secs,
                root.interval_picker.presets.clone(),
            )
        });
        cx.update(|window, app| {
            view.update(app, |root, cx| {
                root.interval_picker
                    .amount_input
                    .as_ref()
                    .unwrap()
                    .update(cx, |input, cx| input.set_value(amount, window, cx));
            })
        });
        refresh(cx);
        reveal(cx, "ad-block-right", button);
        click_debug_element(cx, button);
        assert!(view.read_with(cx, |root, app| {
            root.interval_picker
                .amount_input
                .as_ref()
                .unwrap()
                .read(app)
                .value()
                .is_empty()
                && if button == "interval-cancel" {
                    root.scan_interval_secs == before.0 && root.interval_picker.presets == before.1
                } else {
                    root.scan_interval_secs == 92 && root.interval_picker.presets.contains(&92)
                }
        }));
    }
    click_debug_element(cx, "ad-settings-toggle");
    assert!(view.read_with(cx, |root, _| !root.ad_block.settings_open
        && root.app_state.targets[0].enabled));
    cx.simulate_resize(size(px(1280.), px(700.)));
    refresh(cx);
    assert!(!view.read_with(cx, |root, _| root.ad_block.compact));
    assert_ad_split_uses_available_width(cx);
}
