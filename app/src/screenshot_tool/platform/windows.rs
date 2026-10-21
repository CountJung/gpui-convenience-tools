//! GDI 자원은 RAII로 복원·해제한다. 실제 화면 캡처는 명시적인 사용자 요청에만 실행한다.
use super::{
    resolve_capture_rect, CaptureRequest, DesktopRect, Monitor, ScreenshotError, ScreenshotImage,
};
use windows_sys::Win32::{
    Foundation::{BOOL, HWND, LPARAM, RECT},
    Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject,
        EnumDisplayDevicesW, EnumDisplayMonitors, GetDC, GetDIBits, GetMonitorInfoW, ReleaseDC,
        SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS,
        DISPLAY_DEVICEW, DISPLAY_DEVICE_ACTIVE, HBITMAP, HDC, HGDIOBJ, HMONITOR, MONITORINFOEXW,
        SRCCOPY,
    },
    UI::HiDpi::{SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2},
};

struct DpiGuard(isize);
impl DpiGuard {
    fn new() -> Result<Self, ScreenshotError> {
        // SAFETY: 현재 스레드만 바꾸며 이전 컨텍스트는 Drop에서 반드시 복원한다.
        let previous =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        if previous == 0 {
            Err(ScreenshotError::DpiFailed)
        } else {
            Ok(Self(previous))
        }
    }
}
impl Drop for DpiGuard {
    fn drop(&mut self) {
        // SAFETY: SetThreadDpiAwarenessContext가 반환한 유효한 이전 스레드 컨텍스트다.
        unsafe {
            if SetThreadDpiAwarenessContext(self.0) == 0 {
                log::warn!("스크린샷 작업 스레드의 DPI 컨텍스트 복원 실패");
            }
        }
    }
}

struct SourceDc {
    hwnd: HWND,
    dc: HDC,
}
impl Drop for SourceDc {
    fn drop(&mut self) {
        // SAFETY: GetDC로 획득한 DC를 동일한 HWND와 함께 반환한다.
        unsafe {
            ReleaseDC(self.hwnd, self.dc);
        }
    }
}

struct BitmapDc {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
}
impl BitmapDc {
    fn new(source: HDC, width: u32, height: u32) -> Result<Self, ScreenshotError> {
        super::super::byte_len(width, height)?;
        let width = i32::try_from(width).map_err(|_| ScreenshotError::InvalidDimensions)?;
        let height = i32::try_from(height).map_err(|_| ScreenshotError::InvalidDimensions)?;
        // SAFETY: source는 호출 내내 유효하며 크기는 검증된 양수다.
        let dc = unsafe { CreateCompatibleDC(source) };
        if dc == 0 {
            return Err(ScreenshotError::CaptureFailed);
        }
        let mut result = Self {
            dc,
            bitmap: 0,
            old: 0,
        };
        // SAFETY: 같은 source DC와 검증된 크기를 사용한다.
        result.bitmap = unsafe { CreateCompatibleBitmap(source, width, height) };
        if result.bitmap == 0 {
            return Err(ScreenshotError::CaptureFailed);
        }
        // SAFETY: 새 비트맵은 다른 DC에 선택되지 않았고 복원 핸들을 보관한다.
        result.old = unsafe { SelectObject(dc, result.bitmap) };
        if result.old == 0 || result.old == -1 {
            return Err(ScreenshotError::CaptureFailed);
        }
        Ok(result)
    }

