//! VirtualBox 오프라인 디스크 탐색 상태와 읽기 작업.
//!
//! 이 모듈은 GPUI 렌더링과 VDI/NTFS 읽기 구현을 분리한다. 원본은 항상
//! `VdiReader`의 read-only 경계로 열며, 이 단계에서는 디스크·파티션·현재 경로를
//! 준비하고 목록을 새로 고치는 작업만 제공한다.

use std::path::PathBuf;

use gpui::{AppContext, Context, Entity, Window};
use gpui_component::input::{InputEvent, InputState};

use crate::virtual_disk::{
    ntfs::NtfsGuestFileSource, partition::discover_partitions, vdi::VdiReader, GuestFileAttributes,
    GuestFileEntry, GuestFileKind, GuestFileSource, GuestPath, VdiPartition, VirtualDiskError,
};

use super::AppRoot;

/// VDE-013에서 사용하는 오프라인 VDI 탐색 세션.
pub(crate) struct VirtualDiskSession {
    pub(crate) path_text: String,
    pub(crate) path_input: Option<Entity<InputState>>,
    pub(crate) vdi_path: Option<PathBuf>,
    pub(crate) partitions: Vec<VdiPartition>,
    pub(crate) selected_partition: Option<usize>,
    pub(crate) current_path: GuestPath,
    pub(crate) entries: Vec<GuestFileEntry>,
    pub(crate) source: Option<NtfsGuestFileSource<VdiReader>>,
    pub(crate) error: Option<String>,
}

impl Default for VirtualDiskSession {
    fn default() -> Self {
        Self {
            path_text: String::new(),
            path_input: None,
            vdi_path: None,
            partitions: Vec::new(),
            selected_partition: None,
            current_path: GuestPath::root(),
            entries: Vec::new(),
            source: None,
            error: None,
        }
    }
}

impl AppRoot {
    /// VDI 경로 입력을 첫 렌더 시점에 준비한다.
    pub(crate) fn ensure_virtual_disk_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.virtual_disk.path_input.is_some() {
            return;
        }

        let path = self.virtual_disk.path_text.clone();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(r"예: D:\VM\Windows.vdi")
                .default_value(path)
        });
        let subscription = cx.subscribe(
            &input,
            |this: &mut Self, input: Entity<InputState>, event: &InputEvent, cx| {
                if let InputEvent::Change = event {
                    this.virtual_disk.path_text = input.read(cx).value().to_string();
                    cx.notify();
                }
            },
        );
        self.virtual_disk.path_input = Some(input);
        self.subscriptions.push(subscription);
    }

    /// 입력된 VDI를 read-only로 열고 파티션 목록을 검색한다.
    pub(crate) fn open_virtual_disk(&mut self, cx: &mut Context<Self>) {
        let path_text = self.virtual_disk.path_text.trim().to_string();
        self.virtual_disk.error = None;
        self.virtual_disk.entries.clear();
        self.virtual_disk.partitions.clear();
        self.virtual_disk.selected_partition = None;
        self.virtual_disk.source = None;
        self.virtual_disk.vdi_path = None;
        self.virtual_disk.current_path = GuestPath::root();

        if path_text.is_empty() {
            self.set_virtual_disk_error("VDI 파일 경로를 입력하세요".to_string(), cx);
            return;
        }

        let path = PathBuf::from(&path_text);
        let result = (|| {
            let mut reader = VdiReader::open(&path)?;
            let partitions = discover_partitions(&mut reader)?;
            Ok::<_, VirtualDiskError>((partitions, reader))
        })();

        match result {
            Ok((partitions, _reader)) => {
                self.virtual_disk.path_text = path_text;
                self.virtual_disk.vdi_path = Some(path);
                self.virtual_disk.partitions = partitions;
                self.push_log(
                    "INFO",
                    format!(
                        "VDI를 읽기 전용으로 열었습니다: {}개 파티션",
                        self.virtual_disk.partitions.len()
                    ),
                );
            }
            Err(error) => {
                self.set_virtual_disk_error(format_virtual_disk_error(&error), cx);
            }
        }
        cx.notify();
    }

    /// 선택한 파티션을 NTFS 읽기 전용 소스로 연결하고 루트 목록을 읽는다.
    pub(crate) fn select_virtual_disk_partition(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(partition) = self.virtual_disk.partitions.get(index).cloned() else {
            return;
        };
        let Some(path) = self.virtual_disk.vdi_path.clone() else {
            self.set_virtual_disk_error("먼저 VDI를 열어야 합니다".to_string(), cx);
            return;
        };

        self.virtual_disk.error = None;
        let result = (|| {
            let reader = VdiReader::open(path)?;
            NtfsGuestFileSource::open(reader, partition)
        })();
        match result {
            Ok(source) => {
                self.virtual_disk.source = Some(source);
                self.virtual_disk.selected_partition = Some(index);
                self.virtual_disk.current_path = GuestPath::root();
                self.refresh_virtual_disk_directory(cx);
            }
            Err(error) => self.set_virtual_disk_error(format_virtual_disk_error(&error), cx),
        }
        cx.notify();
    }

    /// 현재 게스트 경로의 항목 목록을 다시 읽는다.
    pub(crate) fn refresh_virtual_disk_directory(&mut self, cx: &mut Context<Self>) {
        let Some(source) = self.virtual_disk.source.as_mut() else {
            self.virtual_disk.entries.clear();
            return;
        };

        let directory = GuestFileEntry {
            path: self.virtual_disk.current_path.clone(),
            kind: GuestFileKind::Directory,
            size_bytes: 0,
            attributes: GuestFileAttributes::default(),
            times: Default::default(),
        };
        match source.list_directory(&directory) {
            Ok(mut entries) => {
                // 탐색기와 같은 순서를 고정한다. 숨김·시스템 항목은 필터링하지 않는다.
                entries.sort_by(|left, right| {
                    let left_kind = matches!(left.kind, GuestFileKind::File);
                    let right_kind = matches!(right.kind, GuestFileKind::File);
                    left_kind
                        .cmp(&right_kind)
                        .then_with(|| left.path.as_str().cmp(right.path.as_str()))
                });
                self.virtual_disk.entries = entries;
                self.virtual_disk.error = None;
            }
            Err(error) => self.set_virtual_disk_error(format_virtual_disk_error(&error), cx),
        }
    }

    fn set_virtual_disk_error(&mut self, message: String, cx: &mut Context<Self>) {
        self.virtual_disk.error = Some(message.clone());
        self.push_log("ERROR", format!("VirtualBox 디스크 탐색: {message}"));
        cx.notify();
    }
}

fn format_virtual_disk_error(error: &VirtualDiskError) -> String {
    format!("읽기 실패: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_session_starts_at_guest_root_without_an_open_source() {
        let session = VirtualDiskSession::default();

        assert!(session.path_input.is_none());
        assert!(session.vdi_path.is_none());
        assert!(session.partitions.is_empty());
        assert!(session.selected_partition.is_none());
        assert!(session.current_path.is_root());
        assert!(session.entries.is_empty());
        assert!(session.source.is_none());
        assert!(session.error.is_none());
    }
}
