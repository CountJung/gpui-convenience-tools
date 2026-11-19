use super::*;
use crate::app::{
    tests::{
        click_debug_element, compact_root, initialize_components, reach_compact_control, refresh,
        resize_compact, test_app_root,
    },
    ActivePanel,
};
use gpui::{
    point, px, size, ClipboardEntry, Entity, Modifiers, MouseButton, TestAppContext,
    VisualTestContext,
};

fn add_window(cx: &mut TestAppContext) -> (Entity<AppRoot>, &mut VisualTestContext) {
    add_root(cx, test_app_root(ActivePanel::Screenshot))
}

fn add_root(cx: &mut TestAppContext, root: AppRoot) -> (Entity<AppRoot>, &mut VisualTestContext) {
    let mut view = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let inner = cx.new(|_| root);
        view = Some(inner.clone());
        gpui_component::Root::new(inner, window, cx)
    });
    (view.unwrap(), cx)
}

fn image() -> ScreenshotImage {
    ScreenshotImage::new(400, 200, [17, 42, 93, 255].repeat(400 * 200)).unwrap()
}

fn seed(view: &Entity<AppRoot>, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        view.update(cx, |this, cx| this.install_screenshot(image(), window, cx))
    });
    refresh(cx);
}

fn seed_monitors(view: &Entity<AppRoot>, cx: &mut VisualTestContext, count: usize) {
    let monitors = (0..count)
        .map(|i| platform::Monitor {
            device_id: format!("synthetic-{i}"),
            rect: platform::DesktopRect {
                left: i as i32 * 2560,
                top: 0,
                width: 2560,
                height: if i == 2 { 1600 } else { 1440 },
            },
            primary: i == 0,
        })
        .collect();
    view.update(cx, |root, cx| root.apply_screenshot_monitors(monitors, cx));
    refresh(cx);
}

#[gpui::test]
fn screenshot_compact_three_monitor_buttons_clear_capture_actions(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = add_root(cx, compact_root(ActivePanel::Screenshot));
    resize_compact(cx);
    for count in [3, 4] {
        seed_monitors(&view, cx, count);
        assert_initial_monitor_rows(cx, count);
    }
    seed_monitors(&view, cx, 8);
    let page = "screenshot-page";
    let last = "screenshot-monitor-7";
    let strip = cx.debug_bounds("screenshot-targets").unwrap();
    let button = cx.debug_bounds(last).unwrap();
    assert!(
        strip.size.height > px(80.) && button.bottom() <= strip.bottom() + px(1.),
        "compact strip must expand through the last row: {button:?} vs {strip:?}"
    );
    // Return from the settings below the preview. The last monitor must require a page wheel.
    reach_compact_control(cx, page, "screenshot-folder-input");
    let before = view.read_with(cx, |root, _| root.screenshot.scroll.offset().y);
    reach_compact_control(cx, page, last);
    view.read_with(cx, |root, _| {
        assert!(root.screenshot.scroll.offset().y > before);
        assert_eq!(root.screenshot.config.target, CaptureTarget::AllDisplays);
        assert!(root.screenshot.config.monitor_binding.is_none());
        assert!(!root.screenshot.failed, "{}", root.screenshot.status);
    });
    click_debug_element(cx, last);
    view.read_with(cx, |root, _| {
        assert!(!root.screenshot.failed, "{}", root.screenshot.status);
        assert_eq!(root.screenshot.config.target, CaptureTarget::Monitor(7));
        let binding = root.screenshot.config.monitor_binding.as_ref().unwrap();
        assert_eq!(binding.device_id, "synthetic-7");
        assert_eq!(
            binding.topology,
            platform::topology(&root.screenshot.monitors)
        );
    });
    reach_compact_control(cx, page, "screenshot-capture");
    click_debug_element(cx, "screenshot-capture");
    view.read_with(cx, |root, _| {
        assert_eq!(
            root.screenshot.status,
            ScreenshotError::CaptureFailed.to_string()
        );
    });
    seed(&view, cx);
    reach_compact_control(cx, page, "screenshot-preview");
}

