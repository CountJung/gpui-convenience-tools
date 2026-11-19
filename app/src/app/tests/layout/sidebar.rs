use super::*;
use gpui::AppContext;

#[gpui::test]
fn sidebar_drag_survives_settings_save_and_reopen(cx: &mut TestAppContext) {
    use crate::config::{load_config, tests::IsolatedDataDir, update_config};

    let _fixture = IsolatedDataDir::new("sidebar-save-reopen");
    let saved = || load_config().unwrap().unwrap();
    update_config(|cfg| cfg.sidebar_width = 240.).unwrap();
    initialize_components(cx);
    fn open(
        cx: &mut TestAppContext,
    ) -> (
        gpui::Entity<gpui_component::Root>,
        &mut gpui::VisualTestContext,
    ) {
        let width = load_config().unwrap().unwrap().sidebar_width;
        cx.add_window_view(move |window, cx| {
            let mut root = test_app_root(ActivePanel::Settings);
            root.sidebar_width = width;
            root.sync.external_side_effects_enabled = true;
            root.scan_interval_secs = 37; // Distinguishes a Save click from a no-op.
            let view = cx.new(|_| root);
            gpui_component::Root::new(view, window, cx)
        })
    }
    let (first_root, first) = open(cx);
    let sidebar =
        |cx: &mut gpui::VisualTestContext| cx.debug_bounds("sidebar-pane").unwrap().size.width;
    let initial = sidebar(first);
    let content = first.debug_bounds("content-pane").unwrap();
    let start = point(content.left() - px(2.), content.center().y);
    drag_divider(first, start, point(start.x + px(75.), start.y));
    let adjusted = sidebar(first);
    assert!(
        adjusted > initial + px(50.) && adjusted < px(359.),
        "{initial:?} -> {adjusted:?}"
    );
    assert!((px(saved().sidebar_width) - adjusted).abs() <= px(1.));
    assert_ne!(saved().scan_interval_secs, 37);
    click_debug_element(first, "save-settings");
    assert_eq!(saved().scan_interval_secs, 37);
    assert!(
        (px(saved().sidebar_width) - adjusted).abs() <= px(1.),
        "full save restored {} instead of {adjusted:?}",
        saved().sidebar_width
    );
    first.update(|window, _| window.remove_window());
    drop(first_root);
    let (_, reopened) = open(cx);
    let restored = sidebar(reopened);
    assert!(
        (restored - adjusted).abs() <= px(1.),
        "{adjusted:?} -> {restored:?}"
    );
}
