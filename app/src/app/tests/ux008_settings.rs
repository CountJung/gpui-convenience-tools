use super::*;
use gpui::AppContext;

#[gpui::test]
fn compact_settings_long_paths_and_save_round_trip(cx: &mut TestAppContext) {
    use crate::config::{
        effective_config_path, load_config, tests::IsolatedDataDir, update_config,
    };

    let _fixture = IsolatedDataDir::new(&"settings-archive-2026-".repeat(6));
    update_config(|cfg| {
        cfg.sidebar_width = 360.;
        cfg.scan_interval_secs = 10;
    })
    .expect("seed isolated settings.json");
    let path = effective_config_path();
    assert!(path.is_file() && path.to_string_lossy().len() > 120);
    let saved = load_config().unwrap().unwrap();
    initialize_components(cx);
    let (_, cx) = cx.add_window_view(|window, cx| {
        let mut root = compact_root(ActivePanel::Settings);
        root.sidebar_width = saved.sidebar_width;
        root.scan_interval_secs = 37;
        root.sync.external_side_effects_enabled = true;
        let view = cx.new(|_| root);
        gpui_component::Root::new(view, window, cx)
    });
    resize_compact(cx);
    for item in [
        "settings-config-path",
        "open-settings-file",
        "settings-log-path",
        "theme-dark",
        "save-settings",
    ] {
        reach_compact_control(cx, "content-area", item);
    }
    assert_eq!(load_config().unwrap().unwrap().scan_interval_secs, 10);
    click_debug_element(cx, "save-settings");
    let saved = load_config().unwrap().unwrap();
    assert_eq!(saved.scan_interval_secs, 37);
}
