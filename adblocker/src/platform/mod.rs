use anyhow::Result;

#[cfg(target_os = "windows")]
pub type NativeWindowHandle = windows_sys::Win32::Foundation::HWND;

#[cfg(not(target_os = "windows"))]
pub type NativeWindowHandle = isize;

pub trait Platform: Send + Sync {
    fn is_target_running(&self, process_name: &str) -> bool;
    fn list_running_processes(&self) -> Result<Vec<String>>;
    fn find_ad_window(&self, process_name: &str) -> Result<Option<NativeWindowHandle>>;
    fn hide_ad(&self, handle: NativeWindowHandle) -> Result<()>;
    fn show_ad(&self, handle: NativeWindowHandle) -> Result<()>;
}

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
#[allow(unused_imports)]
pub use windows::WindowsPlatform as NativePlatform;

#[cfg(target_os = "windows")]
#[allow(unused_imports)]
pub use windows::{
    hide_main_window_to_tray, init_tray_icon, set_tray_service_active,
    set_tray_toggle_handler,
    // Windows 서비스 관리 (SCM)
    WIN_SERVICE_NAME,
    WinServiceState,
    install_win_service,
    uninstall_win_service,
    start_win_service,
    stop_win_service,
    query_win_service_state,
    run_as_windows_service,
    // 작업 스케줄러 관리
    TASK_NAME,
    TaskState,
    install_task,
    uninstall_task,
    run_task_now,
    query_task_state,
};
