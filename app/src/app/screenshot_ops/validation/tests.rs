use super::*;
use crate::app::{
    tests::{click_debug_element, initialize_components, refresh, test_app_root},
    ActivePanel,
};
use gpui::{px, size, Entity, TestAppContext, VisualTestContext};
use std::fs;

fn synthetic() -> ScreenshotImage {
    ScreenshotImage::new(
        4,
        2,
        vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 240, 210, 30, 255, 17, 42, 93, 255, 71,
            22, 199, 255, 201, 90, 8, 255, 10, 20, 30, 255,
        ],
    )
    .unwrap()
}

fn fixture(png: &Path, root: &Path) -> ValidationFixture {
    ValidationFixture::from_values(Some(png.into()), Some(root.into()))
        .unwrap()
        .unwrap()
}

fn add_window(cx: &mut TestAppContext) -> (Entity<AppRoot>, &mut VisualTestContext) {
    let mut view = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let inner = cx.new(|_| test_app_root(ActivePanel::Screenshot));
        view = Some(inner.clone());
        gpui_component::Root::new(inner, window, cx)
    });
    (view.unwrap(), cx)
}

#[test]
fn screenshot_validation_requires_both_explicit_absolute_paths() {
    let _isolation = crate::config::tests::IsolatedDataDir::new("screenshot-validation-gate");
    let root = crate::config::data_dir();
    let png = root.join("synthetic.png");
    assert!(ValidationFixture::from_values(None, None)
        .unwrap()
        .is_none());
    assert!(
        ValidationFixture::from_values(None, Some(root.clone().into()))
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        ValidationFixture::from_values(Some(png.clone().into()), None),
        Err(ValidationError::Gate)
    ));
    for (png, root) in [
        (PathBuf::new(), root.clone()),
        (PathBuf::from("synthetic.png"), root),
        (png, PathBuf::from("relative-data")),
    ] {
        assert!(matches!(
            ValidationFixture::from_values(Some(png.into()), Some(root.into())),
            Err(ValidationError::Gate)
        ));
    }
    // PNG env를 수정하지 않는다. config 공용 격리 guard 이외 전역 환경 변경은 없다.
}

#[test]
fn screenshot_validation_rejects_escape_directory_wrong_format_and_size() {
    let _isolation = crate::config::tests::IsolatedDataDir::new("screenshot-validation-path");
    let outer = crate::config::data_dir();
    let allowed = outer.join("allowed");
    fs::create_dir(&allowed).unwrap();
    let outside = outer.join("outside.png");
    let bytes = synthetic().png().unwrap();
    fs::write(&outside, &bytes).unwrap();
    assert!(matches!(
        fixture(&outside, &allowed).load(),
        Err(ValidationError::Path)
    ));
    assert!(matches!(
        fixture(&allowed.join("../outside.png"), &allowed).load(),
        Err(ValidationError::Path)
    ));
    assert!(matches!(
        fixture(&allowed, &allowed).load(),
        Err(ValidationError::Path)
    ));
    let wrong_extension = allowed.join("synthetic.bin");
    fs::write(&wrong_extension, &bytes).unwrap();
    assert!(matches!(
        fixture(&wrong_extension, &allowed).load(),
        Err(ValidationError::Path)
    ));
    let invalid = allowed.join("invalid.png");
    fs::write(&invalid, b"synthetic non-PNG content").unwrap();
    assert!(matches!(
        fixture(&invalid, &allowed).load(),
        Err(ValidationError::Decode)
    ));
    let huge = allowed.join("oversized.png");
    File::create(&huge)
        .unwrap()
        .set_len(MAX_ENCODED_BYTES + 1)
        .unwrap();
    assert!(matches!(
        fixture(&huge, &allowed).load(),
        Err(ValidationError::Limit)
    ));
    assert_eq!(fs::read(&outside).unwrap(), bytes);
}

