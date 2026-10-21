use super::*;
use crate::app::tests::{click_debug_element, initialize_components, refresh, test_app_root};
use gpui::{
    point, px, size, Entity, ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext,
};

fn add_window(cx: &mut TestAppContext, root: AppRoot) -> (Entity<AppRoot>, &mut VisualTestContext) {
    let mut view = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let inner = cx.new(|_| root);
        view = Some(inner.clone());
        gpui_component::Root::new(inner, window, cx)
    });
    (view.unwrap(), cx)
}

fn scroll(view: &Entity<AppRoot>, cx: &mut VisualTestContext, end: bool) {
    view.update(cx, |root, cx| {
        root.launcher
            .scroll
            .set_offset(point(px(0.0), if end { px(-10000.0) } else { px(0.0) }));
        cx.notify();
    });
    refresh(cx);
}

fn draft(view: &Entity<AppRoot>, cx: &mut VisualTestContext, name: &str, path: &str) {
    cx.update(|window, cx| {
        view.update(cx, |root, cx| {
            root.launcher
                .name
                .as_ref()
                .unwrap()
                .update(cx, |s, cx| s.set_value(name.to_owned(), window, cx));
            root.launcher
                .path
                .as_ref()
                .unwrap()
                .update(cx, |s, cx| s.set_value(path.to_owned(), window, cx));
        })
    });
    refresh(cx);
}

#[gpui::test]
fn launcher_gpui_register_search_select_script_cancel_edit_delete(cx: &mut TestAppContext) {
    let _isolated = crate::config::tests::IsolatedDataDir::new("launcher-ui");
    let path = crate::config::effective_config_path().with_file_name("fixture.ps1");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"# harmless fixture, never run by GPUI tests").unwrap();
    initialize_components(cx);
    let (view, cx) = add_window(cx, test_app_root(ActivePanel::QuickLauncher));
    cx.simulate_resize(size(px(1000.0), px(900.0)));
    refresh(cx);
    scroll(&view, cx, true);
    click_debug_element(cx, "launcher-kind-script");
    draft(&view, cx, "Fixture", path.to_str().unwrap());
    // Reject registration without explicit trust. This test AppRoot cannot save or execute.
    click_debug_element(cx, "launcher-save");
    view.read_with(cx, |root, _| {
        assert!(root.launcher.failed);
        assert!(root.launcher.config.entries.is_empty());
    });
    click_debug_element(cx, "launcher-trust");
    click_debug_element(cx, "launcher-save");
    view.read_with(cx, |root, _| {
        assert!(!root.launcher.failed, "{}", root.launcher.status);
        assert_eq!(root.launcher.config.entries.len(), 1);
        assert!(root.launcher.config.entries[0].trusted);
        assert_eq!(root.launcher.selected, Some(0));
    });
    scroll(&view, cx, false);
    let search = view.read_with(cx, |root, _| root.launcher.search.clone().unwrap());
    cx.update(|window, cx| search.update(cx, |s, cx| s.focus(window, cx)));
    cx.simulate_keystrokes("n o m a t c h");
    refresh(cx);
    view.read_with(cx, |root, cx| {
        assert_eq!(
            root.launcher
                .search
                .as_ref()
                .unwrap()
                .read(cx)
                .value()
                .as_str(),
            "nomatch"
        );
        assert_eq!(root.launcher.query, "nomatch");
    });
    // debug_bounds may retain a selector from a previous frame. Check the rendered empty
    // state together with the actual filter result instead of relying on its absence.
    assert!(cx.debug_bounds("launcher-empty").is_some());
    view.read_with(cx, |root, _| {
        assert!(quick_launcher::matching_indices(
            &root.launcher.config.entries,
            &root.launcher.query
        )
        .is_empty());
        assert!(root.launcher.selected.is_none());
    });
    cx.simulate_keystrokes("ctrl-a F i x");
    refresh(cx);
    click_debug_element(cx, "launcher-select-0");
    click_debug_element(cx, "launcher-run");
    view.read_with(cx, |root, _| {
        assert!(root.launcher.pending_script.is_some());
        assert!(!root.launcher.is_busy());
    });
    click_debug_element(cx, "launcher-cancel");
    view.read_with(cx, |root, _| {
        assert!(root.launcher.pending_script.is_none());
        assert!(!root.launcher.is_busy());
        assert!(root.launcher.children.is_empty());
        assert!(root.launcher.status.contains("취소"));
    });
    click_debug_element(cx, "launcher-edit-0");
    scroll(&view, cx, true);
    draft(&view, cx, "Edited Fixture", path.to_str().unwrap());
    click_debug_element(cx, "launcher-trust");
    click_debug_element(cx, "launcher-save");
    view.read_with(cx, |root, _| {
        assert_eq!(root.launcher.config.entries.len(), 1);
        assert_eq!(root.launcher.config.entries[0].name, "Edited Fixture");
    });
    scroll(&view, cx, false);
    click_debug_element(cx, "launcher-delete-0");
    view.read_with(cx, |root, _| {
        assert!(root.launcher.config.entries.is_empty())
    });
    assert!(
        path.exists(),
        "deleting registration must not delete the target"
    );
    assert!(
        !crate::config::effective_config_path().exists(),
        "test side effects must stay disabled"
    );
}

