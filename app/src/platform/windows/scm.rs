//! Windows 서비스(SCM) 등록과 서비스 모드 실행.
//!
//! **주의**: SCM 서비스는 Session 0(비대화형)에서 실행되어 사용자 세션의 창을
//! 조작할 수 없다. 그래서 광고 차단 자동 시작에는 이 경로를 쓰지 않고
//! [`super::task_scheduler`]를 사용한다. 이 모듈은 서비스 등록 기능 자체를 위해 남아 있다.

use anyhow::{anyhow, Result};
use std::ffi::{OsStr, OsString};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::WindowsPlatform;
use crate::platform::NativeWindowHandle;

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
            let mut last_hidden: Option<NativeWindowHandle> = None;

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

                let mut detected_handle: Option<NativeWindowHandle> = None;

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
