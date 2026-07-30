//! 테마 전환과 스위치 가시성 회귀 테스트.

use super::*;

fn apply_theme_mode(cx: &mut gpui::VisualTestContext, mode: ThemeMode) {
    cx.update(|window, app| crate::theme::change_theme(mode, Some(window), app));
    refresh(cx);
}

fn apply_bundled_theme(cx: &mut gpui::VisualTestContext, file_name: &str, theme_name: &str) {
    let (_, contents) = BUNDLED_THEMES
        .iter()
        .find(|(candidate, _)| *candidate == file_name)
        .unwrap_or_else(|| panic!("{file_name} should be bundled"));
    let set: ThemeSet = serde_json::from_str(contents)
        .unwrap_or_else(|err| panic!("{file_name} theme should parse: {err}"));
    let config = set
        .themes
        .into_iter()
        .find(|theme| theme.name.as_ref() == theme_name)
        .unwrap_or_else(|| panic!("{theme_name} should exist in {file_name}"));
    let mode = config.mode;

    cx.update(|window, app| {
        Theme::global_mut(app).apply_config(&Rc::new(config));
        crate::theme::change_theme(mode, Some(window), app);
    });
    refresh(cx);
}

fn assert_sync_notify_switch_toggles(
    view: &Entity<AppRoot>,
    cx: &mut gpui::VisualTestContext,
    expected_mode: ThemeMode,
    expected_theme_name: Option<&str>,
) {
    let (mode, theme_name, initial) = cx.update(|_, app| {
        let theme = app.theme();
        (
            theme.mode,
            theme.theme_name().to_string(),
            view.read(app).sync_notify_enabled,
        )
    });
    assert_eq!(mode, expected_mode, "theme mode should be applied");
    if let Some(expected_theme_name) = expected_theme_name {
        assert_eq!(
            theme_name, expected_theme_name,
            "the requested bundled theme should be active"
        );
    }
    assert!(!initial, "each theme scenario should start switched off");

    click_debug_element(cx, "sync-notify-toggle");
    let checked = cx.update(|_, app| view.read(app).sync_notify_enabled);
    assert!(checked, "clicking the rendered switch should turn it on");

    click_debug_element(cx, "sync-notify-toggle");
    let checked = cx.update(|_, app| view.read(app).sync_notify_enabled);
    assert!(
        !checked,
        "clicking the rendered switch again should turn it off"
    );
}

#[gpui::test]
fn rendered_switch_toggles_in_light_dark_and_missing_switch_token_theme(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = test_app_root(ActivePanel::FileSync);
        root.sync_notify_enabled = false;
        root
    });

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    apply_theme_mode(cx, ThemeMode::Light);
    assert_sync_notify_switch_toggles(&view, cx, ThemeMode::Light, None);

    apply_theme_mode(cx, ThemeMode::Dark);
    assert_sync_notify_switch_toggles(&view, cx, ThemeMode::Dark, None);

    // Alduin omits the switch palette tokens that caused the original visibility regression.
    apply_bundled_theme(cx, "alduin.json", "Alduin");
    assert_sync_notify_switch_toggles(&view, cx, ThemeMode::Dark, Some("Alduin"));
}
