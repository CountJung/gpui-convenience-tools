use anyhow::{anyhow, Result};

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

/// 콘솔 창 없이 자식 프로세스 생성
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

use windows_sys::Win32::{
    Foundation::{CloseHandle, BOOL, HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM},
    System::LibraryLoader::GetModuleHandleW,
    System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    },
    UI::Shell::{
        IsUserAnAdmin,
        NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
    },
    UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
        DestroyWindow,
        DispatchMessageW, EnumChildWindows,
        EnumWindows, FindWindowW, GetClassNameW, GetMessageW, GetWindowThreadProcessId,
        GetCursorPos, IDI_APPLICATION, IsWindow, LoadIconW, MF_STRING, MSG, PostMessageW,
        PostQuitMessage, RegisterClassW, SW_HIDE, SW_RESTORE, SW_SHOW, SetForegroundWindow,
        ShowWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WM_APP,
        WM_CLOSE, WM_DESTROY, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_RBUTTONUP, WNDCLASSW,
    },
};

use crate::platform::Platform;

const TRAY_MESSAGE_ID: u32 = WM_APP + 1;
const TRAY_ICON_UID: u32 = 1;
const MAIN_WINDOW_TITLE: &str = "gpui-convenience-tools";
const TRAY_MENU_OPEN: usize = 1001;
const TRAY_MENU_TOGGLE: usize = 1002;
const TRAY_MENU_EXIT: usize = 1003;

static TRAY_INITIALIZED: OnceLock<()> = OnceLock::new();
static TRAY_TOGGLE_HANDLER: OnceLock<std::sync::Arc<dyn Fn(bool) + Send + Sync>> =
    OnceLock::new();
static TRAY_SERVICE_ACTIVE: AtomicBool = AtomicBool::new(true);

pub fn set_tray_toggle_handler(handler: std::sync::Arc<dyn Fn(bool) + Send + Sync>) {
    let _ = TRAY_TOGGLE_HANDLER.set(handler);
}

pub fn set_tray_service_active(active: bool) {
    TRAY_SERVICE_ACTIVE.store(active, Ordering::Relaxed);
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn with_tip(mut data: [u16; 128], tip: &str) -> [u16; 128] {
    let encoded = wide_null(tip);
    let max = data.len().saturating_sub(1);
    let len = encoded.len().min(max);
    data[..len].copy_from_slice(&encoded[..len]);
    data
}

fn try_find_main_window() -> Option<HWND> {
    let title = wide_null(MAIN_WINDOW_TITLE);
    // SAFETY: String pointer is a valid null-terminated UTF-16 title.
    let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
    if hwnd == 0 {
        None
    } else {
        Some(hwnd)
    }
}

fn restore_main_window() {
    if let Some(main_hwnd) = try_find_main_window() {
        // SAFETY: main_hwnd is validated by FindWindowW and used with standard show/activate APIs.
        unsafe {
            ShowWindow(main_hwnd, SW_SHOW);
            ShowWindow(main_hwnd, SW_RESTORE);
            SetForegroundWindow(main_hwnd);
        }
    }
}

fn request_app_exit() {
    if let Some(main_hwnd) = try_find_main_window() {
        // SAFETY: Posting WM_CLOSE to the main window requests a normal app shutdown.
        unsafe {
            let _ = PostMessageW(main_hwnd, WM_CLOSE, 0, 0);
        }
    } else {
        std::process::exit(0);
    }
}

fn show_tray_menu(hwnd: HWND) {
    // SAFETY: Menu and cursor APIs are called with valid parameters and cleaned up on all paths.
    unsafe {
        let menu = CreatePopupMenu();
        if menu == 0 {
            return;
        }

        let open = wide_null("Open");
        let toggle = if TRAY_SERVICE_ACTIVE.load(Ordering::Relaxed) {
            wide_null("Disable Blocking")
        } else {
            wide_null("Enable Blocking")
        };
        let exit = wide_null("Exit");

        let _ = AppendMenuW(menu, MF_STRING, TRAY_MENU_OPEN, open.as_ptr());
        let _ = AppendMenuW(menu, MF_STRING, TRAY_MENU_TOGGLE, toggle.as_ptr());
        let _ = AppendMenuW(menu, MF_STRING, TRAY_MENU_EXIT, exit.as_ptr());

        let mut pt = std::mem::zeroed::<POINT>();
        if GetCursorPos(&mut pt) == 0 {
            let _ = DestroyMenu(menu);
            return;
        }

        let _ = SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            pt.x,
            pt.y,
            0,
            hwnd,
            std::ptr::null(),
        );

        let _ = DestroyMenu(menu);

        match cmd as usize {
            TRAY_MENU_OPEN => restore_main_window(),
            TRAY_MENU_TOGGLE => {
                let next = !TRAY_SERVICE_ACTIVE.load(Ordering::Relaxed);
                TRAY_SERVICE_ACTIVE.store(next, Ordering::Relaxed);
                if let Some(handler) = TRAY_TOGGLE_HANDLER.get() {
                    handler(next);
                }
            }
            TRAY_MENU_EXIT => request_app_exit(),
            _ => {}
        }
    }
}

