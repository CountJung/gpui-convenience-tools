//! 창·프로세스 열거.
//!
//! `EnumWindows`로 타겟 프로세스의 최상위 창을 찾고, 명시된 WebView 계열 클래스와
//! 소유자/도구 창 특성을 가진 팝업만 광고 창 후보로 판정한다.
//!
//! 프로세스 목록도 창 열거 기반이므로 **창이 없는 프로세스는 나오지 않는다**.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{anyhow, Result};
use windows_sys::Win32::{
    Foundation::{CloseHandle, BOOL, HANDLE, HWND, LPARAM, RECT},
    System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    },
    UI::Input::KeyboardAndMouse::{EnableWindow, IsWindowEnabled},
    UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindow, GetWindowLongPtrW, GetWindowRect,
        GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible, IsZoomed, SetWindowPos,
        ShowWindow, GWL_EXSTYLE, GW_OWNER, SW_HIDE,
        SW_SHOWMAXIMIZED, SW_SHOWMINNOACTIVE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE,
        SWP_NOZORDER, WS_EX_TOOLWINDOW,
    },
};

use crate::platform::{AdWindowShowState, AdWindowSnapshot};

pub(super) struct TopLevelSearchContext {
    pub(super) process_name_lower: String,
    pub(super) class_filter: String,
    pub(super) found_windows: Vec<HWND>,
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

    context.found_windows.push(hwnd);
    1
}

/// 타겟 프로세스에서 안전한 광고 창 후보를 하나 찾는다.
///
/// 메인 창의 자식 WebView는 같은 클래스명을 공유할 수 있으므로 이 단계에서는
/// `EnumChildWindows`로 내려가지 않는다. 소유자 창이 있거나 도구 창으로 표시된
/// 최상위 팝업만 후보로 반환한다.
pub(super) fn find_ad_window(process_name: &str, class_filter: &str) -> Option<HWND> {
    find_ad_windows(process_name, class_filter).into_iter().next()
}

pub(super) fn find_ad_windows(process_name: &str, class_filter: &str) -> Vec<HWND> {
    let mut context = TopLevelSearchContext {
        process_name_lower: process_name.to_ascii_lowercase(),
        class_filter: class_filter.trim().to_string(),
        found_windows: Vec::new(),
    };
    let lparam = &mut context as *mut TopLevelSearchContext as LPARAM;

    // SAFETY: callback and context pointer are valid for the duration of EnumWindows.
    unsafe {
        EnumWindows(Some(enum_windows_proc), lparam);
    }

    context.found_windows
}

/// 창을 숨기기 전 위치·크기·표시 상태를 읽기 전용으로 캡처한다.
pub(super) fn capture_ad_window_state(hwnd: HWND) -> Result<AdWindowSnapshot> {
    if unsafe { IsWindow(hwnd) } == 0 {
        return Err(anyhow!("invalid HWND for capture_ad_window_state"));
    }

    let mut process_id = 0u32;
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };

    // SAFETY: hwnd is validated above and the output pointers refer to local values.
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut process_id as *mut u32);
        if process_id == 0 || GetWindowRect(hwnd, &mut rect as *mut RECT) == 0 {
            return Err(anyhow!("failed to read the target window state"));
        }
    }

    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 {
        return Err(anyhow!("target window has an invalid size: {width}x{height}"));
    }

    let show_state = if unsafe { IsWindowVisible(hwnd) } == 0 {
        AdWindowShowState::Hidden
    } else if unsafe { IsIconic(hwnd) } != 0 {
        AdWindowShowState::Minimized
    } else if unsafe { IsZoomed(hwnd) } != 0 {
        AdWindowShowState::Maximized
    } else {
        AdWindowShowState::Normal
    };

    Ok(AdWindowSnapshot {
        handle: hwnd,
        process_id,
        x: rect.left,
        y: rect.top,
        width,
        height,
        show_state,
        enabled: unsafe { IsWindowEnabled(hwnd) != 0 },
    })
}

pub(super) fn is_window_alive(hwnd: HWND) -> bool {
    unsafe { IsWindow(hwnd) != 0 }
}

