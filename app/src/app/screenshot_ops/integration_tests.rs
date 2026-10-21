//! 중앙 wiring과 기본 창의 실제 표시 영역 회귀. OS 화면·클립보드·입력에는 접근하지 않는다.
use super::*;
use crate::app::{
    tests::{click_debug_element, initialize_components, refresh, test_app_root},
    ActivePanel,
};
use gpui::{point, px, size, Entity, TestAppContext, VisualTestContext};

fn add_window(cx: &mut TestAppContext) -> (Entity<AppRoot>, &mut VisualTestContext) {
    let mut view = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let inner = cx.new(|_| test_app_root(ActivePanel::Screenshot));
        view = Some(inner.clone());
        gpui_component::Root::new(inner, window, cx)
    });
    (view.unwrap(), cx)
}

#[gpui::test]
fn screenshot_default_window_preview_visible_with_many_monitors(cx: &mut TestAppContext) {
    let _isolation = crate::config::tests::IsolatedDataDir::new("screenshot-default-preview");
    initialize_components(cx);
    let (view, cx) = add_window(cx);
    cx.simulate_resize(size(px(1000.0), px(700.0)));
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.install_screenshot(
                ScreenshotImage::new(640, 360, [17, 42, 93, 255].repeat(640 * 360)).unwrap(),
                window,
                cx,
            );
            this.screenshot.monitors = (0..12)
                .map(|index| platform::Monitor {
                    device_id: format!("synthetic-{index}"),
                    rect: platform::DesktopRect {
                        left: index * 1920,
                        top: 0,
                        width: 1920,
                        height: 1080,
                    },
                    primary: index == 0,
                })
                .collect();
        })
    });
    refresh(cx);
    let page = cx.debug_bounds("screenshot-page").unwrap();
    let preview = cx.debug_bounds("screenshot-preview").unwrap();
    let visible_top = preview.origin.y.max(page.origin.y);
    let visible_bottom =
        (preview.origin.y + preview.size.height).min(page.origin.y + page.size.height);
    assert!(
        visible_bottom - visible_top >= px(260.0),
        "preview={preview:?}, page={page:?}"
    );
    view.read_with(cx, |this, _| {
        let fit = this.screenshot.image_rect().unwrap();
        let painted = this.screenshot.painted_image_bounds.get().unwrap();
        assert_eq!(painted.origin, point(px(fit.left), px(fit.top)));
        assert_eq!(painted.size, size(px(fit.width), px(fit.height)));
        assert!(painted.origin.x >= preview.origin.x);
        assert!(painted.origin.y >= preview.origin.y);
        assert!(painted.origin.x + painted.size.width <= preview.origin.x + preview.size.width);
        assert!(painted.origin.y + painted.size.height <= preview.origin.y + preview.size.height);
        assert!(fit.height >= 240.0);
        assert!(fit.top >= f32::from(page.origin.y));
        assert!(fit.top + fit.height <= f32::from(page.origin.y + page.size.height));
    });
    let save = cx.debug_bounds("screenshot-save").unwrap();
    let auto_save = cx.debug_bounds("screenshot-autosave").unwrap();
    assert!(auto_save.origin.y > save.origin.y);
}

#[gpui::test]
fn screenshot_global_save_and_navigation_preserve_only_preferences(cx: &mut TestAppContext) {
    let _isolation = crate::config::tests::IsolatedDataDir::new("screenshot-global-wiring");
    initialize_components(cx);
    let (view, cx) = add_window(cx);
    cx.simulate_resize(size(px(1000.0), px(1000.0)));
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.install_screenshot(
                ScreenshotImage::new(2, 1, [1, 2, 3, 255].repeat(2)).unwrap(),
                window,
                cx,
            );
            this.screenshot.config.auto_save = true;
            this.screenshot.config.save_folder = "synthetic-output".into();
            this.sync.external_side_effects_enabled = true;
            this.save_current_config().unwrap();
        })
    });
    let config = crate::config::load_config().unwrap().unwrap();
    assert!(config.screenshot.auto_save);
    assert_eq!(config.screenshot.save_folder, "synthetic-output");
    let restored = ScreenshotState::from_config(config.screenshot);
    assert!(restored.image.is_none() && restored.preview.is_none());
    assert!(restored.selection.is_none());
    #[cfg(target_os = "windows")]
    assert_eq!(crate::app::state::NAV_TOOLS.len(), 10);
    cx.update(|_, cx| {
        view.update(cx, |this, _| {
            this.active_panel = ActivePanel::Dashboard;
            this.sync.external_side_effects_enabled = false;
        })
    });
    refresh(cx);
    // GPUI mock wheel만 사용하여 사이드바의 아래쪽 메뉴를 노출한다.
    let nav = cx.debug_bounds("nav-item-스크린샷").unwrap();
    cx.simulate_event(gpui::ScrollWheelEvent {
        position: point(nav.origin.x + px(4.0), px(350.0)),
        delta: gpui::ScrollDelta::Pixels(point(px(0.0), px(-1600.0))),
        ..Default::default()
    });
    refresh(cx);
    #[cfg(target_os = "windows")]
    {
        click_debug_element(cx, "nav-item-스크린샷");
        view.read_with(cx, |this, _| {
            assert_eq!(this.active_panel, ActivePanel::Screenshot)
        });
    }
}