#[test]
fn screenshot_validation_png_limits_precede_pixel_allocation_and_crop_is_immutable() {
    assert!(validate_dimensions(4096, 2048).is_ok());
    for (width, height) in [
        (0, 1),
        (1, 0),
        (4097, 1),
        (4096, 2049),
        (u32::MAX, u32::MAX),
    ] {
        assert_eq!(
            validate_dimensions(width, height),
            Err(ValidationError::Limit)
        );
    }
    let original = synthetic();
    let bytes = original.png().unwrap();
    let decoded = decode_png(&bytes).unwrap();
    assert!(decoded == original);
    for len in [0, 8, 32, bytes.len() / 2] {
        assert!(decode_png(&bytes[..len]).is_err());
    }
    let mut giant_header = bytes.clone();
    giant_header[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(matches!(
        decode_png(&giant_header),
        Err(ValidationError::Limit)
    ));
    let selected = PixelRect {
        x: 1,
        y: 0,
        width: 2,
        height: 2,
    };
    let cropped = decoded.crop(selected).unwrap();
    let pixels = image::load_from_memory(&cropped.png().unwrap())
        .unwrap()
        .into_rgba8();
    assert_eq!(pixels.dimensions(), (2, 2));
    assert_eq!(pixels.get_pixel(0, 0).0, [0, 255, 0, 255]);
    assert_eq!(pixels.get_pixel(1, 1).0, [201, 90, 8, 255]);
    assert_eq!(decoded.png().unwrap(), bytes);
    assert!(decoded
        .crop(PixelRect {
            x: 4,
            y: 0,
            width: 1,
            height: 1
        })
        .is_err());
}

#[gpui::test]
fn screenshot_validation_gpui_seeds_preview_without_autosave_then_crops_synthetic(
    cx: &mut TestAppContext,
) {
    let _isolation = crate::config::tests::IsolatedDataDir::new("screenshot-validation-preview");
    let root = crate::config::data_dir();
    let path = root.join("synthetic.png");
    let bytes = synthetic().png().unwrap();
    fs::write(&path, &bytes).unwrap();
    initialize_components(cx);
    let (view, cx) = add_window(cx);
    cx.simulate_resize(size(px(1000.0), px(1000.0)));
    refresh(cx);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.screenshot.config.auto_save = true;
            this.screenshot.config.save_folder = root.to_string_lossy().into_owned();
            this.start_screenshot_validation_preview(fixture(&path, &root), window, cx);
        })
    });
    cx.run_until_parked();
    refresh(cx);
    view.read_with(cx, |this, _| {
        assert!(!this.screenshot.busy && !this.screenshot.failed);
        assert!(this.screenshot.preview.is_some());
        assert!(this.screenshot.status.contains("합성 검증 PNG"));
        assert!(this.screenshot.validation_seed_attempted);
        assert!(this.screenshot.config.auto_save);
        let image = this.screenshot.image.as_ref().unwrap();
        assert!(image.as_ref() == &synthetic());
        assert!(this.screenshot.selection.is_none());
    });
    let rect = PixelRect {
        x: 1,
        y: 0,
        width: 2,
        height: 2,
    };
    cx.update(|_, cx| view.update(cx, |this, _| this.screenshot.selection = Some(rect)));
    click_debug_element(cx, "screenshot-crop");
    cx.run_until_parked();
    view.read_with(cx, |this, _| {
        assert!(!this.screenshot.busy && !this.screenshot.failed);
        assert!(
            this.screenshot.image.as_ref().unwrap().as_ref() == &synthetic().crop(rect).unwrap()
        );
        assert!(this.screenshot.selection.is_none());
    });
    assert!(cx.read_from_clipboard().is_none()); // TestApp의 mock만 조회한다.
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
}

#[gpui::test]
fn screenshot_validation_gpui_invalid_seed_keeps_preview_selection_and_settings(
    cx: &mut TestAppContext,
) {
    let _isolation = crate::config::tests::IsolatedDataDir::new("screenshot-validation-invalid");
    let root = crate::config::data_dir();
    let path = root.join("invalid.png");
    fs::write(&path, b"invalid synthetic PNG").unwrap();
    initialize_components(cx);
    let (view, cx) = add_window(cx);
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.install_screenshot(synthetic(), window, cx)
        })
    });
    let before = view.read_with(cx, |this, _| this.screenshot.image.clone().unwrap());
    let preview = view.read_with(cx, |this, _| this.screenshot.preview.clone().unwrap());
    let rect = PixelRect {
        x: 1,
        y: 0,
        width: 2,
        height: 2,
    };
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            this.screenshot.config.auto_save = true;
            this.screenshot.selection = Some(rect);
            this.start_screenshot_validation_preview(fixture(&path, &root), window, cx);
        })
    });
    cx.run_until_parked();
    view.read_with(cx, |this, _| {
        assert!(this.screenshot.failed && !this.screenshot.busy);
        assert!(Arc::ptr_eq(
            this.screenshot.image.as_ref().unwrap(),
            &before
        ));
        assert!(Arc::ptr_eq(
            this.screenshot.preview.as_ref().unwrap(),
            &preview
        ));
        assert_eq!(this.screenshot.selection, Some(rect));
        assert!(this.screenshot.config.auto_save);
        assert_eq!(this.screenshot.status, ValidationError::Decode.to_string());
        assert!(!this.screenshot.status.contains(root.to_str().unwrap()));
    });
    // 이미 소비한 요청은 env 재조회/재시드하지 않는다. 외부 env를 수정할 필요가 없다.
    cx.update(|window, cx| {
        view.update(cx, |this, cx| {
            assert!(!this.seed_screenshot_validation_preview(window, cx));
        })
    });
    assert!(cx.read_from_clipboard().is_none());
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
}
