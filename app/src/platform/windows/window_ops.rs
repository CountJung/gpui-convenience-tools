//! 창·프로세스 열거.
//!
//! `EnumWindows`로 타겟 프로세스의 최상위 창을 찾고, 명시된 WebView 계열 클래스와
//! 소유자/도구 창 특성을 가진 팝업만 광고 창 후보로 판정한다.
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
        EnumWindows, GetClassNameW, GetWindow, GetWindowLongPtrW, GetWindowThreadProcessId,
        IsWindowVisible, GWL_EXSTYLE, GW_OWNER, WS_EX_TOOLWINDOW,
    },
};

pub(super) struct TopLevelSearchContext {
    pub(super) process_name_lower: String,
    pub(super) class_filter: String,
    pub(super) found_window: Option<HWND>,
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

    if !is_ad_window_candidate(hwnd, &context.class_filter) {
        return 1;
    }

    context.found_window = Some(hwnd);
    0
}

/// 타겟 프로세스에서 안전한 광고 창 후보를 하나 찾는다.
///
/// 메인 창의 자식 WebView는 같은 클래스명을 공유할 수 있으므로 이 단계에서는
/// `EnumChildWindows`로 내려가지 않는다. 소유자 창이 있거나 도구 창으로 표시된
/// 최상위 팝업만 후보로 반환한다.
pub(super) fn find_ad_window(process_name: &str, class_filter: &str) -> Option<HWND> {
    let mut context = TopLevelSearchContext {
        process_name_lower: process_name.to_ascii_lowercase(),
        class_filter: class_filter.trim().to_string(),
        found_window: None,
    };
    let lparam = &mut context as *mut TopLevelSearchContext as LPARAM;

    // SAFETY: callback and context pointer are valid for the duration of EnumWindows.
    unsafe {
        EnumWindows(Some(enum_windows_proc), lparam);
    }

    context.found_window
}

fn is_ad_window_candidate(hwnd: HWND, class_filter: &str) -> bool {
    // Invisible windows are not useful for the visual action and may be the app's
    // hidden startup/helper window.
    if unsafe { IsWindowVisible(hwnd) } == 0 {
        return false;
    }

    let class_name = class_name_from_hwnd(hwnd);
    if !class_filter_matches(&class_name, class_filter) {
        return false;
    }

    // An unowned, non-tool top-level window is treated as the app's main window.
    // This intentionally favors safety over catching every possible popup until
    // a target-specific selector is available.
    let owner = unsafe { GetWindow(hwnd, GW_OWNER) };
    let ex_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 };
    owner != 0 || ex_style & WS_EX_TOOLWINDOW != 0
}

fn class_filter_matches(class_name: &str, class_filter: &str) -> bool {
    if class_filter.eq_ignore_ascii_case("auto:webview") {
        let class_name_lower = class_name.to_ascii_lowercase();
        return class_name_lower.contains("chrome_widgetwin_1")
            || class_name_lower.contains("webview");
    }

    !class_filter.is_empty() && class_name.eq_ignore_ascii_case(class_filter)
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
    let process_handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };

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

#[cfg(test)]
mod tests {
    use super::class_filter_matches;

    #[test]
    fn explicit_class_filter_is_case_insensitive() {
        assert!(class_filter_matches(
            "Chrome_WidgetWin_1",
            "chrome_widgetwin_1"
        ));
        assert!(!class_filter_matches("Chrome_WidgetWin_1", "OtherWindow"));
    }

    #[test]
    fn auto_webview_filter_matches_known_webview_classes() {
        assert!(class_filter_matches("Chrome_WidgetWin_1", "auto:webview"));
        assert!(class_filter_matches("WebView2Child", "AUTO:WEBVIEW"));
        assert!(!class_filter_matches("MainWindow", "auto:webview"));
    }

    #[test]
    fn empty_class_filter_does_not_match_every_window() {
        assert!(!class_filter_matches("Chrome_WidgetWin_1", ""));
    }
}
