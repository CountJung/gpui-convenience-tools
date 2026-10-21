//! Windows termination uses one opened handle for verification and termination.
//! sysinfo 0.31 invokes taskkill by PID, which cannot close the PID-reuse race.
use super::{ProcessError, ProcessIdentity};

#[cfg(target_os = "windows")]
mod windows {
    use super::*;
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME, HANDLE},
        System::Threading::{
            GetProcessTimes, IsProcessCritical, OpenProcess, QueryFullProcessImageNameW,
            TerminateProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
        },
    };

    struct ProcessHandle(HANDLE);
    impl Drop for ProcessHandle {
        fn drop(&mut self) {
            // SAFETY: owned non-null handle, closed exactly once.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    fn open(pid: u32, terminate: bool) -> Result<ProcessHandle, ProcessError> {
        let rights =
            PROCESS_QUERY_LIMITED_INFORMATION | if terminate { PROCESS_TERMINATE } else { 0 };
        // SAFETY: pid is a scalar; no handle inheritance or privilege changes.
        let handle = unsafe { OpenProcess(rights, 0, pid) };
        if handle == 0 {
            Err(ProcessError::AccessDenied)
        } else {
            Ok(ProcessHandle(handle))
        }
    }

    fn stamp(handle: &ProcessHandle) -> Result<u64, ProcessError> {
        let mut creation = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let mut exit = creation;
        let mut kernel = creation;
        let mut user = creation;
        // SAFETY: four initialized FILETIME out parameters and a valid owned handle.
        if unsafe { GetProcessTimes(handle.0, &mut creation, &mut exit, &mut kernel, &mut user) }
            == 0
        {
            return Err(ProcessError::Unverifiable);
        }
        Ok((u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime))
    }

    pub(super) fn creation_stamp(pid: u32) -> Option<u64> {
        stamp(&open(pid, false).ok()?).ok()
    }

    pub(super) fn terminate_verified(identity: &ProcessIdentity) -> Result<(), ProcessError> {
        let handle = open(identity.pid, true)?;
        let creation = stamp(&handle)?;
        let epoch_secs = creation / 10_000_000;
        if Some(creation) != identity.creation_stamp
            || epoch_secs.checked_sub(11_644_473_600) != Some(identity.start_time)
        {
            return Err(ProcessError::IdentityChanged);
        }
        let mut path = vec![0u16; 32768];
        let mut length = path.len() as u32;
        // SAFETY: writable buffer length is supplied and the same process handle stays alive.
        if unsafe { QueryFullProcessImageNameW(handle.0, 0, path.as_mut_ptr(), &mut length) } == 0 {
            return Err(ProcessError::Unverifiable);
        }
        let actual =
            std::path::PathBuf::from(std::ffi::OsString::from_wide(&path[..length as usize]));
        if actual != identity.path {
            return Err(ProcessError::IdentityChanged);
        }
        let mut critical = 0;
        // SAFETY: critical is a valid BOOL out parameter.
        if unsafe { IsProcessCritical(handle.0, &mut critical) } == 0 {
            return Err(ProcessError::Unverifiable);
        }
        if critical != 0 {
            return Err(ProcessError::Protected);
        }
        // SAFETY: only the identity-verified, noncritical process behind this handle is affected.
        // No taskkill, subprocess tree kill, elevation, or shell command is used.
        if unsafe { TerminateProcess(handle.0, 1) } == 0 {
            Err(ProcessError::AccessDenied)
        } else {
            Ok(())
        }
    }
}

#[cfg(target_os = "windows")]
pub(super) fn creation_stamp(pid: u32) -> Option<u64> {
    windows::creation_stamp(pid)
}
#[cfg(target_os = "windows")]
pub(super) fn terminate_verified(identity: &ProcessIdentity) -> Result<(), ProcessError> {
    windows::terminate_verified(identity)
}
#[cfg(not(target_os = "windows"))]
pub(super) fn creation_stamp(_: u32) -> Option<u64> {
    None
}
#[cfg(not(target_os = "windows"))]
pub(super) fn terminate_verified(_: &ProcessIdentity) -> Result<(), ProcessError> {
    Err(ProcessError::Unsupported)
}
