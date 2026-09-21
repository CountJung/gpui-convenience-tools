//! 릴리스 화면 검증 하네스 전용 초기 상태 주입을 담당한다.

use gpui::Context;

use crate::virtual_disk::GuestPath;

use super::{ActivePanel, AppRoot};

/// 검증 하네스가 패널 전환 입력 없이 특정 화면을 열 수 있도록 하는 실행 전용 선택값이다.
/// 일반 사용자 설정에는 저장하지 않으며, 값이 없거나 알 수 없으면 대시보드로 시작한다.
pub(super) fn initial_panel_from_validation_env() -> ActivePanel {
    match std::env::var("GPUI_CONVENIENCE_TOOLS_INITIAL_PANEL")
        .ok()
        .as_deref()
    {
        Some("file_sync") => ActivePanel::FileSync,
        Some("virtual_disk") => ActivePanel::VirtualDisk,
        Some("auto_start") => ActivePanel::AutoStart,
        _ => ActivePanel::Dashboard,
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
        root.select_virtual_disk_partition(0, cx);
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

    if std::env::var("GPUI_CONVENIENCE_TOOLS_VALIDATION_VDI_SELECT_ALL").as_deref()
        == Ok("1")
    {
        root.select_all_virtual_disk_entries(cx);
    }

    root.virtual_disk.validation_auto_copy_requested =
        std::env::var("GPUI_CONVENIENCE_TOOLS_VALIDATION_VDI_AUTO_COPY").as_deref() == Ok("1");
    root.virtual_disk.copy.validation_cancel_after_ms =
        std::env::var("GPUI_CONVENIENCE_TOOLS_VALIDATION_VDI_CANCEL_AFTER_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|delay_ms| *delay_ms > 0);
}
