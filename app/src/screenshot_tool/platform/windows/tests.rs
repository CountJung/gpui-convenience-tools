//! 화면 DC·창 없이 소유 메모리 DIB의 색상 패턴만 제품 GDI 경계로 읽는다.
use super::*;
use windows_sys::Win32::Graphics::Gdi::CreateDIBSection;

#[test]
fn screenshot_gdi_owned_memory_source_capture_and_cleanup() {
    // SAFETY: NULL은 화면 DC를 읽지 않고 새 메모리 DC만 만든다.
    let mut source = BitmapDc {
        dc: unsafe { CreateCompatibleDC(0) },
        bitmap: 0,
        old: 0,
    };
    assert_ne!(source.dc, 0);
    // SAFETY: C POD를 초기화하고 정확히 32×24 top-down 32bpp를 지정한다.
    let mut info: BITMAPINFO = unsafe { std::mem::zeroed() };
    info.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: 32,
        biHeight: -24,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB,
        ..info.bmiHeader
    };
    let mut bits = std::ptr::null_mut();
    // SAFETY: OS 할당 DIB는 source가 끝까지 소유하며 다른 DC에 선택되지 않았다.
    source.bitmap = unsafe { CreateDIBSection(source.dc, &info, DIB_RGB_COLORS, &mut bits, 0, 0) };
    assert!(source.bitmap != 0 && !bits.is_null());
    // SAFETY: DIB의 정확한 크기만 쓰며 GDI는 아직 이 surface를 사용하지 않았다.
    let pixels = unsafe { std::slice::from_raw_parts_mut(bits.cast::<u8>(), 32 * 24 * 4) };
    for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
        pixel.copy_from_slice(if index / 32 < 12 {
            &[0, 0, 255, 0]
        } else {
            &[255, 0, 0, 0]
        });
    }
    // SAFETY: 소유 DC에 소유 DIB를 선택하고 복원 핸들을 보관한다.
    source.old = unsafe { SelectObject(source.dc, source.bitmap) };
    assert!(source.old != 0 && source.old != -1);
    for _ in 0..8 {
        // read 이전 오류/조기 반환처럼 비트맵이 선택된 채 해제되는 경로도 확인한다.
        let mut early = BitmapDc::new(source.dc, 32, 24).unwrap();
        // 해제된 핸들의 사후 조회는 병렬 GPUI의 핸들 재사용과 경합한다.
        // 실제 해제 API의 성공과 소유권 초기화를 직접 단언한다.
        assert!(early.release());
        assert_eq!((early.dc, early.bitmap, early.old), (0, 0, 0));
        drop(early);
        let image = capture_rect(
            source.dc,
            DesktopRect {
                left: 0,
                top: 0,
                width: 32,
                height: 24,
            },
        )
        .unwrap();
        assert_eq!((image.width(), image.height()), (32, 24));
        let png = image.png().unwrap();
        let decoded = image::load_from_memory(&png).unwrap().into_rgba8();
        for (_, y, pixel) in decoded.enumerate_pixels() {
            assert_eq!(
                pixel.0,
                if y < 12 {
                    [255, 0, 0, 255]
                } else {
                    [0, 0, 255, 255]
                }
            );
        }
    }
    assert!(source.release());
    assert_eq!((source.dc, source.bitmap, source.old), (0, 0, 0));
}