fn assert_initial_monitor_rows(cx: &mut VisualTestContext, count: usize) {
    let targets = cx.debug_bounds("screenshot-targets").unwrap();
    let actions = cx.debug_bounds("screenshot-capture-actions").unwrap();
    let ids = [
        "screenshot-target-all",
        "screenshot-monitor-0",
        "screenshot-monitor-1",
        "screenshot-monitor-2",
        "screenshot-monitor-3",
    ];
    let buttons: Vec<_> = ids[..=count]
        .iter()
        .map(|&id| (id, cx.debug_bounds(id).unwrap()))
        .collect();
    assert!(
        buttons
            .iter()
            .any(|(_, b)| b.top() >= buttons[0].1.bottom()),
        "fixture must exercise wrapping"
    );
    for (id, button) in buttons {
        let clip_gap = targets.bottom() - button.bottom();
        let action_gap = actions.top() - button.bottom();
        assert!(
            button.size.width > px(0.)
                && button.size.height > px(0.)
                && button.left() >= targets.left()
                && button.right() <= targets.right()
                && button.top() >= targets.top()
                && clip_gap >= px(-1.) // Layout bounds and painted edges can round differently.
                && action_gap >= px(0.),
            "{id}: clip_gap={clip_gap:?}, action_gap={action_gap:?}; button={button:?}, targets={targets:?}, actions={actions:?}"
        );
    }
}

#[gpui::test]
fn screenshot_gpui_monitor_changes_latch_block_capture_and_autosave_until_reselected(
    cx: &mut TestAppContext,
) {
    let _isolation = crate::config::tests::IsolatedDataDir::new("screenshot-ui-topology");
    initialize_components(cx);
    let (view, cx) = add_window(cx);
    cx.simulate_resize(size(px(1000.0), px(1000.0)));
    seed(&view, cx);
    let original = platform::tests::fixture_monitors();
    cx.update(|_, cx| {
        view.update(cx, |this, cx| {
            this.apply_screenshot_monitors(original.clone(), cx);
            this.screenshot.config.auto_save = true;
            this.screenshot.config.save_folder =
                crate::config::data_dir().to_string_lossy().into_owned();
            this.set_screenshot_target(CaptureTarget::Monitor(0), cx);
            assert!(this.screenshot_capture_request().is_ok());
        })
    });
    let before = view.read_with(cx, |this, _| this.screenshot.image.clone().unwrap());
    let mut switched = original.clone();
    switched[0].primary = false;
    switched[1].primary = true;
    switched.reverse();
    cx.update(|_, cx| {
        view.update(cx, |this, cx| {
            this.apply_screenshot_monitors(switched.clone(), cx);
            assert!(this.screenshot.config.monitor_binding.is_none());
            // 배치가 다시 원래대로 돌아와도 재선택 전에는 자동 복구하지 않는다.
            this.apply_screenshot_monitors(original.clone(), cx);
        })
    });
    click_debug_element(cx, "screenshot-capture");
    cx.run_until_parked();
    view.read_with(cx, |this, _| {
        assert!(this.screenshot.failed);
        assert_eq!(
            this.screenshot.status,
            ScreenshotError::MonitorChanged.to_string()
        );
        assert!(Arc::ptr_eq(
            this.screenshot.image.as_ref().unwrap(),
            &before
        ));
        assert!(!this.screenshot.busy);
    });
    assert_eq!(
        std::fs::read_dir(crate::config::data_dir())
            .unwrap()
            .count(),
        0
    );
    assert!(cx.read_from_clipboard().is_none());
    cx.update(|_, cx| {
        view.update(cx, |this, cx| {
            this.set_screenshot_target(CaptureTarget::Monitor(0), cx);
            assert!(this.screenshot_capture_request().is_ok());
            let stale_topology = platform::topology(&original);
            this.apply_screenshot_monitors(switched.clone(), cx);
            this.select_screenshot_monitor("fixture-A", &stale_topology, cx);
            assert!(this.screenshot_capture_request().is_err());
            this.set_screenshot_target(CaptureTarget::Monitor(1), cx);
            assert_eq!(
                this.screenshot_capture_request()
                    .unwrap()
                    .device_id
                    .as_deref(),
                Some("fixture-A")
            );
            this.apply_screenshot_monitors(vec![switched[0].clone()], cx);
            assert!(this.screenshot_capture_request().is_err());
        })
    });
}