extern "system" fn tray_wnd_proc(
    hwnd: HWND,
    msg: u32,
    _wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == TRAY_MESSAGE_ID {
        let mouse_msg = lparam as u32;
        if mouse_msg == WM_LBUTTONUP || mouse_msg == WM_LBUTTONDBLCLK {
            restore_main_window();
            return 0;
        }

        if mouse_msg == WM_RBUTTONUP {
            show_tray_menu(hwnd);
            return 0;
        }
    }

    if msg == WM_DESTROY {
        // SAFETY: Structure is zero-initialized and required fields are populated before API call.
        unsafe {
            let mut nid = std::mem::zeroed::<NOTIFYICONDATAW>();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = TRAY_ICON_UID;
            let _ = Shell_NotifyIconW(NIM_DELETE, &mut nid);
            PostQuitMessage(0);
        }
        return 0;
    }

    // SAFETY: Default window procedure is called for unhandled messages.
    unsafe { DefWindowProcW(hwnd, msg, _wparam, lparam) }
}

pub fn init_tray_icon() -> Result<()> {
    if TRAY_INITIALIZED.get().is_some() {
        return Ok(());
    }

    thread::spawn(|| {
        // SAFETY: Null module name requests current process module handle.
        let hinstance: HINSTANCE = unsafe { GetModuleHandleW(std::ptr::null()) };
        if hinstance == 0 {
            return;
        }

        let class_name = wide_null("gpui-convenience-toolsTrayClass");
        let mut wc = unsafe { std::mem::zeroed::<WNDCLASSW>() };
        wc = WNDCLASSW {
            lpfnWndProc: Some(tray_wnd_proc),
            hInstance: hinstance,
            lpszClassName: class_name.as_ptr(),
            ..wc
        };

        // SAFETY: Class descriptor and pointers are valid for this call.
        let atom = unsafe { RegisterClassW(&wc) };
        if atom == 0 {
            return;
        }

        // SAFETY: Creating a hidden helper window with the registered class.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                class_name.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                hinstance,
                std::ptr::null(),
            )
        };

        if hwnd == 0 {
            return;
        }

        // SAFETY: Application icon resource id is standard and valid.
        let hicon = unsafe { LoadIconW(0, IDI_APPLICATION) };

        // SAFETY: Structure is initialized and fields required by NIF flags are populated.
        unsafe {
            let mut nid = std::mem::zeroed::<NOTIFYICONDATAW>();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = TRAY_ICON_UID;
            nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            nid.uCallbackMessage = TRAY_MESSAGE_ID;
            nid.hIcon = hicon;
            nid.szTip = with_tip(nid.szTip, "gpui-convenience-tools");
            if Shell_NotifyIconW(NIM_ADD, &mut nid) == 0 {
                let _ = DestroyWindow(hwnd);
                return;
            }

            let _ = TRAY_INITIALIZED.set(());

            let mut msg = std::mem::zeroed::<MSG>();
            while GetMessageW(&mut msg, 0, 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            DestroyWindow(hwnd);
        }
    });

    Ok(())
}

pub fn hide_main_window_to_tray() -> Result<()> {
    let Some(hwnd) = try_find_main_window() else {
        return Err(anyhow!("main window not found"));
    };

    // SAFETY: hwnd is validated by FindWindowW and only used for visibility change.
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
    }

    Ok(())
}

#[derive(Default)]
pub struct WindowsPlatform;

impl WindowsPlatform {
    pub fn new() -> Self {
        Self
    }
}

struct TopLevelSearchContext {
    process_name_lower: String,
    found_child: Option<HWND>,
}

struct ChildSearchContext {
    found_child: Option<HWND>,
}

