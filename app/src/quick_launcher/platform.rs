//! Windows API 경계. 경로와 실행 내용은 로그에 남기지 않는다.
use super::LauncherError;
use std::{path::PathBuf, process::Command};

pub fn normalize_path(path: PathBuf) -> Result<PathBuf, LauncherError> {
    #[cfg(windows)]
    {
        let text = path.to_str().ok_or(LauncherError::InvalidPath)?;
        Ok(PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(text)))
    }
    #[cfg(not(windows))]
    {
        Ok(path)
    }
}

#[cfg(windows)]
fn windows_directory(system: bool) -> Result<PathBuf, LauncherError> {
    use windows_sys::Win32::System::SystemInformation::{
        GetSystemDirectoryW, GetWindowsDirectoryW,
    };
    let mut buf = vec![0u16; 32768];
    // SAFETY: valid UTF-16 output buffer and capacity; check the returned length before reading.
    let length = unsafe {
        if system {
            GetSystemDirectoryW(buf.as_mut_ptr(), buf.len() as u32)
        } else {
            GetWindowsDirectoryW(buf.as_mut_ptr(), buf.len() as u32)
        }
    } as usize;
    if length == 0 || length >= buf.len() {
        return Err(LauncherError::StartFailed);
    }
    let text = String::from_utf16(&buf[..length]).map_err(|_| LauncherError::InvalidPath)?;
    Ok(PathBuf::from(text))
}

pub fn explorer() -> Result<PathBuf, LauncherError> {
    #[cfg(windows)]
    {
        Ok(windows_directory(false)?.join("explorer.exe"))
    }
    #[cfg(not(windows))]
    {
        Err(LauncherError::UnsupportedPlatform)
    }
}

pub fn powershell() -> Result<PathBuf, LauncherError> {
    #[cfg(windows)]
    {
        Ok(windows_directory(true)?.join("WindowsPowerShell/v1.0/powershell.exe"))
    }
    #[cfg(not(windows))]
    {
        Err(LauncherError::UnsupportedPlatform)
    }
}

pub fn hide_console(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    #[cfg(not(windows))]
    let _ = command;
}

/// Restore only this process's main window (including a window hidden to the tray).
/// Same show/restore policy as platform/windows/tray.rs; never targets another app instance.
#[cfg(all(windows, not(test)))]
pub fn restore_own_main_window() {
    use windows_sys::Win32::{
        Foundation::{BOOL, HWND, LPARAM},
        UI::WindowsAndMessaging::{
            EnumWindows, GetWindowTextW, GetWindowThreadProcessId, ShowWindow, SW_RESTORE, SW_SHOW,
        },
    };
    unsafe extern "system" fn visit(hwnd: HWND, _: LPARAM) -> BOOL {
        let mut process_id = 0;
        // SAFETY: output pointer is valid; EnumWindows supplies a live top-level window handle.
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut process_id);
        }
        if process_id != std::process::id() {
            return 1;
        }
        let mut title = [0u16; 128];
        // SAFETY: buffer is writable and capacity matches its allocated length.
        let count =
            unsafe { GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32) } as usize;
        if String::from_utf16_lossy(&title[..count]) != "gpui-convenience-tools" {
            return 1;
        }
        // SAFETY: the handle is verified as this process's own application main window.
        unsafe {
            ShowWindow(hwnd, SW_SHOW);
            ShowWindow(hwnd, SW_RESTORE);
        }
        0
    }
    // SAFETY: callback has the exact required ABI; it captures no borrowed pointers.
    unsafe {
        EnumWindows(Some(visit), 0);
    }
}
