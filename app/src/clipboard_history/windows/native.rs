//! Win32 잠금을 유지한 채 GlobalSize 상한 확인 후에만 본문을 복사한다.
use super::*;
use std::ffi::c_void;

#[link(name = "user32")]
extern "system" {
    fn OpenClipboard(owner: isize) -> i32;
    fn CloseClipboard() -> i32;
    fn RegisterClipboardFormatW(name: *const u16) -> u32;
    fn IsClipboardFormatAvailable(format: u32) -> i32;
    fn GetClipboardSequenceNumber() -> u32;
    fn GetClipboardData(format: u32) -> isize;
}
#[link(name = "kernel32")]
extern "system" {
    fn GlobalSize(memory: isize) -> usize;
    fn GlobalLock(memory: isize) -> *const c_void;
    fn GlobalUnlock(memory: isize) -> i32;
}

struct Open;
impl Open {
    fn new() -> Result<Self, CaptureResult> {
        // SAFETY: null owner, 동일 스레드의 Drop에서 닫는다.
        if unsafe { OpenClipboard(0) } == 0 {
            Err(CaptureResult::Unavailable)
        } else {
            Ok(Self)
        }
    }
}
impl Drop for Open {
    fn drop(&mut self) {
        // SAFETY: 이 인스턴스가 성공적으로 연 잠금만 닫는다.
        unsafe {
            CloseClipboard();
        }
    }
}

fn format(name: &str) -> u32 {
    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    // SAFETY: 호출 동안 유효한 NUL 종료 UTF-16.
    unsafe { RegisterClipboardFormatW(wide.as_ptr()) }
}

fn password_focus() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetGUIThreadInfo, GetWindowLongW, ES_PASSWORD, GUITHREADINFO, GWL_STYLE,
    };
    // SAFETY: 올바른 크기의 출력 구조체이며 창 텍스트는 읽지 않는다.
    unsafe {
        let mut info: GUITHREADINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
        GetGUIThreadInfo(0, &mut info) != 0
            && info.hwndFocus != 0
            && GetWindowLongW(info.hwndFocus, GWL_STYLE) & ES_PASSWORD != 0
    }
}

fn locked_probe() -> Probe {
    if password_focus() {
        return Probe::Sensitive;
    }
    for name in [
        "ExcludeClipboardContentFromMonitorProcessing",
        "CanIncludeInClipboardHistory",
        "Clipboard Viewer Ignore",
        "application/x-keepassxc",
        "application/x-kde-passwordManagerHint",
    ] {
        let id = format(name);
        if id == 0 {
            return Probe::Unavailable;
        }
        // SAFETY: 형식 존재 여부만 읽으며 본문은 읽지 않는다.
        if unsafe { IsClipboardFormatAvailable(id) } != 0 {
            return Probe::Sensitive;
        }
    }
    Probe::Allowed(sequence())
}

pub fn probe() -> Probe {
    if password_focus() {
        return Probe::Sensitive;
    }
    let Ok(_open) = Open::new() else {
        return Probe::Unavailable;
    };
    locked_probe()
}

pub fn sequence() -> u32 {
    // SAFETY: 인수·포인터 없이 변경 번호만 읽는다.
    unsafe { GetClipboardSequenceNumber() }
}

struct NativeSource {
    _open: Open,
    handle: isize,
}
impl SnapshotSource for NativeSource {
    fn probe(&self) -> Probe {
        locked_probe()
    }
    fn sequence(&self) -> u32 {
        sequence()
    }
    fn metadata(
        &mut self,
        config: &ClipboardConfig,
    ) -> Result<(SnapshotKind, usize), CaptureResult> {
        let png = format("PNG");
        if png == 0 {
            return Err(CaptureResult::Unavailable);
        }
        // PNG가 존재하더라도 image OFF이면 GetClipboardData 자체를 호출하지 않는다.
        let available = |id| unsafe { IsClipboardFormatAvailable(id) != 0 };
        let (kind, id) = if config.collect_images && available(png) {
            (SnapshotKind::Png, png)
        } else if config.collect_text && available(13) {
            (SnapshotKind::Utf16, 13) // CF_UNICODETEXT
        } else {
            return Err(CaptureResult::Disabled);
        };
        // SAFETY: 클립보드 잠금 중 HGLOBAL만 얻고 아직 본문을 복사하지 않는다.
        self.handle = unsafe { GetClipboardData(id) };
        if self.handle == 0 {
            return Err(CaptureResult::Unavailable);
        }
        // SAFETY: 선택한 UTF-16/PNG 형식은 HGLOBAL이며 크기만 조회한다.
        Ok((kind, unsafe { GlobalSize(self.handle) }))
    }
    fn copy(&self, bytes: usize) -> Result<Vec<u8>, CaptureResult> {
        // SAFETY: read_bounded에서 유형·크기·sequence를 확인했다. 잠금 동안 유효하다.
        unsafe {
            let data = GlobalLock(self.handle).cast::<u8>();
            if data.is_null() {
                return Err(CaptureResult::Unavailable);
            }
            let result = std::slice::from_raw_parts(data, bytes).to_vec();
            GlobalUnlock(self.handle);
            Ok(result)
        }
    }
}

pub fn snapshot(config: &ClipboardConfig, expected: u32) -> Result<Candidate, CaptureResult> {
    let mut source = NativeSource {
        _open: Open::new()?,
        handle: 0,
    };
    read_bounded(&mut source, config, expected)
}