#[gpui::test]
fn screenshot_selection_snapshot_defers_crop_and_busy_reset_is_ignored(cx: &mut TestAppContext) {
    let _isolation = crate::config::tests::IsolatedDataDir::new("screenshot-ui-snapshot");
    initialize_components(cx);
    let (view, cx) = add_window(cx);
    seed(&view, cx);
    cx.update(|_, cx| {
        view.update(cx, |this, cx| {
            this.screenshot.selection = Some(PixelRect {
                x: 1,
                y: 1,
                width: 2,
                height: 2,
            });
            let selected = this.selected_screenshot().unwrap();
            assert!(Arc::ptr_eq(
                &selected.image,
                this.screenshot.image.as_ref().unwrap()
            ));
            this.screenshot.busy = true;
            this.reset_screenshot_selection(cx);
            assert_eq!(this.screenshot.selection, selected.rect);
            let image = selected.materialize().unwrap();
            assert_eq!((image.width(), image.height()), (2, 2));
            this.screenshot.busy = false;
        })
    });
}

#[gpui::test]
fn screenshot_gpui_drag_crop_copy_and_clear_use_only_mock_clipboard(cx: &mut TestAppContext) {
    let _isolation = crate::config::tests::IsolatedDataDir::new("screenshot-ui-drag");
    initialize_components(cx);
    let (view, cx) = add_root(cx, compact_root(ActivePanel::Screenshot));
    resize_compact(cx);
    seed(&view, cx);
    let page = "screenshot-page";
    reach_compact_control(cx, page, "screenshot-preview");
    let fit = view.read_with(cx, |this, _| this.screenshot.image_rect().unwrap());
    let start = point(
        px(fit.left + fit.width * 0.25),
        px(fit.top + fit.height * 0.25),
    );
    let end = point(
        px(fit.left + fit.width * 0.75),
        px(fit.top + fit.height * 0.75),
    );
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::none());
    refresh(cx);
    let selection = view.read_with(cx, |this, _| this.screenshot.selection.unwrap());
    // 논리 px의 float 반올림은 양 끝에 1px까지 보수적으로 포함한다.
    assert!((99..=100).contains(&selection.x));
    assert!((49..=50).contains(&selection.y));
    assert!((200..=202).contains(&selection.width));
    assert!((100..=102).contains(&selection.height));
    reach_compact_control(cx, page, "screenshot-copy");
    click_debug_element(cx, "screenshot-copy");
    cx.run_until_parked();
    let copied = cx.read_from_clipboard().unwrap();
    let ClipboardEntry::Image(copied) = &copied.entries()[0] else {
        panic!("expected mock PNG image");
    };
    let decoded = image::load_from_memory(copied.bytes()).unwrap();
    assert_eq!(
        (decoded.width(), decoded.height()),
        (selection.width, selection.height)
    );
    view.read_with(cx, |this, _| assert!(!this.screenshot.failed));
    reach_compact_control(cx, page, "screenshot-crop");
    click_debug_element(cx, "screenshot-crop");
    cx.run_until_parked();
    view.read_with(cx, |this, _| {
        let image = this.screenshot.image.as_ref().unwrap();
        assert_eq!(
            (image.width(), image.height()),
            (selection.width, selection.height)
        );
        assert!(this.screenshot.selection.is_none());
    });
    let dir = crate::config::data_dir();
    let target = dir.join("compact-crop.png");
    view.update(cx, |root, _| {
        root.screenshot.config.save_folder = dir.to_string_lossy().into_owned();
        root.sync.external_side_effects_enabled = true;
    });
    reach_compact_control(cx, page, "screenshot-save");
    click_debug_element(cx, "screenshot-save");
    assert!(cx.did_prompt_for_new_path());
    cx.simulate_new_path_selection(|_| Some(target.clone()));
    cx.run_until_parked();
    view.update(cx, |root, _| {
        root.sync.external_side_effects_enabled = false;
        assert!(!root.screenshot.busy && !root.screenshot.failed);
    });
    assert_eq!(
        image::image_dimensions(target).unwrap(),
        (selection.width, selection.height)
    );
    reach_compact_control(cx, page, "screenshot-clear");
    click_debug_element(cx, "screenshot-clear");
    view.read_with(cx, |this, _| {
        assert!(this.screenshot.image.is_none());
        assert!(this.screenshot.preview.is_none());
    });
    assert!(matches!(
        &cx.read_from_clipboard().unwrap().entries()[0],
        ClipboardEntry::Image(_)
    ));
    reach_compact_control(cx, page, "screenshot-copy");
    click_debug_element(cx, "screenshot-copy");
    view.read_with(cx, |this, _| {
        assert_eq!(this.screenshot.status, ScreenshotError::NoImage.to_string())
    });
    // 테스트 빌드에서는 버튼을 눌러도 실제 데스크톱 캡처가 절대 실행되지 않는다.
    reach_compact_control(cx, page, "screenshot-capture");
    click_debug_element(cx, "screenshot-capture");
    view.read_with(cx, |this, _| {
        assert!(this.screenshot.failed);
        assert!(this.screenshot.image.is_none());
    });
}

