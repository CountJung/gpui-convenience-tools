//! Windows 플랫폼 구현.
//!
//! Win32 종속 코드를 책임별로 나눠 담는다.
//!
//! | 모듈 | 책임 |
//! | --- | --- |
//! | [`window_ops`] | 창·프로세스 열거와 광고 창 0×0 축소 |
//! | [`tray`] | 시스템 트레이 아이콘 |
//! | [`scm`] | Windows 서비스 등록·서비스 모드 실행 |
//! | [`task_scheduler`] | 로그온 시 자동 시작(`schtasks`) |
//! | [`services`] | 설치된 서비스 조회·제어 |

mod scm;
mod services;
mod task_scheduler;
mod tray;
mod window_ops;

pub use scm::{
    install_win_service, query_win_service_state, run_as_windows_service, start_win_service,
    stop_win_service, uninstall_win_service, WinServiceState, WIN_SERVICE_NAME,
};
pub use services::is_elevated;
pub use task_scheduler::{
    install_task, query_task_state, run_task_now, uninstall_task, TaskState, TASK_NAME,
};
pub use tray::{
    hide_main_window_to_tray, init_tray_icon, set_tray_service_active, set_tray_toggle_handler,
};

use anyhow::{anyhow, Result};

use windows_sys::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{IsWindow, ShowWindow, SW_HIDE, SW_SHOW},
};

use services::{
    delete_sys_service_impl, list_sys_services_impl, query_sys_service_impl, start_sys_service_impl,
    stop_sys_service_impl,
};
use window_ops::{is_process_id_running, list_running_window_process_names};

use crate::platform::{AdWindowSnapshot, Platform};

/// Rust 문자열을 null-terminated UTF-16 버퍼로 변환한다. Win32 W계열 API 인자용.
pub(super) fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

#[derive(Default)]
pub struct WindowsPlatform;

impl WindowsPlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Platform for WindowsPlatform {
    fn is_target_running(&self, process_name: &str) -> bool {
        list_running_window_process_names()
            .iter()
            .any(|running| running.eq_ignore_ascii_case(process_name))
    }

    fn list_running_processes(&self) -> Result<Vec<String>> {
        Ok(list_running_window_process_names())
    }

    fn find_ad_window(&self, process_name: &str, ad_window_class: &str) -> Result<Option<HWND>> {
        Ok(window_ops::find_ad_window(process_name, ad_window_class))
    }

    fn find_ad_windows(&self, process_name: &str, ad_window_class: &str) -> Result<Vec<HWND>> {
        Ok(window_ops::find_ad_windows(process_name, ad_window_class))
    }

    fn hide_ad(&self, handle: HWND) -> Result<()> {
        // SAFETY: IsWindow only reads window metadata.
        if unsafe { IsWindow(handle) } == 0 {
            return Err(anyhow!("invalid HWND for hide_ad"));
        }

        // SAFETY: ShowWindow is called with a verified window handle.
        unsafe {
            ShowWindow(handle, SW_HIDE);
        }

        Ok(())
    }

    fn show_ad(&self, handle: HWND) -> Result<()> {
        // SAFETY: IsWindow only reads window metadata.
        if unsafe { IsWindow(handle) } == 0 {
            return Err(anyhow!("invalid HWND for show_ad"));
        }

        // SAFETY: ShowWindow is called with a verified window handle.
        unsafe {
            ShowWindow(handle, SW_SHOW);
        }

        Ok(())
    }

    fn collapse_ad(&self, handle: HWND) -> Result<()> {
        window_ops::collapse_ad_window(handle)
    }

    fn is_ad_window_alive(&self, handle: HWND) -> bool {
        window_ops::is_window_alive(handle)
    }

    fn ad_window_process_id(&self, handle: HWND) -> Result<u32> {
        window_ops::window_process_id(handle)
    }

    fn is_ad_window_collapsed(&self, handle: HWND) -> Result<bool> {
        window_ops::is_window_collapsed(handle)
    }

    fn is_process_id_running(&self, process_id: u32) -> bool {
        is_process_id_running(process_id)
    }

    fn capture_ad_window_state(&self, handle: HWND) -> Result<AdWindowSnapshot> {
        window_ops::capture_ad_window_state(handle)
    }

    fn restore_ad_window_state(&self, snapshot: &AdWindowSnapshot) -> Result<()> {
        window_ops::restore_ad_window_state(snapshot)
    }

    fn list_sys_services(&self) -> Result<Vec<crate::platform::SysServiceInfo>> {
        list_sys_services_impl()
    }

    fn start_sys_service(&self, name: &str) -> Result<()> {
        start_sys_service_impl(name)
    }

    fn stop_sys_service(&self, name: &str) -> Result<()> {
        stop_sys_service_impl(name)
    }

    fn query_sys_service(&self, name: &str) -> Result<crate::platform::SysServiceInfo> {
        query_sys_service_impl(name)
    }

    fn delete_sys_service(&self, name: &str) -> Result<()> {
        delete_sys_service_impl(name)
    }

    fn is_elevated(&self) -> bool {
        is_elevated()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use window_ops::process_name_from_pid;

    #[test]
    #[ignore = "Requires KakaoTalk process availability on the host machine."]
    fn kakao_talk_process_presence_check() {
        let platform = WindowsPlatform::new();
        let _ = platform.is_target_running("KakaoTalk.exe");
    }

    #[test]
    fn current_process_name_query_runs() {
        let current_pid = std::process::id();
        let process_name = process_name_from_pid(current_pid);
        assert!(process_name.is_some());
    }
}