#[gpui::test]
fn launcher_gpui_hotkey_event_only_navigates_and_restores_without_panel_entry(
    cx: &mut TestAppContext,
) {
    initialize_components(cx);
    let mut root = test_app_root(ActivePanel::Dashboard);
    root.launcher = LauncherState::from_config(LauncherConfig {
        enabled: true,
        ..Default::default()
    });
    let (view, cx) = add_window(cx, root);
    view.update(cx, |root, cx| {
        assert!(root.launcher.search.is_none());
        root.tick_launcher(cx);
        assert_eq!(
            root.launcher.applied_hotkey,
            Some((true, LauncherHotkey::CtrlAltSpace))
        );
        assert!(
            root.launcher.hotkey.is_none(),
            "mock GPUI must not register real OS hotkeys"
        );
        root.handle_launcher_hotkey(HotkeyEvent::Activated, cx);
        assert_eq!(root.active_panel, ActivePanel::QuickLauncher);
        assert!(root.launcher.launch_result.is_none());
        assert!(root.launcher.pending_script.is_none());
        root.handle_launcher_hotkey(HotkeyEvent::Failed(1409), cx);
        assert!(root.launcher.hotkey_failed);
        assert!(root.launcher.hotkey_status.contains("1409"));
    });
    refresh(cx);
    assert!(cx.debug_bounds("launcher-page").is_some());
    click_debug_element(cx, "launcher-hotkey-toggle");
    view.read_with(cx, |root, _| {
        assert!(!root.launcher.config.enabled);
        assert!(!root.launcher.hotkey_failed);
        assert_eq!(
            root.launcher.applied_hotkey,
            Some((false, LauncherHotkey::CtrlAltSpace))
        );
    });
    click_debug_element(cx, "launcher-hotkey-combo");
    view.read_with(cx, |root, _| {
        assert_eq!(root.launcher.config.hotkey, LauncherHotkey::CtrlAltF9)
    });
}

#[gpui::test]
fn launcher_gpui_settings_persist_and_transient_inputs_do_not(cx: &mut TestAppContext) {
    let _isolated = crate::config::tests::IsolatedDataDir::new("launcher-settings");
    crate::config::update_config(|cfg| cfg.scan_interval_secs = 71).unwrap();
    initialize_components(cx);
    let (view, cx) = add_window(cx, test_app_root(ActivePanel::QuickLauncher));
    refresh(cx);
    view.update(cx, |root, cx| {
        // Only persistence enabled here; no tick or process execution is invoked.
        root.sync.external_side_effects_enabled = true;
        root.launcher.config.hotkey = LauncherHotkey::CtrlAltF9;
        root.launcher.config.enabled = true;
        root.launcher.query = "not-persisted".into();
        root.persist_launcher_config(cx);
        root.sync.external_side_effects_enabled = false;
    });
    let restored = crate::config::load_config().unwrap().unwrap();
    assert!(restored.launcher.enabled);
    assert_eq!(restored.launcher.hotkey, LauncherHotkey::CtrlAltF9);
    assert_eq!(restored.scan_interval_secs, 71);
    assert!(
        !std::fs::read_to_string(crate::config::effective_config_path())
            .unwrap()
            .contains("not-persisted")
    );
    let state = LauncherState::from_config(restored.launcher);
    assert!(state.query.is_empty());
    assert!(state.search.is_none());
    assert!(state.pending_script.is_none());
    let path = crate::config::effective_config_path();
    let saved = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"{").unwrap();
    view.update(cx, |root, cx| {
        root.sync.external_side_effects_enabled = true;
        root.persist_launcher_config(cx);
        root.sync.external_side_effects_enabled = false;
        assert!(root.launcher.save_error);
    });
    assert_eq!(std::fs::read(&path).unwrap(), b"{");
    std::fs::write(&path, saved).unwrap();
    view.update(cx, |root, cx| {
        root.sync.external_side_effects_enabled = true;
        root.persist_launcher_config(cx);
        root.sync.external_side_effects_enabled = false;
        assert!(!root.launcher.save_error);
    });
    view.update(cx, |root, _| {
        root.sync.external_side_effects_enabled = true;
        root.launcher.config.enabled = false;
        root.launcher.save_error = true;
        root.save_current_config().unwrap();
        root.sync.external_side_effects_enabled = false;
        assert!(!root.launcher.save_error);
    });
    assert!(
        !crate::config::load_config()
            .unwrap()
            .unwrap()
            .launcher
            .enabled
    );
}

