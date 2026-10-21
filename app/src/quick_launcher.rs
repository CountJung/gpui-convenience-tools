//! 실행 대상 검증과 shell-free 실행 계획. 등록되지 않은 자유 명령은 받지 않는다.
// Windows 경계는 소유 범위 안의 platform 하위 모듈로 격리한다.
pub(crate) mod hotkey;
pub(crate) mod platform;

use crate::config::launcher::{LauncherEntry, LauncherKind};
use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LauncherError {
    #[error("직접 확인한 절대 경로만 신뢰 등록할 수 있습니다.")]
    Untrusted,
    #[error("존재하는 로컬 절대 경로를 지정하세요. 네트워크·장치 경로는 지원하지 않습니다.")]
    InvalidPath,
    #[error(
        "앱은 .exe, 스크립트는 .ps1만 지원합니다. 명령 셸·인터프리터는 앱으로 등록할 수 없습니다."
    )]
    UnsupportedTarget,
    #[error("인수는 문자열 JSON 배열이어야 합니다 (최대 64개, 합계 16 KiB, NUL 금지).")]
    InvalidArguments,
    #[error("폴더에는 인수를 지정할 수 없습니다.")]
    FolderArguments,
    #[error("스크립트 실행은 매번 명시적 확인이 필요합니다.")]
    ConfirmationRequired,
    #[cfg(not(windows))]
    #[error("이 플랫폼에서는 빠른 실행을 지원하지 않습니다.")]
    UnsupportedPlatform,
    #[error("실행을 시작하지 못했습니다. 대상의 존재·권한·시스템 정책을 확인하세요.")]
    StartFailed,
}

pub fn parse_args(text: &str) -> Result<Vec<String>, LauncherError> {
    if text.len() > 128 * 1024 {
        return Err(LauncherError::InvalidArguments);
    }
    let args: Vec<String> =
        serde_json::from_str(text).map_err(|_| LauncherError::InvalidArguments)?;
    validate_args(&args)?;
    Ok(args)
}

fn validate_args(args: &[String]) -> Result<(), LauncherError> {
    if args.len() > 64
        || args.iter().map(String::len).sum::<usize>() > 16 * 1024
        || args.iter().any(|s| s.contains('\0'))
    {
        return Err(LauncherError::InvalidArguments);
    }
    Ok(())
}

fn local_absolute(path: &Path) -> bool {
    if !path.is_absolute() {
        return false;
    }
    #[cfg(windows)]
    {
        let s = path.to_string_lossy();
        let b = s.as_bytes();
        b.len() >= 3
            && b[0].is_ascii_alphabetic()
            && b[1] == b':'
            && matches!(b[2], b'\\' | b'/')
            && !s.contains('\0')
    }
    #[cfg(not(windows))]
    {
        !path.to_string_lossy().contains('\0')
    }
}

/// 정규화된 대상만 저장한다. 실행 직전에도 다시 검사해 삭제·종류 변경을 차단한다.
pub fn validate_entry(entry: &LauncherEntry) -> Result<PathBuf, LauncherError> {
    if !entry.trusted {
        return Err(LauncherError::Untrusted);
    }
    validate_args(&entry.args)?;
    let path = Path::new(&entry.path);
    if !local_absolute(path) {
        return Err(LauncherError::InvalidPath);
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| LauncherError::InvalidPath)?;
    let canonical = platform::normalize_path(canonical)?;
    if !local_absolute(&canonical) {
        return Err(LauncherError::InvalidPath);
    }
    let metadata = canonical
        .metadata()
        .map_err(|_| LauncherError::InvalidPath)?;
    match entry.kind {
        LauncherKind::Folder => {
            if !metadata.is_dir() {
                return Err(LauncherError::InvalidPath);
            }
            if !entry.args.is_empty() {
                return Err(LauncherError::FolderArguments);
            }
        }
        kind => {
            let extension = canonical.extension().and_then(|s| s.to_str()).unwrap_or("");
            let stem = canonical
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let blocked = [
                "cmd",
                "powershell",
                "powershell_ise",
                "pwsh",
                "wscript",
                "cscript",
                "mshta",
                "rundll32",
                "regsvr32",
                "bash",
                "sh",
                "wsl",
                "python",
                "python3",
                "node",
                "msiexec",
            ];
            if !metadata.is_file()
                || (kind == LauncherKind::Application
                    && (!extension.eq_ignore_ascii_case("exe") || blocked.contains(&stem.as_str())))
                || (kind == LauncherKind::Script && !extension.eq_ignore_ascii_case("ps1"))
            {
                return Err(LauncherError::UnsupportedTarget);
            }
        }
    }
    Ok(canonical)
}

/// 인수는 개별 argv다. cmd /c, PowerShell -Command, ShellExecute 문자열을 만들지 않는다.
pub struct LaunchPlan {
    pub program: PathBuf,
    pub args: Vec<String>,
}

pub fn plan(entry: &LauncherEntry, confirmed: bool) -> Result<LaunchPlan, LauncherError> {
    let path = validate_entry(entry)?;
    if entry.kind == LauncherKind::Script && !confirmed {
        return Err(LauncherError::ConfirmationRequired);
    }
    match entry.kind {
        LauncherKind::Application => Ok(LaunchPlan {
            program: path,
            args: entry.args.clone(),
        }),
        LauncherKind::Folder => Ok(LaunchPlan {
            program: platform::explorer()?,
            args: vec![path.to_string_lossy().into_owned()],
        }),
        LauncherKind::Script => {
            let mut args = vec![
                "-NoLogo".into(),
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-File".into(),
                path.to_string_lossy().into_owned(),
            ];
            args.extend(entry.args.clone());
            Ok(LaunchPlan {
                program: platform::powershell()?,
                args,
            })
        }
    }
}

impl LaunchPlan {
    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        platform::hide_console(&mut command);
        command
    }

    pub fn spawn(&self) -> Result<Child, LauncherError> {
        self.command()
            .spawn()
            .map_err(|_| LauncherError::StartFailed)
    }
}

pub fn matching_indices(entries: &[LauncherEntry], query: &str) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            (e.name.to_lowercase().contains(&query) || e.path.to_lowercase().contains(&query))
                .then_some(i)
        })
        .collect()
}

#[cfg(test)]
mod tests;
