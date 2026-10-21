//! Read-only sampling and explicit, identity-checked termination of user processes.
mod termination;
mod worker;

use crate::config::ProcessSort;
use std::path::PathBuf;
use sysinfo::{Pid, Process, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
pub(crate) use worker::{MonitorWorker, WorkerReply};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProcessIdentity {
    pub pid: u32,
    pub start_time: u64,
    pub path: PathBuf,
    /// Windows FILETIME retains sub-second creation precision against PID reuse.
    pub creation_stamp: Option<u64>,
}

#[derive(Clone, Debug)]
pub(crate) struct ProcessRow {
    pub identity: ProcessIdentity,
    pub name: String,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub blocked_reason: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ProcessError {
    #[error("대상 PID가 종료되었거나 다른 프로세스로 바뀌었습니다. 새로고침하세요.")]
    IdentityChanged,
    #[error("Access denied or protected process. No automatic elevation is attempted.")]
    AccessDenied,
    #[error("System, critical, other-user and this application's processes cannot be terminated.")]
    Protected,
    #[error("The process identity could not be verified safely. Refresh and try again.")]
    Unverifiable,
    #[cfg(not(target_os = "windows"))]
    #[error("Safe process termination is currently supported only on Windows.")]
    Unsupported,
    #[error("The background process monitor is unavailable. Retry refresh.")]
    WorkerUnavailable,
    #[error("프로세스 갱신 실패 · 표시 중인 목록은 오래된 정보입니다. 다시 새로고침하세요.")]
    ScanFailed,
}

pub(crate) fn refresh_kind() -> ProcessRefreshKind {
    ProcessRefreshKind::new()
        .with_cpu()
        .with_memory()
        .with_user(UpdateKind::Always)
        .with_exe(UpdateKind::Always)
}

fn row_from_process(
    process: &Process,
    owner: Option<&sysinfo::Uid>,
    cpu_count: usize,
) -> ProcessRow {
    let path = process
        .exe()
        .map_or_else(PathBuf::new, std::path::Path::to_path_buf);
    let identity = ProcessIdentity {
        pid: process.pid().as_u32(),
        start_time: process.start_time(),
        path,
        creation_stamp: termination::creation_stamp(process.pid().as_u32()),
    };
    let name = process.name().to_string_lossy().into_owned();
    let blocked_reason = policy_reason(
        &identity,
        &name,
        owner.is_some_and(regular_owner) && process.user_id() == owner,
        owner.is_some()
            && process
                .effective_user_id()
                .is_none_or(|id| Some(id) == owner),
    );
    let cpu = process.cpu_usage() / cpu_count.max(1) as f32;
    ProcessRow {
        identity,
        name,
        cpu_percent: if cpu.is_finite() {
            cpu.clamp(0.0, 100.0)
        } else {
            0.0
        },
        memory_bytes: process.memory(),
        blocked_reason,
    }
}

fn regular_owner(owner: &sysinfo::Uid) -> bool {
    #[cfg(target_os = "windows")]
    {
        let sid = owner.to_string();
        !["S-1-5-18", "S-1-5-19", "S-1-5-20"].contains(&sid.as_str())
            && !sid.starts_with("S-1-5-80-")
            && !sid.starts_with("S-1-5-82-")
            && !sid.starts_with("S-1-5-90-")
    }
    #[cfg(not(target_os = "windows"))]
    {
        **owner != 0
    }
}

pub(crate) fn policy_reason(
    identity: &ProcessIdentity,
    name: &str,
    same_owner: bool,
    same_effective_owner: bool,
) -> Option<&'static str> {
    if identity.pid <= 4 || identity.pid == std::process::id() {
        return Some("시스템 PID 또는 이 앱은 종료할 수 없습니다.");
    }
    let lower = name.to_ascii_lowercase();
    if [
        "system",
        "registry",
        "secure system",
        "memory compression",
        "smss.exe",
        "csrss.exe",
        "wininit.exe",
        "winlogon.exe",
        "services.exe",
        "lsass.exe",
        "lsaiso.exe",
        "svchost.exe",
        "fontdrvhost.exe",
        "dwm.exe",
        "init",
        "systemd",
        "launchd",
        "kernel_task",
    ]
    .contains(&lower.as_str())
    {
        return Some("시스템/중요 프로세스는 종료할 수 없습니다.");
    }
    let path = identity
        .path
        .to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase();
    let windows = std::env::var("SystemRoot")
        .unwrap_or_else(|_| "C:\\Windows".into())
        .replace('/', "\\")
        .to_ascii_lowercase();
    if path.starts_with(&(windows.trim_end_matches('\\').to_owned() + "\\")) {
        return Some("Windows 시스템 디렉터리의 프로세스는 종료할 수 없습니다.");
    }
    if !same_owner || !same_effective_owner {
        return Some("현재 사용자 소유가 아니거나 소유자를 확인할 수 없습니다.");
    }
    if identity.start_time == 0 || identity.path.as_os_str().is_empty() {
        return Some("시작 시각/실행 경로를 확인할 수 없어 종료를 차단했습니다.");
    }
    #[cfg(target_os = "windows")]
    if identity.creation_stamp.is_none() {
        return Some("프로세스 생성 신원을 확인할 수 없어 종료를 차단했습니다.");
    }
    #[cfg(not(target_os = "windows"))]
    return Some("안전한 프로세스 종료는 현재 Windows에서만 지원합니다.");
    #[cfg(target_os = "windows")]
    None
}

/// Filter before truncation so searching can find processes outside the original top N.
pub(crate) fn visible_rows<'a>(
    rows: &'a [ProcessRow],
    query: &str,
    sort: ProcessSort,
    limit: usize,
) -> Vec<&'a ProcessRow> {
    let query = query.trim().to_lowercase();
    let mut visible: Vec<_> = rows
        .iter()
        .filter(|row| {
            query.is_empty()
                || row.name.to_lowercase().contains(&query)
                || row.identity.pid.to_string().contains(&query)
                || row
                    .identity
                    .path
                    .to_string_lossy()
                    .to_lowercase()
                    .contains(&query)
        })
        .collect();
    visible.sort_by(|a, b| {
        let order = match sort {
            ProcessSort::Cpu => b.cpu_percent.total_cmp(&a.cpu_percent),
            ProcessSort::Memory => b.memory_bytes.cmp(&a.memory_bytes),
        };
        order.then_with(|| a.identity.pid.cmp(&b.identity.pid))
    });
    visible.truncate(limit);
    visible
}

pub(crate) fn terminate(identity: &ProcessIdentity) -> Result<(), ProcessError> {
    // A fresh System prevents cached PID metadata from authorizing a replacement process.
    let mut system = System::new();
    let pids = [
        Pid::from_u32(identity.pid),
        Pid::from_u32(std::process::id()),
    ];
    system.refresh_processes_specifics(ProcessesToUpdate::Some(&pids), refresh_kind());
    let current = system
        .process(pids[0])
        .ok_or(ProcessError::IdentityChanged)?;
    let owner = system.process(pids[1]).and_then(Process::user_id);
    let fresh = row_from_process(current, owner, 1);
    if fresh.identity != *identity {
        return Err(ProcessError::IdentityChanged);
    }
    if fresh.blocked_reason.is_some() {
        return Err(ProcessError::Protected);
    }
    termination::terminate_verified(identity)
}

#[cfg(test)]
mod tests;