struct RunningProcessContext {
    names: BTreeSet<String>,
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
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

fn list_running_window_process_names() -> Vec<String> {
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

fn class_name_from_hwnd(hwnd: HWND) -> String {
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

fn process_name_from_pid(process_id: u32) -> Option<String> {
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

impl Platform for WindowsPlatform {
    fn is_target_running(&self, process_name: &str) -> bool {
        self.find_ad_window(process_name).ok().flatten().is_some()
    }

    fn list_running_processes(&self) -> Result<Vec<String>> {
        Ok(list_running_window_process_names())
    }

    fn find_ad_window(&self, process_name: &str) -> Result<Option<HWND>> {
        let mut context = TopLevelSearchContext {
            process_name_lower: process_name.to_ascii_lowercase(),
            found_child: None,
        };

        let lparam = &mut context as *mut TopLevelSearchContext as LPARAM;

        // SAFETY: callback and context pointer are valid for the duration of EnumWindows.
        unsafe {
            EnumWindows(Some(enum_windows_proc), lparam);
        }

        Ok(context.found_child)
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

// ─────────────────────────────────────────────
// Windows 서비스 관리
// ─────────────────────────────────────────────

use windows_service::{
    define_windows_service,
    service::{
        ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
        ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
    service_manager::{ServiceManager, ServiceManagerAccess},
};

pub const WIN_SERVICE_NAME: &str = "gpui-convenience-tools";

/// SCM에서 조회한 서비스 상태
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WinServiceState {
    /// 서비스가 SCM에 등록되지 않은 상태
    NotInstalled,
    Stopped,
    StartPending,
    Running,
    StopPending,
    /// 조회 실패 (권한 부족 등)
    Unknown,
}

impl std::fmt::Display for WinServiceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInstalled => write!(f, "Not Installed"),
            Self::Stopped => write!(f, "Stopped"),
            Self::StartPending => write!(f, "Starting..."),
            Self::Running => write!(f, "Running"),
            Self::StopPending => write!(f, "Stopping..."),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// SCM에 질의해 서비스 현재 상태를 반환한다.
#[allow(dead_code)]
pub fn query_win_service_state() -> WinServiceState {
    let manager = match ServiceManager::local_computer(
        None::<&OsStr>,
        ServiceManagerAccess::CONNECT,
    ) {
        Ok(m) => m,
        Err(_) => return WinServiceState::Unknown,
    };

    let service = match manager.open_service(WIN_SERVICE_NAME, ServiceAccess::QUERY_STATUS) {
        Ok(s) => s,
        Err(_) => return WinServiceState::NotInstalled,
    };

    match service.query_status() {
        Ok(status) => match status.current_state {
            ServiceState::Stopped => WinServiceState::Stopped,
            ServiceState::StartPending => WinServiceState::StartPending,
            ServiceState::Running => WinServiceState::Running,
            ServiceState::StopPending => WinServiceState::StopPending,
            _ => WinServiceState::Unknown,
        },
        Err(_) => WinServiceState::Unknown,
    }
}

/// SCM에 서비스를 설치한다. 관리자 권한이 필요하다.
#[allow(dead_code)]
pub fn install_win_service() -> Result<()> {
    let exe_path = std::env::current_exe()
        .map_err(|e| anyhow!("current_exe failed: {e}"))?;

    let manager = ServiceManager::local_computer(
        None::<&OsStr>,
        ServiceManagerAccess::CREATE_SERVICE,
    )
    .map_err(|e| anyhow!("SCM open failed: {e}"))?;

    let service_info = ServiceInfo {
        name: OsString::from(WIN_SERVICE_NAME),
        display_name: OsString::from("gpui-convenience-tools Blocker"),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe_path,
        launch_arguments: vec![OsString::from("--service")],
        dependencies: vec![],
        account_name: None,      // LocalSystem
        account_password: None,
    };

    let _service = manager
        .create_service(&service_info, ServiceAccess::empty())
        .map_err(|e| anyhow!("create_service failed: {e}"))?;

    log::info!("Windows service installed: {WIN_SERVICE_NAME}");
    Ok(())
}

/// SCM에서 서비스를 제거한다. 관리자 권한이 필요하다.
#[allow(dead_code)]
pub fn uninstall_win_service() -> Result<()> {
    let manager = ServiceManager::local_computer(
        None::<&OsStr>,
        ServiceManagerAccess::CONNECT,
    )
    .map_err(|e| anyhow!("SCM open failed: {e}"))?;

    let service = manager
        .open_service(
            WIN_SERVICE_NAME,
            ServiceAccess::STOP | ServiceAccess::DELETE | ServiceAccess::QUERY_STATUS,
        )
        .map_err(|e| anyhow!("open_service failed: {e}"))?;

    let status = service
        .query_status()
        .map_err(|e| anyhow!("query_status failed: {e}"))?;

    if status.current_state != ServiceState::Stopped {
        service
            .stop()
            .map_err(|e| anyhow!("stop failed: {e}"))?;
        // 중지 완료까지 최대 5초 대기
        for _ in 0..5 {
            std::thread::sleep(Duration::from_secs(1));
            if let Ok(s) = service.query_status() {
                if s.current_state == ServiceState::Stopped {
                    break;
                }
            }
        }
    }

    service
        .delete()
        .map_err(|e| anyhow!("delete failed: {e}"))?;

    log::info!("Windows service uninstalled: {WIN_SERVICE_NAME}");
    Ok(())
}

/// 등록된 서비스를 시작한다.
#[allow(dead_code)]
pub fn start_win_service() -> Result<()> {
    let manager = ServiceManager::local_computer(
        None::<&OsStr>,
        ServiceManagerAccess::CONNECT,
    )
    .map_err(|e| anyhow!("SCM open failed: {e}"))?;

    let service = manager
        .open_service(WIN_SERVICE_NAME, ServiceAccess::START)
        .map_err(|e| anyhow!("open_service failed: {e}"))?;

    service
        .start(&[] as &[&OsStr])
        .map_err(|e| anyhow!("start failed: {e}"))?;

    log::info!("Windows service started: {WIN_SERVICE_NAME}");
    Ok(())
}

/// 실행 중인 서비스를 중지한다.
#[allow(dead_code)]
pub fn stop_win_service() -> Result<()> {
    let manager = ServiceManager::local_computer(
        None::<&OsStr>,
        ServiceManagerAccess::CONNECT,
    )
    .map_err(|e| anyhow!("SCM open failed: {e}"))?;

    let service = manager
        .open_service(WIN_SERVICE_NAME, ServiceAccess::STOP)
        .map_err(|e| anyhow!("open_service failed: {e}"))?;

    service
        .stop()
        .map_err(|e| anyhow!("stop failed: {e}"))?;

    log::info!("Windows service stopped: {WIN_SERVICE_NAME}");
    Ok(())
}

// ─────────────────────────────────────────────
// 서비스 모드 실행 (--service 플래그)
// ─────────────────────────────────────────────

/// 서비스 모드에서 사용하는 스캔 상태
struct ServiceScannerState {
    service_enabled: bool,
    targets: Vec<crate::app::TargetApp>,
}

define_windows_service!(ffi_service_main, service_main_impl);

fn service_main_impl(arguments: Vec<OsString>) {
    if let Err(e) = run_service_loop(arguments) {
        log::error!("Windows service loop error: {e}");
    }
}

fn run_service_loop(_arguments: Vec<OsString>) -> Result<()> {
    let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                let _ = stop_tx.send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(WIN_SERVICE_NAME, event_handler)
        .map_err(|e| anyhow!("register control handler failed: {e}"))?;

    // 시작 중 상태 보고
    status_handle
        .set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })
        .map_err(|e| anyhow!("set_service_status failed: {e}"))?;

    // 설정 로드 → 스캔 상태 초기화
    let config = crate::config::load_config()
        .ok()
        .flatten()
        .unwrap_or_default();

    let scanner_state = Arc::new(Mutex::new(ServiceScannerState {
        service_enabled: config.service_enabled,
        targets: config.targets,
    }));

    let platform: Arc<dyn crate::platform::Platform> = Arc::new(WindowsPlatform::new());
    let scanner_state_bg = Arc::clone(&scanner_state);
    let platform_bg = Arc::clone(&platform);

    // 백그라운드 차단 루프
    let _blocker = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build();
        let Ok(rt) = rt else {
            return;
        };

        rt.block_on(async move {
            let mut last_hidden: Option<super::NativeWindowHandle> = None;

            loop {
                let snapshot = scanner_state_bg
                    .lock()
                    .ok()
                    .map(|s| (s.service_enabled, s.targets.clone()));

                let Some((service_enabled, targets)) = snapshot else {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                };

                if !service_enabled {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }

                let mut detected_handle: Option<super::NativeWindowHandle> = None;

                for target in targets.iter().filter(|t| t.enabled) {
                    if !platform_bg.is_target_running(&target.process_name) {
                        continue;
                    }
                    if let Ok(Some(hwnd)) = platform_bg.find_ad_window(&target.process_name) {
                        detected_handle = Some(hwnd);
                        break;
                    }
                }

                if let Some(hwnd) = detected_handle {
                    if let Err(e) = platform_bg.hide_ad(hwnd) {
                        log::warn!("hide_ad failed: {e}");
                    } else if last_hidden != Some(hwnd) {
                        log::info!("Ad window hidden (service mode)");
                    }
                    last_hidden = Some(hwnd);
                } else if let Some(hwnd) = last_hidden {
                    let _ = platform_bg.show_ad(hwnd);
                    last_hidden = None;
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
    });

    // SCM STOP 신호 대기
    let _ = stop_rx.recv();

    // 중지 상태 보고
    let _ = status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    });

    log::info!("Windows service stopped gracefully.");
    Ok(())
}

/// SCM 서비스 디스패처를 시작한다. `--service` 플래그로 호출된 경우 main에서 사용한다.
pub fn run_as_windows_service() -> Result<()> {
    service_dispatcher::start(WIN_SERVICE_NAME, ffi_service_main)
        .map_err(|e| anyhow!("service_dispatcher::start failed: {e}"))?;
    Ok(())
}

// ─────────────────────────────────────────────
// 작업 스케줄러 (schtasks.exe) 관리
// ─────────────────────────────────────────────

/// 작업 스케줄러에 등록할 작업 이름
pub const TASK_NAME: &str = "gpui-convenience-tools";

/// 작업 스케줄러 작업 상태
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskState {
    /// 작업이 등록되지 않은 상태
    NotInstalled,
    /// 등록됨, 실행 대기 중
    Ready,
    /// 현재 실행 중
    Running,
    /// 비활성화됨
    Disabled,
    /// 조회 실패
    Unknown,
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInstalled => write!(f, "등록되지 않음"),
            Self::Ready => write!(f, "대기 중"),
            Self::Running => write!(f, "실행 중"),
            Self::Disabled => write!(f, "비활성화"),
            Self::Unknown => write!(f, "알 수 없음"),
        }
    }
}

/// 작업 스케줄러에서 작업 상태를 조회한다.
pub fn query_task_state() -> TaskState {
    let output = std::process::Command::new("schtasks")
        .args(["/Query", "/TN", TASK_NAME, "/FO", "CSV", "/NH"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    let Ok(output) = output else {
        return TaskState::Unknown;
    };

    if !output.status.success() {
        // 비정상 종료 = 작업 미등록 or 조회 실패
        return TaskState::NotInstalled;
    }

    // CSV 출력: "<name>","<next_run>","<status>"
    let stdout = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
    if stdout.contains("running") || stdout.contains("실행 중") {
        TaskState::Running
    } else if stdout.contains("disabled") || stdout.contains("사용 안 함") {
        TaskState::Disabled
    } else {
        TaskState::Ready
    }
}

/// 로그온 시 자동 시작 작업을 등록한다.
///
/// 현재 실행 파일 경로에 `--tray` 인수를 붙여 트레이 모드로 기동한다.
pub fn install_task() -> Result<()> {
    let exe_path = std::env::current_exe()
        .map_err(|e| anyhow!("current_exe failed: {e}"))?;
    let exe_str = exe_path.to_string_lossy();
    let tr = format!("\"{}\" --tray", exe_str);

    let output = std::process::Command::new("schtasks")
        .args([
            "/Create",
            "/TN", TASK_NAME,
            "/TR", &tr,
            "/SC", "ONLOGON",
            "/IT",
            "/F",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| anyhow!("schtasks /Create 실행 실패: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(anyhow!(
            "schtasks /Create 실패: {}{}",
            stderr.trim(),
            stdout.trim()
        ));
    }

    log::info!("Task Scheduler 작업 등록: {TASK_NAME}");
    Ok(())
}

/// 등록된 작업을 삭제한다.
pub fn uninstall_task() -> Result<()> {
    let output = std::process::Command::new("schtasks")
        .args(["/Delete", "/TN", TASK_NAME, "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| anyhow!("schtasks /Delete 실행 실패: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(anyhow!(
            "schtasks /Delete 실패: {}{}",
            stderr.trim(),
            stdout.trim()
        ));
    }

    log::info!("Task Scheduler 작업 삭제: {TASK_NAME}");
    Ok(())
}

/// 등록된 작업을 즉시 실행한다.
pub fn run_task_now() -> Result<()> {
    let output = std::process::Command::new("schtasks")
        .args(["/Run", "/TN", TASK_NAME])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| anyhow!("schtasks /Run 실행 실패: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(anyhow!(
            "schtasks /Run 실패: {}{}",
            stderr.trim(),
            stdout.trim()
        ));
    }

    log::info!("Task Scheduler 작업 즉시 실행: {TASK_NAME}");
    Ok(())
}

// ─────────────────────────────────────────────
// B-1: 시스템 서비스 목록/제어 (SCM Win32)
// ─────────────────────────────────────────────

/// 현재 프로세스가 관리자 권한으로 실행 중인지 확인한다.
#[allow(dead_code)]
pub fn is_elevated() -> bool {
    // SAFETY: IsUserAnAdmin은 현재 스레드 토큰만 조회하며 부작용이 없다.
    unsafe { IsUserAnAdmin() != 0 }
}

/// Wide null-terminated 포인터를 String으로 변환한다.
///
/// # Safety
/// `ptr`은 null이거나, null-terminator로 끝나는 유효한 UTF-16 문자열 메모리를 가리켜야 한다.
fn wide_ptr_to_string(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0;
    // SAFETY: 호출자 계약에 의해 null-terminated UTF-16 메모리다.
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len)).to_string()
    }
}

/// SCM 핸들을 열고 반환한다.
fn open_scm_handle(access: u32) -> Result<isize> {
    // SAFETY: null 포인터는 로컬 컴퓨터 / 기본 데이터베이스를 의미하는 유효한 인수다.
    let handle = unsafe {
        windows_sys::Win32::System::Services::OpenSCManagerW(
            std::ptr::null(),
            std::ptr::null(),
            access,
        )
    };
    if handle == 0 {
        Err(anyhow!("OpenSCManager failed (access={access:#x})"))
    } else {
        Ok(handle)
    }
}

/// Win32 SCM API로 설치된 Win32 서비스 목록을 반환한다.
#[allow(dead_code)]
fn list_sys_services_impl() -> Result<Vec<crate::platform::SysServiceInfo>> {
    use crate::platform::{SysServiceInfo, SysServiceStartType, SysServiceStatus};
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, EnumServicesStatusExW, ENUM_SERVICE_STATUS_PROCESSW,
        SC_ENUM_PROCESS_INFO, SC_MANAGER_CONNECT, SC_MANAGER_ENUMERATE_SERVICE,
        SERVICE_STATE_ALL, SERVICE_WIN32,
    };