pub(super) fn window_process_id(hwnd: HWND) -> Result<u32> {
    if unsafe { IsWindow(hwnd) } == 0 {
        return Err(anyhow!("invalid HWND for process ID query"));
    }

    let mut process_id = 0u32;
    // SAFETY: hwnd is valid and process_id points to writable local storage.
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut process_id as *mut u32);
    }
    if process_id == 0 {
        return Err(anyhow!("window process ID is unavailable"));
    }

    Ok(process_id)
}

pub(super) fn is_window_collapsed(hwnd: HWND) -> Result<bool> {
    if unsafe { IsWindow(hwnd) } == 0 {
        return Err(anyhow!("invalid HWND for collapse state query"));
    }

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: hwnd is valid and rect points to writable local storage.
    if unsafe { GetWindowRect(hwnd, &mut rect as *mut RECT) } == 0 {
        return Err(anyhow!("GetWindowRect failed while checking collapse state"));
    }

    Ok(rect.right == rect.left
        && rect.bottom == rect.top
        && unsafe { IsWindowEnabled(hwnd) } == 0)
}

/// 광고 후보 창을 0×0으로 축소하고 입력 대상에서 제외한다.
pub(super) fn collapse_ad_window(hwnd: HWND) -> Result<()> {
    if unsafe { IsWindow(hwnd) } == 0 {
        return Err(anyhow!("invalid HWND for collapse_ad_window"));
    }

    // SAFETY: the verified HWND remains valid for this synchronous Win32 call. NOMOVE keeps
    // the saved screen position while NOACTIVATE/NOZORDER avoid focus and stacking changes.
    if unsafe {
        SetWindowPos(
            hwnd,
            0,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOACTIVATE | SWP_NOZORDER,
        )
    } == 0
    {
        return Err(anyhow!("SetWindowPos failed while collapsing the ad window"));
    }

    // SAFETY: disabling a verified window is synchronous and does not transfer ownership.
    unsafe {
        EnableWindow(hwnd, 0);
    }
    if unsafe { IsWindowEnabled(hwnd) } != 0 {
        return Err(anyhow!("EnableWindow failed while collapsing the ad window"));
    }

    Ok(())
}

/// 캡처한 창 상태를 같은 프로세스에 속한 동일 HWND에만 복원한다.
pub(super) fn restore_ad_window_state(snapshot: &AdWindowSnapshot) -> Result<()> {
    let hwnd = snapshot.handle;
    if unsafe { IsWindow(hwnd) } == 0 {
        return Err(anyhow!("saved HWND is no longer valid"));
    }

    let mut process_id = 0u32;
    // SAFETY: hwnd was validated and process_id points to writable local storage.
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut process_id as *mut u32);
    }
    if process_id != snapshot.process_id {
        return Err(anyhow!(
            "saved HWND belongs to a different process: expected {}, got {}",
            snapshot.process_id,
            process_id
        ));
    }

    // SAFETY: HWND and dimensions were captured from Win32, and flags prevent focus/z-order
    // changes while restoring the user's original geometry.
    if unsafe {
        SetWindowPos(
            hwnd,
            0,
            snapshot.x,
            snapshot.y,
            snapshot.width,
            snapshot.height,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )
    } == 0
    {
        return Err(anyhow!("SetWindowPos failed while restoring the saved window state"));
    }

    let show_command = match snapshot.show_state {
        AdWindowShowState::Hidden => SW_HIDE,
        AdWindowShowState::Normal => SW_SHOWNOACTIVATE,
        AdWindowShowState::Minimized => SW_SHOWMINNOACTIVE,
        AdWindowShowState::Maximized => SW_SHOWMAXIMIZED,
    };

    // SAFETY: HWND and command are validated Win32 values.
    unsafe {
        ShowWindow(hwnd, show_command);
        EnableWindow(hwnd, if snapshot.enabled { 1 } else { 0 });
    }

    if unsafe { IsWindowEnabled(hwnd) != 0 } != snapshot.enabled {
        return Err(anyhow!("EnableWindow failed while restoring the saved window state"));
    }

    Ok(())
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
    window_process_id(hwnd)
        .ok()
        .and_then(process_name_from_pid)
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

