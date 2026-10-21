use super::*;

fn fixture() -> ScreenshotImage {
    ScreenshotImage::new(4, 3, (0..12).flat_map(|v| [v, 255 - v, 40, 255]).collect()).unwrap()
}

#[test]
fn screenshot_png_roundtrip_and_crop_preserve_original_pixels() {
    let image = fixture();
    let original = image.rgba.clone();
    let crop = image
        .crop(PixelRect {
            x: 1,
            y: 1,
            width: 2,
            height: 2,
        })
        .unwrap();
    assert_eq!(
        crop.rgba,
        [5u8, 6, 9, 10]
            .into_iter()
            .flat_map(|v| [v, 255 - v, 40, 255])
            .collect::<Vec<_>>()
    );
    let decoded =
        image::load_from_memory_with_format(&crop.png().unwrap(), image::ImageFormat::Png)
            .unwrap()
            .into_rgba8();
    assert_eq!(decoded.dimensions(), (2, 2));
    assert_eq!(decoded.as_raw(), &crop.rgba);
    assert_eq!(image.rgba, original);
    assert!(crop.matches_png(&crop.png().unwrap()));
    let mut alternative = Vec::new();
    image::codecs::png::PngEncoder::new_with_quality(
        &mut alternative,
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::NoFilter,
    )
    .write_image(
        &crop.rgba,
        crop.width(),
        crop.height(),
        ExtendedColorType::Rgba8,
    )
    .unwrap();
    assert!(crop.matches_png(&alternative));
    assert!(!image.matches_png(&alternative));
    assert!(!crop.matches_png(b"not a PNG fixture"));
    let render = crop.render_image().unwrap();
    assert_eq!(&render.as_bytes(0).unwrap()[..4], &[40, 250, 5, 255]);
}

#[test]
fn screenshot_dimensions_and_crop_edges_fail_closed() {
    assert!(ScreenshotImage::new(0, 1, vec![]).is_err());
    assert!(ScreenshotImage::new(u32::MAX, u32::MAX, vec![]).is_err());
    assert!(ScreenshotImage::new(4, 3, vec![0; 4]).is_err());
    assert!(fixture()
        .crop(PixelRect {
            x: 4,
            y: 0,
            width: 1,
            height: 1
        })
        .is_err());
    let clamped = fixture()
        .crop(PixelRect {
            x: 3,
            y: 2,
            width: u32::MAX,
            height: u32::MAX,
        })
        .unwrap();
    assert_eq!((clamped.width(), clamped.height()), (1, 1));
}

#[test]
fn screenshot_preview_letterbox_reverse_drag_scale_and_clamp() {
    // 모니터가 음수 좌표에 있거나 창 DPI가 바뀌어도 UI 논리 px를 이미지 픽셀로만 변환한다.
    let fit = PreviewRect {
        left: -100.0,
        top: 20.0,
        width: 600.0,
        height: 400.0,
    }
    .fit(1920, 1080)
    .unwrap();
    assert_eq!(fit.left, -100.0);
    assert_eq!(fit.width, 600.0);
    assert!((fit.top - 51.25).abs() < 0.01);
    assert_eq!(
        fit.selection((500.0, 388.75), (-100.0, 51.25), 1920, 1080)
            .unwrap(),
        PixelRect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080
        }
    );
    let full = fit
        .selection((-900.0, -900.0), (9000.0, 9000.0), 1920, 1080)
        .unwrap();
    assert_eq!(
        full,
        PixelRect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080
        }
    );
    let rect = PreviewRect {
        left: 0.0,
        top: 0.0,
        width: 200.0,
        height: 100.0,
    };
    assert_eq!(
        rect.selection((25.0, 10.0), (75.0, 60.0), 400, 200)
            .unwrap(),
        PixelRect {
            x: 50,
            y: 20,
            width: 100,
            height: 100
        }
    );
    assert!(!fit.contains((-101.0, 60.0)));
}

#[test]
fn screenshot_preview_rejects_empty_nonfinite_and_letterbox_points() {
    let rect = PreviewRect {
        left: 0.0,
        top: 0.0,
        width: 200.0,
        height: 100.0,
    };
    assert!(rect.selection((1.0, 1.0), (1.0, 1.0), 20, 10).is_err());
    assert!(rect.selection((1.0, 1.0), (f32::NAN, 2.0), 20, 10).is_err());
    assert!(rect
        .selection((-10.0, -10.0), (-1.0, -1.0), 20, 10)
        .is_err());
    assert!(PreviewRect {
        width: f32::INFINITY,
        ..rect
    }
    .fit(20, 10)
    .is_none());
    assert!(rect.fit(0, 10).is_none());
    let fit = rect.fit(100, 100).unwrap();
    assert_eq!(fit.left, 50.0);
    assert!(!fit.contains((25.0, 50.0)));
}

#[test]
fn screenshot_monitor_union_preserves_negative_origins_and_different_sizes() {
    use platform::{desktop_union, DesktopRect, Monitor};
    let monitors = [
        Monitor {
            device_id: "fixture-A".into(),
            rect: DesktopRect {
                left: -2560,
                top: -400,
                width: 2560,
                height: 1440,
            },
            primary: false,
        },
        Monitor {
            device_id: "fixture-B".into(),
            rect: DesktopRect {
                left: 0,
                top: 0,
                width: 3840,
                height: 2160,
            },
            primary: true,
        },
    ];
    assert_eq!(
        desktop_union(&monitors).unwrap(),
        DesktopRect {
            left: -2560,
            top: -400,
            width: 6400,
            height: 2560
        }
    );
    assert!(desktop_union(&[]).is_err());
}

#[test]
fn screenshot_png_save_creates_unique_names_without_overwrite_and_handles_failure() {
    let _isolation = crate::config::tests::IsolatedDataDir::new("screenshot-png");
    let directory = crate::config::data_dir();
    let target = directory.join("fixture.png");
    std::fs::write(&target, b"original-content").unwrap();
    let image = fixture();
    let next = save_png_new(&image, &target).unwrap();
    assert_eq!(next.file_name().unwrap(), "fixture (1).png");
    assert_eq!(std::fs::read(&target).unwrap(), b"original-content");
    assert_eq!(
        image::open(next).unwrap().into_rgba8().as_raw(),
        &image.rgba
    );
    let next = save_png_new(&image, &directory.join("fixture.jpg")).unwrap();
    assert_eq!(next.file_name().unwrap(), "fixture (2).png");
    assert_eq!(
        save_png_new(&image, &directory.join("missing/output.png")),
        Err(ScreenshotError::SaveFailed)
    );
    assert!(!directory.join("missing").exists());
}
