//! 창·프로세스 열거.
//!
//! `EnumWindows`로 타겟 프로세스의 최상위 창을 찾고, 그 자식 중
//! WebView 계열 클래스를 광고 창으로 판정한다.
//!
//! 프로세스 목록도 창 열거 기반이므로 **창이 없는 프로세스는 나오지 않는다**.

use std::collections::BTreeSet;
use std::path::Path;

use windows_sys::Win32::{
    Foundation::{CloseHandle, BOOL, HANDLE, HWND, LPARAM},
    System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    },
    UI::WindowsAndMessaging::{
        EnumChildWindows, EnumWindows, GetClassNameW, GetWindowThreadProcessId,
    },
};

pub(super) struct TopLevelSearchContext {
    pub(super) process_name_lower: String,
    pub(super) found_child: Option<HWND>,
}

struct ChildSearchContext {
    found_child: Option<HWND>,
}

struct RunningProcessContext {
    names: BTreeSet<String>,
}

pub(super) unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let context = &mut *(lparam as *mut TopLevelSearchContext);

    let Some(process_name) = process_name_from_hwnd(hwnd) else {
        return 1;
    };

    if !process_name.eq_ignore_ascii_case(&context.process_name_lower) {
        return 1;
    }

    let mut child_context = ChildSearchContext { found_child: None };
    let child_param = &mut child_context as *mut ChildSearchContext as LPARAM;

    EnumChildWindows(hwnd, Some(enum_child_proc), child_param);

    if let Some(child) = child_context.found_child {
        context.found_child = Some(child);
        return 0;
    }

    1
}

unsafe extern "system" fn enum_child_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let context = &mut *(lparam as *mut ChildSearchContext);

    let class_name = class_name_from_hwnd(hwnd);
    let class_name_lower = class_name.to_ascii_lowercase();

    let is_webview = class_name_lower.contains("chrome_widgetwin_1")
        || class_name_lower.contains("webview");

    if is_webview {
        context.found_child = Some(hwnd);
        return 0;
    }

    1
}

unsafe extern "system" fn enum_running_processes_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let context = &mut *(lparam as *mut RunningProcessContext);

    if let Some(process_name) = process_name_from_hwnd(hwnd) {
        if !process_name.is_empty() {
            context.names.insert(process_name);
        }
    }

    1
}

pub(super) fn list_running_window_process_names() -> Vec<String> {
    let mut context = RunningProcessContext {
        names: BTreeSet::new(),
    };
    let lparam = &mut context as *mut RunningProcessContext as LPARAM;

    // SAFETY: callback and context pointer are valid for the duration of EnumWindows.
    unsafe {
        EnumWindows(Some(enum_running_processes_proc), lparam);
    }

    context.names.into_iter().collect()
}

pub(super) fn class_name_from_hwnd(hwnd: HWND) -> String {
    let mut buffer = vec![0u16; 256];
    // SAFETY: The buffer is valid for writes and hwnd is provided by the window enumeration API.
    let len = unsafe { GetClassNameW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };

    if len <= 0 {
        return String::new();
    }

    String::from_utf16_lossy(&buffer[..len as usize])
}

fn process_name_from_hwnd(hwnd: HWND) -> Option<String> {
    let mut process_id = 0u32;

    // SAFETY: process_id points to valid memory and hwnd comes from Win32 APIs.
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut process_id as *mut u32);
    }

    if process_id == 0 {
        return None;
    }

    process_name_from_pid(process_id)
}

pub(super) fn process_name_from_pid(process_id: u32) -> Option<String> {
    // SAFETY: OpenProcess is called with query-only rights and returns null on failure.
    let process_handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            process_id,
        )
    };

    if process_handle == 0 {
        return None;
    }

    let process_name = query_process_image_name(process_handle);

    // SAFETY: handle was obtained from OpenProcess.
    unsafe {
        CloseHandle(process_handle);
    }

    process_name
}

fn query_process_image_name(process_handle: HANDLE) -> Option<String> {
    let mut buffer = vec![0u16; 512];
    let mut size = buffer.len() as u32;

    // SAFETY: The buffer and size pointers are valid and process_handle is a live process handle.
    let ok = unsafe {
        QueryFullProcessImageNameW(
            process_handle,
            0,
            buffer.as_mut_ptr(),
            &mut size as *mut u32,
        )
    };

    if ok == 0 || size == 0 {
        return None;
    }

    let full_path = String::from_utf16_lossy(&buffer[..size as usize]);
    let file_name = Path::new(&full_path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase());

    file_name
}
