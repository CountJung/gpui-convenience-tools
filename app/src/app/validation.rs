//! 릴리스 화면 검증 하네스 전용 초기 상태 주입을 담당한다.

use gpui::Context;

use crate::virtual_disk::GuestPath;

use super::{ActivePanel, AppRoot};

/// 격리된 패널 화면을 실제 창 이동이나 사용자 입력 없이 지원 크기로 연다.
pub(crate) fn panel_preview_size() -> Option<gpui::Size<gpui::Pixels>> {
    std::env::var_os("GPUI_CONVENIENCE_TOOLS_DATA_DIR").filter(|dir| !dir.is_empty())?;
    let (width, height) = match std::env::var("GPUI_CONVENIENCE_TOOLS_INITIAL_PANEL")
        .ok()?
        .as_str()
    {
        "services-minimum" | "ad-block-minimum" | "log-minimum" | "log-list-minimum"
        | "process-minimum" => (920., 480.),
        "services-default" | "ad-block-default" | "log-default" | "process-default" => {
            (1000., 700.)
        }
        _ => return None,
    };
    Some(gpui::size(gpui::px(width), gpui::px(height)))
}

/// 검증 하네스가 패널 전환 입력 없이 특정 화면을 열 수 있도록 하는 실행 전용 선택값이다.
/// 일반 사용자 설정에는 저장하지 않으며, 값이 없거나 알 수 없으면 대시보드로 시작한다.
pub(super) fn initial_panel_from_validation_env() -> ActivePanel {
    match std::env::var("GPUI_CONVENIENCE_TOOLS_INITIAL_PANEL")
        .ok()
        .as_deref()
    {
        Some("services-minimum" | "services-default") if panel_preview_size().is_some() => {
            ActivePanel::Services
        }
        Some("ad-block-minimum" | "ad-block-default") if panel_preview_size().is_some() => {
            ActivePanel::AdBlock
        }
        Some("file_sync") => ActivePanel::FileSync,
        Some("virtual_disk") => ActivePanel::VirtualDisk,
        Some("openssl") => ActivePanel::OpenSsl,
        Some("timestamp") => ActivePanel::Timestamp,
        Some("process_monitor") => ActivePanel::ProcessMonitor,
        Some("process-minimum" | "process-default") if panel_preview_size().is_some() => {
            ActivePanel::ProcessMonitor
        }
        Some("log-minimum" | "log-default" | "log-list-minimum")
            if panel_preview_size().is_some() =>
        {
            ActivePanel::Logs
        }
        Some("clipboard_history") => ActivePanel::ClipboardHistory,
        Some("quick_launcher") => ActivePanel::QuickLauncher,
        Some("screenshot") => ActivePanel::Screenshot,
        Some("auto_start") => ActivePanel::AutoStart,
        Some("settings") => ActivePanel::Settings,
        _ => ActivePanel::Dashboard,
    }
}

/// 단절된 검증 창을 위한 고정 표본. 실제 프로세스 경로나 사용자 로그를 수집하지 않는다.
pub(super) fn seed_detail_preview(root: &mut AppRoot) {
    use crate::process_monitor::{ProcessIdentity, ProcessRow};
    if panel_preview_size().is_none() {
        return;
    }
    match root.active_panel {
        ActivePanel::ProcessMonitor => {
            let rows = ["Alpha", "Beta"].into_iter().enumerate().map(|(index, variant)| ProcessRow {
                name: "worker.exe".into(),
                identity: ProcessIdentity {
                    pid: u32::MAX - index as u32,
                    start_time: 1,
                    creation_stamp: None,
                    path: format!(r"C:\Example\SyntheticApplication\LongDirectoryWithoutSpacesForLayoutVerification\NestedDirectory\{variant}\worker.exe").into(),
                },
                cpu_percent: 1.,
                memory_bytes: 4_194_304,
                blocked_reason: Some("화면 검증용 합성 프로세스 · 종료 불가"),
            }).collect();
            root.process_monitor.seed_preview_rows(rows);
        }
        ActivePanel::Logs => {
            root.app_state.log_entries = [
                ("INFO", "작업을 시작했습니다.".to_owned()),
                (
                    "WARN",
                    format!(
                        "여러 줄 메시지입니다.\n{}\n마지막 문자: 끝",
                        "LongUnbrokenSample0123456789".repeat(6)
                    ),
                ),
                ("INFO", "다음 행 · 앞 행과 겹치지 않아야 합니다.".to_owned()),
            ]
            .into_iter()
            .map(|(level, message)| super::LogEntry {
                level: level.into(),
                message,
            })
            .collect();
            if std::env::var("GPUI_CONVENIENCE_TOOLS_INITIAL_PANEL").as_deref()
                != Ok("log-list-minimum")
            {
                root.app_state.selected_log_entry = Some(root.app_state.log_entries[1].clone());
            }
        }
        _ => {}
    }
}