#[gpui::test]
fn screenshot_gpui_settings_restore_without_image_history(cx: &mut TestAppContext) {
    let _isolation = crate::config::tests::IsolatedDataDir::new("screenshot-ui-settings");
    crate::config::update_config(|cfg| cfg.scan_interval_secs = 77).unwrap();
    initialize_components(cx);
    let (view, cx) = add_window(cx);
    cx.simulate_resize(size(px(1000.0), px(1000.0)));
    refresh(cx);
    cx.update(|_, cx| view.update(cx, |this, _| this.sync.external_side_effects_enabled = true));
    click_debug_element(cx, "screenshot-autosave");
    click_debug_element(cx, "screenshot-target-all");
    let folder = view.read_with(cx, |this, _| this.screenshot.folder_input.clone().unwrap());
    let path = crate::config::data_dir().to_string_lossy().into_owned();
    cx.update(|window, cx| {
        folder.update(cx, |input, cx| input.set_value(path.clone(), window, cx))
    });
    refresh(cx);
    let config = crate::config::load_config().unwrap().unwrap();
    assert!(config.screenshot.auto_save);
    assert_eq!(config.screenshot.save_folder, path);
    assert_eq!(config.scan_interval_secs, 77);
    let state = ScreenshotState::from_config(config.screenshot);
    assert!(state.image.is_none());
    assert!(state.selection.is_none());
    assert!(state.config.auto_save);
    assert_eq!(
        serde_json::to_value(state.config)
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        4
    );
}

#[gpui::test]
fn screenshot_gpui_png_dialog_cancel_save_unique_and_failure_stay_isolated(
    cx: &mut TestAppContext,
) {
    let _isolation = crate::config::tests::IsolatedDataDir::new("screenshot-ui-export");
    let directory = crate::config::data_dir();
    initialize_components(cx);
    let (view, cx) = add_window(cx);
    cx.simulate_resize(size(px(1000.0), px(1000.0)));
    seed(&view, cx);
    cx.update(|_, cx| {
        view.update(cx, |this, _| {
            this.sync.external_side_effects_enabled = true;
            this.screenshot.config.save_folder = directory.to_string_lossy().into_owned();
            this.screenshot.selection = Some(PixelRect {
                x: 10,
                y: 10,
                width: 12,
                height: 8,
            });
        })
    });
    click_debug_element(cx, "screenshot-save");
    assert!(cx.did_prompt_for_new_path());
    cx.simulate_new_path_selection(|_| None);
    cx.run_until_parked();
    view.read_with(cx, |this, _| {
        assert!(!this.screenshot.busy);
        assert!(this.screenshot.status.contains("취소"));
        assert!(this.screenshot.image.is_some());
    });
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 0);
    let target = directory.join("Screenshot.png");
    std::fs::write(&target, b"existing-fixture").unwrap();
    click_debug_element(cx, "screenshot-save");
    cx.simulate_new_path_selection(|_| Some(target.clone()));
    cx.run_until_parked();
    view.read_with(cx, |this, _| {
        assert!(!this.screenshot.busy);
        assert!(!this.screenshot.failed);
    });
    assert_eq!(std::fs::read(&target).unwrap(), b"existing-fixture");
    let output = image::open(directory.join("Screenshot (1).png")).unwrap();
    assert_eq!((output.width(), output.height()), (12, 8));
    cx.update(|_, cx| {
        view.update(cx, |this, _| {
            this.screenshot.config.save_folder =
                directory.join("missing").to_string_lossy().into_owned();
        })
    });
    click_debug_element(cx, "screenshot-save-default");
    cx.run_until_parked();
    view.read_with(cx, |this, _| {
        assert!(this.screenshot.failed);
        assert!(this.screenshot.image.is_some());
        assert!(!this
            .screenshot
            .status
            .contains(&directory.to_string_lossy().to_string()));
    });
    assert!(!directory.join("missing").exists());
}