    let scm = open_scm_handle(SC_MANAGER_CONNECT | SC_MANAGER_ENUMERATE_SERVICE)?;

    // 1차 호출: 필요한 버퍼 크기 조회 (버퍼 null → ERROR_INSUFFICIENT_BUFFER 예상).
    let mut bytes_needed: u32 = 0;
    let mut services_returned: u32 = 0;
    let mut resume_handle: u32 = 0;
    // SAFETY: 버퍼 포인터 null, 크기 0으로 필요 크기를 bytes_needed에 기록한다.
    unsafe {
        EnumServicesStatusExW(
            scm,
            SC_ENUM_PROCESS_INFO,
            SERVICE_WIN32,
            SERVICE_STATE_ALL,
            std::ptr::null_mut(),
            0,
            &mut bytes_needed,
            &mut services_returned,
            &mut resume_handle,
            std::ptr::null(),
        );
    }

    if bytes_needed == 0 {
        // SAFETY: scm은 open_scm_handle이 반환한 유효한 핸들이다.
        unsafe { CloseServiceHandle(scm); }
        return Ok(vec![]);
    }

    let mut buffer = vec![0u8; bytes_needed as usize];
    resume_handle = 0;
    services_returned = 0;

    // 2차 호출: 서비스 목록을 buffer에 채운다.
    // SAFETY: buffer는 bytes_needed 크기로 할당됐고 모든 포인터는 유효하다.
    let ok = unsafe {
        EnumServicesStatusExW(
            scm,
            SC_ENUM_PROCESS_INFO,
            SERVICE_WIN32,
            SERVICE_STATE_ALL,
            buffer.as_mut_ptr(),
            bytes_needed,
            &mut bytes_needed,
            &mut services_returned,
            &mut resume_handle,
            std::ptr::null(),
        )
    };

