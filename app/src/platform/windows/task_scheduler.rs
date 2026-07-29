//! 작업 스케줄러 기반 자동 시작.
//!
//! `schtasks`를 ONLOGON + `/IT`(인터랙티브) 플래그로 등록해 사용자 세션에서 실행되게 한다.
//! SCM의 Session 0 격리 문제를 피하기 위한 선택이다.

use anyhow::{anyhow, Result};
use std::os::windows::process::CommandExt;

/// 콘솔 창 없이 자식 프로세스 생성
const CREATE_NO_WINDOW: u32 = 0x0800_0000;


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