#[gpui::test]
fn screenshot_gpui_empty_drag_does_not_silently_export_whole_image(cx: &mut TestAppContext) {
    let _isolation = crate::config::tests::IsolatedDataDir::new("screenshot-ui-empty");
    initialize_components(cx);
    let (view, cx) = add_window(cx);
    cx.simulate_resize(size(px(1000.0), px(1000.0)));
    seed(&view, cx);
    let fit = view.read_with(cx, |this, _| this.screenshot.image_rect().unwrap());
    let start = point(px(fit.left + 15.0), px(fit.top + 15.0));
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(start, MouseButton::Left, Modifiers::none());
    refresh(cx);
    click_debug_element(cx, "screenshot-copy");
    cx.run_until_parked();
    assert!(cx.read_from_clipboard().is_none());
    view.read_with(cx, |this, _| {
        assert_eq!(
            this.screenshot.status,
            ScreenshotError::EmptySelection.to_string()
        )
    });
    click_debug_element(cx, "screenshot-reset-selection");
    view.read_with(cx, |this, _| assert!(this.selected_screenshot().is_ok()));
    // mouse_up_out도 실제 표시 이미지 좌표로 클램프한다.
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(
        point(px(-500.0), px(-500.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    refresh(cx);
    view.read_with(cx, |this, _| {
        let selection = this.screenshot.selection.unwrap();
        assert_eq!((selection.x, selection.y), (0, 0));
        assert!(selection.width <= 400 && selection.height <= 200);
    });
}

#[gpui::test]
fn screenshot_gpui_preview_scales_and_minimum_window_scroll_reaches_exports(
    cx: &mut TestAppContext,
) {
    let _isolation = crate::config::tests::IsolatedDataDir::new("screenshot-ui-scroll");
    initialize_components(cx);
    let (view, cx) = add_window(cx);
    for (width, height) in [(1000.0, 700.0), (1600.0, 1100.0), (920.0, 480.0)] {
        cx.simulate_resize(size(px(width), px(height)));
        seed(&view, cx);
        let page = cx.debug_bounds("screenshot-page").unwrap();
        let preview = cx.debug_bounds("screenshot-preview").unwrap();
        assert!(preview.size.width > px(300.0));
        assert!(preview.size.width <= page.size.width);
        assert!(preview.size.height >= px(220.0));
        if height == 700.0 {
            seed_monitors(&view, cx, 12);
            let targets = cx.debug_bounds("screenshot-targets").unwrap();
            let last = cx.debug_bounds("screenshot-monitor-11").unwrap();
            assert!(targets.size.height <= px(80.0) && last.bottom() > targets.bottom());
            let preview = cx.debug_bounds("screenshot-preview").unwrap();
            assert!(
                preview.bottom().min(page.bottom()) - preview.top().max(page.top()) >= px(260.0),
                "preview={preview:?}, page={page:?}"
            );
            reach_compact_control(cx, "screenshot-targets", "screenshot-monitor-11");
            click_debug_element(cx, "screenshot-monitor-11");
            view.read_with(cx, |root, _| {
                assert_eq!(root.screenshot.config.target, CaptureTarget::Monitor(11));
            });
        }
        if height == 480.0 {
            let before = view.read_with(cx, |this, _| this.screenshot.scroll.offset());
            cx.simulate_event(gpui::ScrollWheelEvent {
                position: point(
                    page.origin.x + page.size.width / 2.0,
                    page.origin.y + page.size.height / 2.0,
                ),
                delta: gpui::ScrollDelta::Pixels(point(px(0.0), px(-2200.0))),
                modifiers: Modifiers::none(),
                ..Default::default()
            });
            refresh(cx);
            let after = view.read_with(cx, |this, _| this.screenshot.scroll.offset());
            assert!(after.y < before.y);
            let footer = cx.debug_bounds("screenshot-footer").unwrap();
            assert!(footer.origin.y < page.origin.y + page.size.height);
            assert!(
                footer.origin.y + footer.size.height <= page.origin.y + page.size.height + px(1.0)
            );
        }
    }
}