    fn read(&mut self, width: u32, height: u32) -> Result<ScreenshotImage, ScreenshotError> {
        // GetDIBits에는 어떤 DC에도 선택되지 않은 비트맵을 전달해야 한다.
        // SAFETY: old는 생성 시 반환된 원래 비트맵이며 DC는 여전히 살아 있다.
        let restored = unsafe { SelectObject(self.dc, self.old) };
        if restored == 0 || restored == -1 {
            return Err(ScreenshotError::CaptureFailed);
        }
        self.old = 0;
        let mut pixels = super::super::zeroed_pixels(width, height)?;
        // SAFETY: BITMAPINFO는 C POD이며 나머지 필드는 0 초기화가 유효하다.
        let mut info: BITMAPINFO = unsafe { std::mem::zeroed() };
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..info.bmiHeader
        };
        // SAFETY: top-down 32bpp의 정확한 width*height*4 용량을 할당했다.
        let lines = unsafe {
            GetDIBits(
                self.dc,
                self.bitmap,
                0,
                height,
                pixels.as_mut_ptr().cast(),
                &mut info,
                DIB_RGB_COLORS,
            )
        };
        if lines != height as i32 {
            return Err(ScreenshotError::CaptureFailed);
        }
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.swap(0, 2);
            pixel[3] = 255; // BI_RGB의 미정의 alpha는 완전 불투명으로 정규화한다.
        }
        ScreenshotImage::new(width, height, pixels)
    }
    fn release(&mut self) -> bool {
        let mut success = true;
        // SAFETY: 자신이 소유한 핸들만 다룬다. 복원 실패 시에도 DC를 먼저 해제하여
        // 비트맵이 선택된 채 DeleteObject에 실패하는 경로를 막는다.
        unsafe {
            if self.old != 0 && self.old != -1 {
                let restored = SelectObject(self.dc, self.old);
                success &= restored != 0 && restored != -1;
            }
            self.old = 0;
            if self.dc != 0 {
                success &= DeleteDC(self.dc) != 0;
                self.dc = 0;
            }
            if self.bitmap != 0 {
                success &= DeleteObject(self.bitmap) != 0;
                self.bitmap = 0;
            }
        }
        success
    }
}
impl Drop for BitmapDc {
    fn drop(&mut self) {
        if !self.release() {
            log::warn!("스크린샷 비트맵/DC 자원 해제 실패");
        }
    }
}

#[cfg_attr(test, allow(dead_code))]
pub(super) fn monitors() -> Result<Vec<Monitor>, ScreenshotError> {
    let _dpi = DpiGuard::new()?;
    enumerate()
}

#[cfg_attr(test, allow(dead_code))]
fn enumerate() -> Result<Vec<Monitor>, ScreenshotError> {
    struct Enumeration {
        monitors: Vec<Monitor>,
        error: Option<ScreenshotError>,
    }
    unsafe extern "system" fn callback(
        handle: HMONITOR,
        _: HDC,
        _: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        // SAFETY: EnumDisplayMonitors가 동기 호출하는 동안 Enumeration 포인터가 유효하다.
        let enumeration = unsafe { &mut *(data as *mut Enumeration) };
        // SAFETY: MONITORINFOEXW는 C POD이며 cbSize를 API 호출 전에 설정한다.
        let mut info: MONITORINFOEXW = unsafe { std::mem::zeroed() };
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        // SAFETY: 열거 중인 유효한 monitor와 크기가 맞는 출력 구조체를 전달한다.
        if unsafe { GetMonitorInfoW(handle, &mut info.monitorInfo) } == 0 {
            enumeration.error = Some(ScreenshotError::MonitorChanged);
            return 0;
        }
        let device_id = match device_identity(&info.szDevice) {
            Ok(id) => id,
            Err(error) => {
                enumeration.error = Some(error);
                return 0;
            }
        };
        let r = info.monitorInfo.rcMonitor;
        let width = i64::from(r.right) - i64::from(r.left);
        let height = i64::from(r.bottom) - i64::from(r.top);
        if width <= 0 || height <= 0 || width > i64::from(i32::MAX) || height > i64::from(i32::MAX)
        {
            enumeration.error = Some(ScreenshotError::InvalidDimensions);
            return 0;
        }
        enumeration.monitors.push(Monitor {
            device_id,
            rect: DesktopRect {
                left: r.left,
                top: r.top,
                width: width as u32,
                height: height as u32,
            },
            primary: info.monitorInfo.dwFlags & 1 != 0,
        });
        1
    }
    let mut enumeration = Enumeration {
        monitors: Vec::new(),
        error: None,
    };
    // SAFETY: callback은 data에 전달한 구조체만 사용하며 이 호출이 끝나기 전에 종료된다.
    if unsafe {
        EnumDisplayMonitors(
            0,
            std::ptr::null(),
            Some(callback),
            &mut enumeration as *mut Enumeration as isize,
        )
    } == 0
    {
        return Err(enumeration.error.unwrap_or(ScreenshotError::CaptureFailed));
    }
    let mut monitors = enumeration.monitors;
    if monitors.is_empty() {
        return Err(ScreenshotError::MonitorUnavailable);
    }
    monitors.sort_by_key(|m| (!m.primary, m.rect.left, m.rect.top));
    Ok(monitors)
}

