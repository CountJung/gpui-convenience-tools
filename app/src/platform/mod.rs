use anyhow::Result;

#[cfg(target_os = "windows")]
pub type NativeWindowHandle = windows_sys::Win32::Foundation::HWND;

#[cfg(not(target_os = "windows"))]
pub type NativeWindowHandle = isize;

/// 광고 후보 창을 조작하기 전 저장하는 원래 표시 상태.
///
/// 핸들은 프로세스가 종료된 뒤 재사용될 수 있으므로 복원 시 `process_id`도 함께
/// 확인한다. 위치와 크기는 가상 화면 기준 좌표로 저장한다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AdWindowSnapshot {
    pub(crate) handle: NativeWindowHandle,
    pub(crate) process_id: u32,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) show_state: AdWindowShowState,
    pub(crate) enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AdWindowShowState {
    Hidden,
    Normal,
    Minimized,
    Maximized,
}

// ─────────────────────────────────────────────
// B-1: 시스템 서비스 정보 구조체
// ─────────────────────────────────────────────

/// SCM에서 조회한 시스템 서비스의 요약 정보.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub struct SysServiceInfo {
    pub name: String,
    pub display_name: String,
    pub status: SysServiceStatus,
    pub start_type: SysServiceStartType,
}

/// 시스템 서비스 실행 상태.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub enum SysServiceStatus {
    Running,
    Stopped,
    Paused,
    StartPending,
    StopPending,
    Unknown,
}

impl std::fmt::Display for SysServiceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "Running"),
            Self::Stopped => write!(f, "Stopped"),
            Self::Paused => write!(f, "Paused"),
            Self::StartPending => write!(f, "Starting..."),
            Self::StopPending => write!(f, "Stopping..."),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// 시스템 서비스 시작 유형.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub enum SysServiceStartType {
    Automatic,
    Manual,
    Disabled,
    Unknown,
}

impl std::fmt::Display for SysServiceStartType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Automatic => write!(f, "Automatic"),
            Self::Manual => write!(f, "Manual"),
            Self::Disabled => write!(f, "Disabled"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

#[allow(dead_code)]
pub trait Platform: Send + Sync {
    fn is_target_running(&self, process_name: &str) -> bool;
    fn list_running_processes(&self) -> Result<Vec<String>>;
    fn find_ad_window(
        &self,
        process_name: &str,
        ad_window_class: &str,
    ) -> Result<Option<NativeWindowHandle>>;
    fn hide_ad(&self, handle: NativeWindowHandle) -> Result<()>;
    fn show_ad(&self, handle: NativeWindowHandle) -> Result<()>;

    /// 광고 창을 활성화하지 않은 채 화면 크기를 0×0으로 축소하고 입력을 막는다.
    fn collapse_ad(&self, _handle: NativeWindowHandle) -> Result<()> {
        Err(anyhow::anyhow!("광고 창 0×0 축소는 지원되지 않습니다."))
    }

    /// 저장된 창이 속한 프로세스가 아직 실행 중인지 확인한다.
    fn is_process_id_running(&self, _process_id: u32) -> bool {
        false
    }

    /// 광고 창을 숨기기 전 원래 상태를 캡처한다.
    fn capture_ad_window_state(&self, _handle: NativeWindowHandle) -> Result<AdWindowSnapshot> {
        Err(anyhow::anyhow!("광고 창 상태 캡처는 지원되지 않습니다."))
    }

    /// 이전에 캡처한 광고 창 상태를 복원한다.
    fn restore_ad_window_state(&self, _snapshot: &AdWindowSnapshot) -> Result<()> {
        Err(anyhow::anyhow!("광고 창 상태 복원은 지원되지 않습니다."))
    }

    // ─── B-1: 시스템 서비스 관리 ───

    /// 설치된 Win32 서비스 목록을 반환한다. 비Windows는 빈 목록.
    fn list_sys_services(&self) -> Result<Vec<SysServiceInfo>> {
        Ok(vec![])
    }

    /// 지정한 서비스를 시작한다. 관리자 권한 필요.
    fn start_sys_service(&self, _name: &str) -> Result<()> {
        Err(anyhow::anyhow!("not supported on this platform"))
    }

    /// 지정한 서비스를 중지한다. 관리자 권한 필요.
    fn stop_sys_service(&self, _name: &str) -> Result<()> {
        Err(anyhow::anyhow!("not supported on this platform"))
    }

    /// 지정한 서비스의 전체 정보를 조회한다.
    fn query_sys_service(&self, _name: &str) -> Result<SysServiceInfo> {
        Err(anyhow::anyhow!("not supported on this platform"))
    }

    /// 지정한 서비스를 삭제(영구 제거)한다. 관리자 권한 필요.
    fn delete_sys_service(&self, _name: &str) -> Result<()> {
        Err(anyhow::anyhow!("not supported on this platform"))
    }

    /// 현재 프로세스가 관리자 권한으로 실행 중이면 `true`.
    fn is_elevated(&self) -> bool {
        false
    }
}

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(not(target_os = "windows"))]
pub mod fallback;

#[cfg(target_os = "windows")]
#[allow(unused_imports)]
pub use windows::WindowsPlatform as NativePlatform;

#[cfg(not(target_os = "windows"))]
#[allow(unused_imports)]
pub use fallback::FallbackPlatform as NativePlatform;

#[cfg(target_os = "windows")]
#[allow(unused_imports)]
pub use windows::{
    hide_main_window_to_tray, init_tray_icon, is_elevated, set_tray_service_active,
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