pub(super) fn is_process_id_running(process_id: u32) -> bool {
    // SAFETY: OpenProcess is called with query-only rights and returns null on failure.
    let process_handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
    if process_handle == 0 {
        return false;
    }

    // SAFETY: handle was obtained from OpenProcess.
    unsafe {
        CloseHandle(process_handle);
    }
    true
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
    use super::{
        capture_ad_window_state, class_filter_matches, collapse_ad_window, find_ad_windows,
        is_window_collapsed, process_name_from_pid, restore_ad_window_state, window_process_id,
    };
    use crate::platform::{AdWindowShowState, AdWindowSnapshot};

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

    #[test]
    fn invalid_window_state_cannot_be_captured() {
        assert!(capture_ad_window_state(0).is_err());
    }

    #[test]
    fn invalid_window_state_cannot_be_restored() {
        let snapshot = AdWindowSnapshot {
            handle: 0,
            process_id: 26440,
            x: 10,
            y: 20,
            width: 300,
            height: 200,
            show_state: AdWindowShowState::Normal,
            enabled: true,
        };

        assert!(restore_ad_window_state(&snapshot).is_err());
    }

    #[test]
    fn invalid_window_cannot_be_collapsed() {
        assert!(collapse_ad_window(0).is_err());
    }

    #[test]
    fn invalid_window_state_queries_fail_closed() {
        assert!(is_window_collapsed(0).is_err());
        assert!(window_process_id(0).is_err());
    }

    #[test]
    fn collapse_and_restore_preserve_a_fixture_window_state() {
        use std::ptr::null;
        use windows_sys::Win32::{
            Foundation::{RECT, HWND},
            UI::{
                Input::KeyboardAndMouse::{IsWindowEnabled},
                WindowsAndMessaging::{
                    CreateWindowExW, DestroyWindow, GetWindowRect, IsWindowVisible, WS_POPUP,
                    WS_VISIBLE,
                },
            },
        };

        let class_name = super::super::wide_null("STATIC");
        let title = super::super::wide_null("gpui-ad-003-fixture");
        // The fixture is visible but placed off-screen so this test does not cover the user's
        // windows. It is created and destroyed entirely within this test.
        let hwnd: HWND = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_POPUP | WS_VISIBLE,
                -32000,
                -32000,
                120,
                80,
                0,
                0,
                0,
                null(),
            )
        };
        assert_ne!(hwnd, 0, "fixture window should be created");

        let snapshot = capture_ad_window_state(hwnd).expect("fixture state should be captured");
        assert_eq!(snapshot.width, 120);
        assert_eq!(snapshot.height, 80);
        assert!(snapshot.enabled);

        collapse_ad_window(hwnd).expect("fixture should collapse");
        let mut collapsed = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        assert_ne!(unsafe { GetWindowRect(hwnd, &mut collapsed) }, 0);
        assert_eq!(collapsed.right - collapsed.left, 0);
        assert_eq!(collapsed.bottom - collapsed.top, 0);
        assert_eq!(unsafe { IsWindowEnabled(hwnd) }, 0);

        restore_ad_window_state(&snapshot).expect("fixture state should be restored");
        let mut restored = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        assert_ne!(unsafe { GetWindowRect(hwnd, &mut restored) }, 0);
        assert_eq!(restored.right - restored.left, snapshot.width);
        assert_eq!(restored.bottom - restored.top, snapshot.height);
        assert_ne!(unsafe { IsWindowVisible(hwnd) }, 0);
        assert_ne!(unsafe { IsWindowEnabled(hwnd) }, 0);

        unsafe {
            DestroyWindow(hwnd);
        }
    }

    #[test]
    fn finds_all_matching_tool_window_fixtures() {
        use std::ptr::null;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE,
        };

        let class_name = super::super::wide_null("STATIC");
        let title_a = super::super::wide_null("gpui-ad-004-fixture-a");
        let title_b = super::super::wide_null("gpui-ad-004-fixture-b");
        let create = |title: &[u16]| unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_POPUP | WS_VISIBLE,
                -32000,
                -32000,
                80,
                60,
                0,
                0,
                0,
                null(),
            )
        };
        let first = create(&title_a);
        let second = create(&title_b);
        assert_ne!(first, 0, "first fixture window should be created");
        assert_ne!(second, 0, "second fixture window should be created");

        let process_name = process_name_from_pid(std::process::id())
            .expect("the current test process should have an image name");
        let found = find_ad_windows(&process_name, "STATIC");
        assert!(found.contains(&first));
        assert!(found.contains(&second));

        unsafe {
            DestroyWindow(first);
            DestroyWindow(second);
        }
    }
}