#[gpui::test]
fn launcher_gpui_native_path_results_select_cancel_and_fail_without_side_effects(
    cx: &mut TestAppContext,
) {
    let _isolated = crate::config::tests::IsolatedDataDir::new("launcher-path-results");
    initialize_components(cx);
    let (view, cx) = add_window(cx, test_app_root(ActivePanel::QuickLauncher));
    refresh(cx);
    // Synthetic results exercise exactly the handler used by the native prompt task.
    // No native dialog, file access, registration, or process execution occurs.
    for (folder, path, kind) in [
        (false, r"C:\Fixture\app.exe", LauncherKind::Application),
        (false, r"C:\Fixture\safe.PS1", LauncherKind::Script),
        (true, r"C:\Fixture\folder", LauncherKind::Folder),
    ] {
        cx.update(|window, cx| {
            view.update(cx, |root, cx| {
                root.launcher.draft_trusted = true;
                root.launcher.pending_script = Some(LauncherEntry::default());
                root.apply_launcher_path_result(
                    folder,
                    Ok(Some(vec![PathBuf::from(path)])),
                    window,
                    cx,
                );
                assert_eq!(root.launcher.draft_kind, kind);
                assert!(!root.launcher.draft_trusted);
                assert!(root.launcher.pending_script.is_none());
                assert_eq!(
                    root.launcher
                        .path
                        .as_ref()
                        .unwrap()
                        .read(cx)
                        .value()
                        .as_str(),
                    path
                );
            })
        });
        refresh(cx);
    }
    cx.update(|window, cx| {
        view.update(cx, |root, cx| {
            root.launcher.draft_trusted = true;
            root.launcher.pending_script = Some(LauncherEntry::default());
            for cancelled in [Ok(None), Ok(Some(vec![]))] {
                root.apply_launcher_path_result(false, cancelled, window, cx);
                assert!(root.launcher.draft_trusted);
                assert!(root.launcher.pending_script.is_some());
                assert_eq!(root.launcher.draft_kind, LauncherKind::Folder);
                assert_eq!(
                    root.launcher
                        .path
                        .as_ref()
                        .unwrap()
                        .read(cx)
                        .value()
                        .as_str(),
                    r"C:\Fixture\folder"
                );
            }
            root.apply_launcher_path_result(false, Err(()), window, cx);
            assert!(root.launcher.failed);
            assert!(root.launcher.status.contains("절대 경로"));
            assert!(!root.launcher.status.contains("Fixture"));
            assert!(root.launcher.config.entries.is_empty());
            assert!(!root.launcher.is_busy());
            assert!(root.launcher.children.is_empty());
        })
    });
    assert!(!crate::config::effective_config_path().exists());
}

