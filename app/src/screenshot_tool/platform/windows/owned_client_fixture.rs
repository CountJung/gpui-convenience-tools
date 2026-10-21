//! 명시적 별도 실행 전용. 개인 화면/클립보드/파일에 접근하지 않는 실제 GDI fixture.
//!
//! 화면 DC의 BitBlt는 창 가림과 합성 타이밍을 완전히 차단할 수 없다. 따라서 소유한
//! STATIC 팝업이 PW_CLIENTONLY로 직접 렌더한 전용 DIB를 source로 삼는다. 제품과 같은
//! capture_rect의 BitBlt/CAPTUREBLT → GetDIBits 경계를 검증하지만 실제 데스크톱 합성,
//! 장치 열거, 혼합 DPI 모니터 배치를 검증했다고 주장하지 않는다.
use super::*;
use windows_sys::Win32::{
    Graphics::Gdi::{CreateDIBSection, GdiFlush, UpdateWindow},
    Storage::Xps::PrintWindow,
    UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, GetClientRect, GetForegroundWindow, IsWindow,
        IsWindowVisible, ShowWindow, SW_SHOWNOACTIVATE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        WS_POPUP,
    },
};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 200;
const SS_WHITERECT: u32 = 6;
const PW_CLIENTONLY: u32 = 1;

struct OwnedPopup(HWND);

impl OwnedPopup {
    fn show() -> Self {
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        // SAFETY: 내장 단색 STATIC 클래스, 빈 제목, 부모/자식 없음. WS_VISIBLE 대신
        // SW_SHOWNOACTIVATE로 표시하며 어떤 입력·활성화·포커스 설정 API도 호출하지 않는다.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                class.as_ptr(),
                std::ptr::null(),
                WS_POPUP | SS_WHITERECT,
                16,
                16,
                WIDTH as i32,
                HEIGHT as i32,
                0,
                0,
                0,
                std::ptr::null(),
            )
        };
        assert_ne!(hwnd, 0, "owned fixture popup creation failed");
        // 첫 assert 이후의 모든 경로에서 창을 RAII로 정리한다.
        let popup = Self(hwnd);
        // SAFETY: 이 스레드가 생성한 소유 HWND만 표시·갱신한다.
        unsafe {
            ShowWindow(popup.0, SW_SHOWNOACTIVATE);
            UpdateWindow(popup.0);
        }
        popup
    }

    fn client_rect(&self) -> DesktopRect {
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        // SAFETY: 살아 있는 소유 HWND의 메타데이터와 유효한 출력 버퍼만 사용한다.
        assert_ne!(unsafe { GetClientRect(self.0, &mut rect) }, 0);
        assert_eq!(
            (rect.left, rect.top, rect.right, rect.bottom),
            (0, 0, WIDTH as i32, HEIGHT as i32)
        );
        DesktopRect {
            left: 0,
            top: 0,
            width: WIDTH,
            height: HEIGHT,
        }
    }
}

impl Drop for OwnedPopup {
    fn drop(&mut self) {
        // SAFETY: 이 스레드에서 생성한 소유 팝업만 파괴한다. 패닉 중 이중 패닉을 만들지 않는다.
        if unsafe { DestroyWindow(self.0) } == 0 {
            log::error!("스크린샷 소유 GDI fixture 창 정리 실패");
        }
    }
}

struct OwnedClientDib {
    bitmap: BitmapDc,
    rect: DesktopRect,
}

