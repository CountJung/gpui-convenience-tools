//! 플랫폼 경계. 모니터 목록 조회는 픽셀을 읽지 않으며 캡처는 버튼 이벤트에서만 호출한다.
use super::{ScreenshotError, ScreenshotImage};
use crate::config::CaptureTarget;

#[cfg(test)]
pub(crate) mod tests;
#[cfg(target_os = "windows")]
mod windows;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DesktopRect {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct Monitor {
    /// EnumDisplayDevicesW의 GUID_DEVINTERFACE_MONITOR 경로. 표시 순서와 독립적이다.
    pub device_id: String,
    pub rect: DesktopRect,
    pub primary: bool,
}

#[cfg_attr(test, allow(dead_code))] // GPUI 테스트는 합성 모니터만 사용한다.
pub(crate) fn monitors() -> Result<Vec<Monitor>, ScreenshotError> {
    #[cfg(target_os = "windows")]
    {
        windows::monitors()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err(ScreenshotError::Unsupported)
    }
}

pub(crate) struct CaptureRequest {
    pub target: CaptureTarget,
    pub device_id: Option<String>,
    pub topology: Vec<String>,
}

#[cfg_attr(test, allow(dead_code))] // 테스트에서는 실제 화면 캡처를 호출하지 않는다.
pub(crate) fn capture(request: CaptureRequest) -> Result<ScreenshotImage, ScreenshotError> {
    #[cfg(target_os = "windows")]
    {
        windows::capture(request)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = request;
        Err(ScreenshotError::Unsupported)
    }
}

/// 장치별 물리 배치·주 모니터 여부를 함께 고정한다. 열거 순서 자체에는 의존하지 않는다.
pub(crate) fn topology(monitors: &[Monitor]) -> Vec<String> {
    let mut stamps: Vec<_> = monitors
        .iter()
        .map(|m| {
            format!(
                "{}:{}:{},{},{},{}:{}",
                m.device_id.len(),
                m.device_id,
                m.rect.left,
                m.rect.top,
                m.rect.width,
                m.rect.height,
                m.primary
            )
        })
        .collect();
    stamps.sort_unstable();
    stamps
}

pub(crate) fn resolve_capture_rect(
    request: &CaptureRequest,
    monitors: &[Monitor],
) -> Result<DesktopRect, ScreenshotError> {
    if request.topology.is_empty() || request.topology != topology(monitors) {
        return Err(ScreenshotError::MonitorChanged);
    }
    match request.target {
        CaptureTarget::AllDisplays => desktop_union(monitors),
        CaptureTarget::Monitor(_) => {
            let id = request
                .device_id
                .as_deref()
                .filter(|id| !id.is_empty())
                .ok_or(ScreenshotError::MonitorChanged)?;
            let mut matches = monitors.iter().filter(|m| m.device_id == id);
            let monitor = matches.next().ok_or(ScreenshotError::MonitorChanged)?;
            if matches.next().is_some() {
                return Err(ScreenshotError::MonitorChanged);
            }
            Ok(monitor.rect)
        }
    }
}

/// 음수 원점을 포함한 물리 픽셀 합집합. 덧셈은 i64로 계산해 오버플로를 차단한다.
pub(crate) fn desktop_union(monitors: &[Monitor]) -> Result<DesktopRect, ScreenshotError> {
    let first = monitors
        .first()
        .ok_or(ScreenshotError::MonitorUnavailable)?;
    let mut left = i64::from(first.rect.left);
    let mut top = i64::from(first.rect.top);
    let mut right = left;
    let mut bottom = top;
    for monitor in monitors {
        left = left.min(i64::from(monitor.rect.left));
        top = top.min(i64::from(monitor.rect.top));
        right = right.max(i64::from(monitor.rect.left) + i64::from(monitor.rect.width));
        bottom = bottom.max(i64::from(monitor.rect.top) + i64::from(monitor.rect.height));
    }
    let width = u32::try_from(right - left).map_err(|_| ScreenshotError::InvalidDimensions)?;
    let height = u32::try_from(bottom - top).map_err(|_| ScreenshotError::InvalidDimensions)?;
    super::byte_len(width, height)?;
    Ok(DesktopRect {
        left: left as i32,
        top: top as i32,
        width,
        height,
    })
}