#[gpui::test]
fn launcher_gpui_sidebar_navigation_and_background_tick_without_panel_entry(
    cx: &mut TestAppContext,
) {
    initialize_components(cx);
    let (view, cx) = add_window(cx, test_app_root(ActivePanel::Dashboard));
    cx.simulate_resize(size(px(1000.0), px(900.0)));
    refresh(cx);
    let (tx, rx) = mpsc::channel();
    view.update(cx, |root, cx| {
        root.launcher.config.enabled = true;
        root.launcher.launch_result = Some(rx);
        AppRoot::start_event_refresh_loop(cx);
        assert!(root.launcher.search.is_none());
    });
    cx.run_until_parked();
    tx.send(Err(LauncherError::StartFailed)).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    cx.run_until_parked();
    view.read_with(cx, |root, _| {
        assert_eq!(
            root.launcher.applied_hotkey,
            Some((true, LauncherHotkey::CtrlAltSpace))
        );
        assert!(root.launcher.hotkey.is_none());
        assert!(root.launcher.failed);
        assert!(root.launcher.launch_result.is_none());
        assert!(root.launcher.search.is_none());
        assert_eq!(root.active_panel, ActivePanel::Dashboard);
    });
    // Only Windows exposes this native launcher in the sidebar. The background assertions
    // above remain platform-independent; worker completion must not navigate or execute.
    #[cfg(windows)]
    {
        refresh(cx);
        view.update(cx, |root, cx| {
            root.sidebar_scroll_handle
                .set_offset(point(px(0.0), px(-10000.0)));
            cx.notify();
        });
        refresh(cx);
        click_debug_element(cx, "nav-item-빠른 실행기");
        assert!(cx.debug_bounds("launcher-page").is_some());
        view.read_with(cx, |root, _| {
            assert_eq!(root.active_panel, ActivePanel::QuickLauncher)
        });
    }
}

#[gpui::test]
fn launcher_gpui_page_bounds_scroll_and_trust_invalidation(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = add_window(cx, test_app_root(ActivePanel::QuickLauncher));
    for (width, height) in [(920.0, 480.0), (1000.0, 700.0), (1500.0, 900.0)] {
        cx.simulate_resize(size(px(width), px(height)));
        scroll(&view, cx, false);
        let viewport = cx.debug_bounds("launcher-page").unwrap();
        for selector in [
            "launcher-name",
            "launcher-path",
            "launcher-args",
            "launcher-search",
            "launcher-save",
        ] {
            let bounds = cx.debug_bounds(selector).unwrap();
            assert!(bounds.size.width > px(80.0));
            assert!(bounds.origin.x >= viewport.origin.x);
            assert!(
                bounds.origin.x + bounds.size.width
                    <= viewport.origin.x + viewport.size.width + px(1.0)
            );
        }
        cx.simulate_event(ScrollWheelEvent {
            position: viewport.center(),
            delta: ScrollDelta::Pixels(point(px(0.0), px(-3000.0))),
            ..Default::default()
        });
        refresh(cx);
        let save = cx.debug_bounds("launcher-save").unwrap();
        assert!(save.origin.y >= viewport.origin.y);
        assert!(
            save.origin.y + save.size.height <= viewport.origin.y + viewport.size.height + px(1.0)
        );
        if height == 480.0 {
            view.read_with(cx, |root, _| {
                assert!(root.launcher.scroll.offset().y < px(0.0))
            });
        }
    }
    click_debug_element(cx, "launcher-trust");
    view.read_with(cx, |root, _| assert!(root.launcher.draft_trusted));
    draft(&view, cx, "new-name", "new-path");
    view.read_with(cx, |root, _| assert!(!root.launcher.draft_trusted));
}

#[gpui::test]
fn launcher_gpui_registered_rows_keep_names_and_paths_separate(cx: &mut TestAppContext) {
    initialize_components(cx);
    let mut root = test_app_root(ActivePanel::QuickLauncher);
    root.launcher.config.entries = vec![LauncherEntry {
        name: "합성 등록 · 문서 폴더 · 읽을 수 있는 이름".into(),
        path: format!(r"C:\synthetic\{}\folder", "long-path-".repeat(30)),
        kind: LauncherKind::Folder,
        trusted: true,
        ..Default::default()
    }];
    let (view, cx) = add_window(cx, root);
    for (width, height) in [(920.0, 480.0), (1000.0, 700.0), (1500.0, 900.0)] {
        cx.simulate_resize(size(px(width), px(height)));
        scroll(&view, cx, false);
        let card = cx.debug_bounds("launcher-entry-0").unwrap();
        let name = cx.debug_bounds("launcher-select-0").unwrap();
        let path = cx.debug_bounds("launcher-entry-path-0").unwrap();
        assert!(name.size.width >= px(180.0));
        assert!(path.origin.y >= name.origin.y + name.size.height);
        assert!(path.origin.y + path.size.height <= card.origin.y + card.size.height);
        assert!(
            path.size.height < px(30.0),
            "long paths remain one readable line"
        );
        assert!(path.origin.x + path.size.width <= card.origin.x + card.size.width);
        for selector in ["launcher-edit-0", "launcher-delete-0"] {
            let control = cx.debug_bounds(selector).unwrap();
            assert!(control.origin.y + control.size.height <= path.origin.y);
        }
    }
}