    // SAFETY: scm 핸들 해제.
    unsafe { CloseServiceHandle(scm); }

    if ok == 0 {
        return Err(anyhow!("EnumServicesStatusExW failed"));
    }

    let mut result = Vec::with_capacity(services_returned as usize);
    // SAFETY: buffer에는 API가 채운 ENUM_SERVICE_STATUS_PROCESSW 배열이 있다.
    let entry_ptr = buffer.as_ptr() as *const ENUM_SERVICE_STATUS_PROCESSW;
    for i in 0..(services_returned as usize) {
        let entry = unsafe { &*entry_ptr.add(i) };
        let name = wide_ptr_to_string(entry.lpServiceName as *const u16);
        let display_name = wide_ptr_to_string(entry.lpDisplayName as *const u16);

        let status = match entry.ServiceStatusProcess.dwCurrentState {
            1 => SysServiceStatus::Stopped,
            2 => SysServiceStatus::StartPending,
            3 => SysServiceStatus::StopPending,
            4 => SysServiceStatus::Running,
            7 => SysServiceStatus::Paused,
            _ => SysServiceStatus::Unknown,
        };

        result.push(SysServiceInfo {
            name,
            display_name,
            status,
            // 목록 조회 시 start_type은 Unknown; query_sys_service로 상세 조회 가능.
            start_type: SysServiceStartType::Unknown,
        });
    }