/// DISPLAY1 등의 표시 이름은 장치 ID가 아니다. OS의 모니터 인터페이스 경로를 사용한다.
#[cfg_attr(test, allow(dead_code))]
fn device_identity(adapter: &[u16; 32]) -> Result<String, ScreenshotError> {
    let mut active = None;
    for index in 0..32 {
        // SAFETY: DISPLAY_DEVICEW는 C POD이며 cb를 초기화하고 호출 동안 버퍼를 유지한다.
        let mut device: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
        device.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
        // EDD_GET_DEVICE_INTERFACE_NAME=1: DeviceID에 GUID_DEVINTERFACE_MONITOR 경로를 받는다.
        // SAFETY: GetMonitorInfoW가 반환한 NUL 종료 adapter명과 유효한 출력 버퍼다.
        if unsafe { EnumDisplayDevicesW(adapter.as_ptr(), index, &mut device, 1) } == 0 {
            return active.ok_or(ScreenshotError::MonitorChanged);
        }
        if device.StateFlags & DISPLAY_DEVICE_ACTIVE == 0 {
            continue;
        }
        let end = device
            .DeviceID
            .iter()
            .position(|c| *c == 0)
            .filter(|end| *end > 0)
            .ok_or(ScreenshotError::MonitorChanged)?;
        let id = String::from_utf16(&device.DeviceID[..end])
            .map_err(|_| ScreenshotError::MonitorChanged)?;
        // 복제/모호한 장치를 인덱스로 추측하지 않는다.
        if active.replace(id).is_some() {
            return Err(ScreenshotError::MonitorChanged);
        }
    }
    Err(ScreenshotError::MonitorChanged)
}

#[cfg_attr(test, allow(dead_code))]
pub(super) fn capture(request: CaptureRequest) -> Result<ScreenshotImage, ScreenshotError> {
    let _dpi = DpiGuard::new()?;
    let monitors = enumerate()?;
    // 이 검증 이전에는 데스크톱 DC를 얻거나 화면 픽셀을 읽지 않는다.
    let rect = resolve_capture_rect(&request, &monitors)?;
    super::super::byte_len(rect.width, rect.height)?;
    // SAFETY: NULL HWND는 데스크톱 DC. 이 경로는 명시적인 수동 캡처에만 사용한다.
    let source = SourceDc {
        hwnd: 0,
        dc: unsafe { GetDC(0) },
    };
    if source.dc == 0 {
        return Err(ScreenshotError::CaptureFailed);
    }
    let image = capture_rect(source.dc, rect)?;
    // 캡처 도중 핫플러그/주 모니터 전환이 관찰되면 결과를 설치·자동 저장하지 않는다.
    resolve_capture_rect(&request, &enumerate()?)?;
    Ok(image)
}

/// source는 호출 내내 살아 있는 동일 스레드의 DC다. rect는 그 DC 기준 좌표다.
/// 제품은 검증된 데스크톱 범위만, fixture는 소유한 DIB의 클라이언트 범위만 전달한다.
fn capture_rect(source: HDC, rect: DesktopRect) -> Result<ScreenshotImage, ScreenshotError> {
    super::super::byte_len(rect.width, rect.height)?;
    if source == 0 {
        return Err(ScreenshotError::CaptureFailed);
    }
    let mut bitmap = BitmapDc::new(source, rect.width, rect.height)?;
    // SAFETY: 목적 비트맵 크기와 source DC 기준의 검증된 픽셀 좌표를 사용한다.
    let ok = unsafe {
        BitBlt(
            bitmap.dc,
            0,
            0,
            rect.width as i32,
            rect.height as i32,
            source,
            rect.left,
            rect.top,
            SRCCOPY | CAPTUREBLT,
        )
    };
    if ok == 0 {
        return Err(ScreenshotError::CaptureFailed);
    }
    bitmap.read(rect.width, rect.height)
}

#[cfg(test)]
mod owned_client_fixture;
#[cfg(test)]
mod tests;