/// 릴리스 화면 검증에서만 격리된 합성 VDI를 읽어 초기 상태를 만든다.
///
/// 환경 변수가 없으면 아무 동작도 하지 않으며, 경로·선택 상태를 사용자 설정에 저장하지
/// 않는다. 실제 제품 흐름은 사용자가 화면에서 VDI를 열고 파티션을 선택한다.
pub(super) fn seed_validation_virtual_disk(root: &mut AppRoot, cx: &mut Context<AppRoot>) {
    let Ok(path) = std::env::var("GPUI_CONVENIENCE_TOOLS_VALIDATION_VDI_PATH") else {
        return;
    };
    if path.trim().is_empty() {
        return;
    }

    root.virtual_disk.path_text = path;
    root.open_virtual_disk(cx);
    if !root.virtual_disk.partitions.is_empty() {
        let requested_partition_number =
            std::env::var("GPUI_CONVENIENCE_TOOLS_VALIDATION_VDI_PARTITION_NUMBER")
                .ok()
                .and_then(|value| value.parse::<u32>().ok())
                .filter(|number| *number > 0);
        let partition_index = requested_partition_number
            .and_then(|number| {
                root.virtual_disk
                    .partitions
                    .iter()
                    .position(|partition| partition.number == number)
            })
            .or_else(|| {
                if requested_partition_number.is_some() {
                    root.push_log(
                        "ERROR",
                        "검증용 VDI 파티션 번호를 찾지 못했습니다. 선택을 생략합니다.".to_string(),
                    );
                    root.virtual_disk.entries.clear();
                    root.virtual_disk.error = Some(
                        "검증용 VDI 파티션 번호를 찾지 못했습니다. 파티션 번호를 확인하세요."
                            .to_string(),
                    );
                    None
                } else {
                    Some(0)
                }
            });
        if let Some(partition_index) = partition_index {
            root.select_virtual_disk_partition(partition_index, cx);
        }
    }

    if let Ok(target) = std::env::var("GPUI_CONVENIENCE_TOOLS_VALIDATION_VDI_TARGET") {
        root.virtual_disk.copy.target_path_text = target;
    }
    if let Ok(path) = std::env::var("GPUI_CONVENIENCE_TOOLS_VALIDATION_VDI_GUEST_PATH") {
        let path = path.trim();
        if !path.is_empty() {
            match GuestPath::new(path) {
                Ok(guest_path) => {
                    root.virtual_disk.current_path = guest_path;
                    root.virtual_disk.selected_paths.clear();
                    root.virtual_disk.selection_anchor = None;
                    root.refresh_virtual_disk_directory(cx);
                }
                Err(error) => {
                    root.push_log("ERROR", format!("검증용 게스트 경로 시드 실패: {error}"));
                }
            }
        }
    }

    if std::env::var("GPUI_CONVENIENCE_TOOLS_VALIDATION_VDI_SELECT_ALL").as_deref() == Ok("1") {
        root.select_all_virtual_disk_entries(cx);
    }

    root.virtual_disk.validation_auto_copy_requested =
        std::env::var("GPUI_CONVENIENCE_TOOLS_VALIDATION_VDI_AUTO_COPY").as_deref() == Ok("1");
    root.virtual_disk.copy.validation_cancel_after_ms =
        std::env::var("GPUI_CONVENIENCE_TOOLS_VALIDATION_VDI_CANCEL_AFTER_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|delay_ms| *delay_ms > 0);
    root.virtual_disk.copy.validation_read_delay_ms =
        std::env::var("GPUI_CONVENIENCE_TOOLS_VALIDATION_VDI_READ_DELAY_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|delay_ms| *delay_ms > 0);
}
