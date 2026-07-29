//! 설치된 Win32 서비스 조회와 제어.
//!
//! `EnumServicesStatusEx`로 목록을 얻고 `StartService` / `ControlService` / `DeleteService`로
//! 제어한다. 조회를 제외한 모든 제어에는 관리자 권한이 필요하다.

use anyhow::{anyhow, Result};

use super::wide_null;

use windows_sys::Win32::UI::Shell::IsUserAnAdmin;

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
pub(super) fn list_sys_services_impl() -> Result<Vec<crate::platform::SysServiceInfo>> {
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
pub(super) fn start_sys_service_impl(name: &str) -> Result<()> {
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
pub(super) fn stop_sys_service_impl(name: &str) -> Result<()> {
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
pub(super) fn query_sys_service_impl(name: &str) -> Result<crate::platform::SysServiceInfo> {
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
pub(super) fn delete_sys_service_impl(name: &str) -> Result<()> {
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