    Ok(result)
}

/// 지정한 서비스를 시작한다. 관리자 권한이 없으면 실패한다.
#[allow(dead_code)]
fn start_sys_service_impl(name: &str) -> Result<()> {
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, OpenServiceW, StartServiceW, SC_MANAGER_CONNECT, SERVICE_START,
    };

    let scm = open_scm_handle(SC_MANAGER_CONNECT)?;
    let service_name = wide_null(name);
    // SAFETY: service_name은 wide_null이 생성한 null-terminated UTF-16 벡터다.
    let svc = unsafe { OpenServiceW(scm, service_name.as_ptr(), SERVICE_START) };
    if svc == 0 {
        unsafe { CloseServiceHandle(scm); }
        return Err(anyhow!("OpenService(START) failed for '{name}'"));
    }

    // SAFETY: 인수 0개로 서비스를 비동기 시작한다.
    let ok = unsafe { StartServiceW(svc, 0, std::ptr::null()) };
    unsafe {
        CloseServiceHandle(svc);
        CloseServiceHandle(scm);
    }

    if ok == 0 {
        Err(anyhow!("StartService failed for '{name}'"))
    } else {
        log::info!("System service start requested: {name}");
        Ok(())
    }
}

/// 지정한 서비스를 중지한다. 관리자 권한이 없으면 실패한다.
#[allow(dead_code)]
fn stop_sys_service_impl(name: &str) -> Result<()> {
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, ControlService, OpenServiceW, SERVICE_STATUS,
        SC_MANAGER_CONNECT, SERVICE_CONTROL_STOP, SERVICE_STOP,
    };

    let scm = open_scm_handle(SC_MANAGER_CONNECT)?;
    let service_name = wide_null(name);
    // SAFETY: service_name은 wide_null이 생성한 null-terminated UTF-16 벡터다.
    let svc = unsafe { OpenServiceW(scm, service_name.as_ptr(), SERVICE_STOP) };
    if svc == 0 {
        unsafe { CloseServiceHandle(scm); }
        return Err(anyhow!("OpenService(STOP) failed for '{name}'"));
    }

    // SAFETY: SERVICE_STATUS는 zero-initialized 출력 파라미터로 사용된다.
    let mut status = unsafe { std::mem::zeroed::<SERVICE_STATUS>() };
    let ok = unsafe { ControlService(svc, SERVICE_CONTROL_STOP, &mut status) };
    unsafe {
        CloseServiceHandle(svc);
        CloseServiceHandle(scm);
    }

    if ok == 0 {
        Err(anyhow!("ControlService(STOP) failed for '{name}'"))
    } else {
        log::info!("System service stop requested: {name}");
        Ok(())
    }
}