impl OwnedClientDib {
    fn render(popup: &OwnedPopup) -> Result<Self, ScreenshotError> {
        let rect = popup.client_rect();
        let len = super::super::super::byte_len(rect.width, rect.height)?;
        // SAFETY: NULL은 화면 DC를 가져오지 않고 새 메모리 DC만 생성한다.
        let dc = unsafe { CreateCompatibleDC(0) };
        if dc == 0 {
            return Err(ScreenshotError::CaptureFailed);
        }
        let mut bitmap = BitmapDc {
            dc,
            bitmap: 0,
            old: 0,
        };
        // SAFETY: C POD의 0 초기화 후 top-down RGBA8과 같은 크기의 32bpp DIB를 지정한다.
        let mut info: BITMAPINFO = unsafe { std::mem::zeroed() };
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: rect.width as i32,
            biHeight: -(rect.height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..info.bmiHeader
        };
        let mut bits = std::ptr::null_mut();
        // SAFETY: 메모리는 OS가 할당하며 BitmapDc가 비트맵과 DC를 끝까지 소유한다.
        bitmap.bitmap = unsafe { CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, 0, 0) };
        if bitmap.bitmap == 0 || bits.is_null() {
            return Err(ScreenshotError::CaptureFailed);
        }
        // SAFETY: DWORD 정렬된 width*height*4 바이트를 소유한다. GDI 렌더 이전에만 직접 쓴다.
        unsafe { std::ptr::write_bytes(bits.cast::<u8>(), 0, len) };
        // SAFETY: 소유 DIB를 유일한 소유 DC에 선택하고 원래 객체를 RAII에 보관한다.
        bitmap.old = unsafe { SelectObject(dc, bitmap.bitmap) };
        if bitmap.old == 0 || bitmap.old == -1 {
            return Err(ScreenshotError::CaptureFailed);
        }
        // SAFETY: 유일한 대상은 제목/자식 없는 소유 단색 STATIC 팝업이다. 데스크톱 또는
        // 다른 HWND의 DC를 얻지 않으며 PW_CLIENTONLY로 이 창이 직접 DIB에 그리게 한다.
        if unsafe { PrintWindow(popup.0, dc, PW_CLIENTONLY) } == 0 {
            return Err(ScreenshotError::CaptureFailed);
        }
        // SAFETY: 이 테스트 스레드의 GDI 배치를 끝낸 후 제품의 픽셀 읽기 경로로 넘긴다.
        if unsafe { GdiFlush() } == 0 {
            return Err(ScreenshotError::CaptureFailed);
        }
        Ok(Self { bitmap, rect })
    }

    fn capture(&self) -> Result<ScreenshotImage, ScreenshotError> {
        // 임의의 화면 좌표를 받을 수 없다. source와 범위가 이 fixture의 소유 DIB로 고정된다.
        capture_rect(self.bitmap.dc, self.rect)
    }
}

#[test]
#[ignore = "Shows a 320x200 no-activate fixture popup; run separately only after notice and explicit approval"]
fn screenshot_gdi_owned_popup_client_dib_uses_product_capture_rect() {
    let _dpi = DpiGuard::new().expect("fixture DPI context unavailable");
    // SAFETY: 포커스 HWND 메타데이터만 읽는다. 사용자 포커스를 변경하거나 복구하지 않는다.
    let foreground = unsafe { GetForegroundWindow() };
    let popup = OwnedPopup::show();
    // SAFETY: 소유 창의 표시 여부와 전경 창 메타데이터만 읽는다.
    assert_ne!(unsafe { IsWindowVisible(popup.0) }, 0);
    // SAFETY: 전경 HWND를 읽을 뿐 활성화·포커스 API를 호출하지 않는다.
    assert_eq!(
        unsafe { GetForegroundWindow() },
        foreground,
        "foreground changed; do not restore over user input"
    );
    let mut surface = OwnedClientDib::render(&popup).expect("owned client DIB render failed");
    for _ in 0..4 {
        let image = surface.capture().expect("owned client GDI capture failed");
        assert_eq!((image.width(), image.height()), (WIDTH, HEIGHT));
        let png = image.png().expect("fixture PNG encoding failed");
        let decoded = image::load_from_memory(&png).unwrap().into_rgba8();
        assert_eq!(decoded.dimensions(), (WIDTH, HEIGHT));
        assert!(decoded
            .pixels()
            .all(|pixel| pixel.0 == [255, 255, 255, 255]));
    }
    assert!(surface.bitmap.release(), "owned DIB cleanup API failed");
    assert_eq!(
        (surface.bitmap.dc, surface.bitmap.bitmap, surface.bitmap.old),
        (0, 0, 0)
    );
    drop(surface);
    let hwnd = popup.0;
    drop(popup);
    // SAFETY: 소유했던 HWND의 파괴 여부만 조회한다.
    assert_eq!(unsafe { IsWindow(hwnd) }, 0, "owned popup cleanup failed");
    // SAFETY: 사용자 포커스를 변경하지 않고 전경 HWND 메타데이터만 읽는다.
    assert_eq!(
        unsafe { GetForegroundWindow() },
        foreground,
        "foreground changed; user focus was not modified/restored by fixture"
    );
}