/// 지정한 서비스의 현재 상태와 설정을 상세 조회한다.
#[allow(dead_code)]
fn query_sys_service_impl(name: &str) -> Result<crate::platform::SysServiceInfo> {
    use crate::platform::{SysServiceInfo, SysServiceStartType, SysServiceStatus};
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, OpenServiceW, QueryServiceConfigW, QueryServiceStatusEx,
        QUERY_SERVICE_CONFIGW, SC_MANAGER_CONNECT, SC_STATUS_PROCESS_INFO,
        SERVICE_QUERY_CONFIG, SERVICE_QUERY_STATUS, SERVICE_STATUS_PROCESS,
    };

    let scm = open_scm_handle(SC_MANAGER_CONNECT)?;
    let service_name = wide_null(name);
    // SAFETY: service_name은 wide_null이 생성한 null-terminated UTF-16 벡터다.
    let svc = unsafe {
        OpenServiceW(
            scm,
            service_name.as_ptr(),
            SERVICE_QUERY_STATUS | SERVICE_QUERY_CONFIG,
        )
    };
    if svc == 0 {
        unsafe { CloseServiceHandle(scm); }
        return Err(anyhow!("OpenService failed for '{name}'"));
    }

    // 상태 조회
    let mut bytes_needed: u32 = 0;
    let mut status_buf = vec![0u8; std::mem::size_of::<SERVICE_STATUS_PROCESS>()];
    // SAFETY: status_buf는 SERVICE_STATUS_PROCESS 크기로 할당됐다.
    unsafe {
        QueryServiceStatusEx(
            svc,
            SC_STATUS_PROCESS_INFO,
            status_buf.as_mut_ptr(),
            status_buf.len() as u32,
            &mut bytes_needed,
        );
    }

    // SAFETY: status_buf는 QueryServiceStatusEx가 채운 SERVICE_STATUS_PROCESS다.
    let status_proc = unsafe { &*(status_buf.as_ptr() as *const SERVICE_STATUS_PROCESS) };
    let sys_status = match status_proc.dwCurrentState {
        1 => SysServiceStatus::Stopped,
        2 => SysServiceStatus::StartPending,
        3 => SysServiceStatus::StopPending,
        4 => SysServiceStatus::Running,
        7 => SysServiceStatus::Paused,
        _ => SysServiceStatus::Unknown,
    };

    // 설정 조회: 1차 호출로 필요 크기 파악.
    let mut config_bytes_needed: u32 = 0;
    // SAFETY: null 버퍼, 크기 0으로 필요 크기만 조회한다.
    unsafe {
        QueryServiceConfigW(svc, std::ptr::null_mut(), 0, &mut config_bytes_needed);
    }

    let (start_type, display_name) = if config_bytes_needed > 0 {
        let mut config_buf = vec![0u8; config_bytes_needed as usize];
        // SAFETY: config_buf는 config_bytes_needed 크기로 할당됐다.
        let ok = unsafe {
            QueryServiceConfigW(
                svc,
                config_buf.as_mut_ptr() as *mut QUERY_SERVICE_CONFIGW,
                config_bytes_needed,
                &mut config_bytes_needed,
            )
        };
        if ok != 0 {
            // SAFETY: QueryServiceConfigW가 config_buf를 QUERY_SERVICE_CONFIGW로 채웠다.
            let cfg = unsafe { &*(config_buf.as_ptr() as *const QUERY_SERVICE_CONFIGW) };
            let st = match cfg.dwStartType {
                2 => SysServiceStartType::Automatic,
                3 => SysServiceStartType::Manual,
                4 => SysServiceStartType::Disabled,
                _ => SysServiceStartType::Unknown,
            };
            let dn = wide_ptr_to_string(cfg.lpDisplayName as *const u16);
            (st, dn)
        } else {
            (SysServiceStartType::Unknown, String::new())
        }
    } else {
        (SysServiceStartType::Unknown, String::new())
    };

    unsafe {
        CloseServiceHandle(svc);
        CloseServiceHandle(scm);
    }

    let display = if display_name.is_empty() { name.to_string() } else { display_name };
    Ok(SysServiceInfo {
        name: name.to_string(),
        display_name: display,
        status: sys_status,
        start_type,
    })
}

/// 지정한 서비스를 SCM에서 영구 삭제한다. 관리자 권한이 필요하다.
#[allow(dead_code)]
fn delete_sys_service_impl(name: &str) -> Result<()> {
    use windows_sys::Win32::System::Services::{CloseServiceHandle, DeleteService, OpenServiceW, SC_MANAGER_CONNECT};
    // DELETE = 0x00010000
    const DELETE_ACCESS: u32 = 0x00010000;

    let scm = open_scm_handle(SC_MANAGER_CONNECT)?;
    let service_name = wide_null(name);
    // SAFETY: service_name은 wide_null이 생성한 null-terminated UTF-16 벡터다.
    let svc = unsafe { OpenServiceW(scm, service_name.as_ptr(), DELETE_ACCESS) };
    if svc == 0 {
        unsafe { CloseServiceHandle(scm); }
        return Err(anyhow!("OpenServiceW failed for '{name}': service not found or access denied"));
    }
    // SAFETY: svc는 유효한 서비스 핸들이다.
    let result = unsafe { DeleteService(svc) };
    unsafe {
        CloseServiceHandle(svc);
        CloseServiceHandle(scm);
    }
    if result == 0 {
        Err(anyhow!("DeleteService failed for '{name}'"))
    } else {
        log::info!("System service deleted: {name}");
        Ok(())
    }
}
