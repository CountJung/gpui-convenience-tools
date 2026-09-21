//! VirtualBox 오프라인 디스크 탐색 상태와 읽기 작업.
//!
//! 이 모듈은 GPUI 렌더링과 VDI/NTFS 읽기 구현을 분리한다. 원본은 항상
//! `VdiReader`의 read-only 경계로 열며, 디스크·파티션·현재 경로·선택·포커스 상태와
//! 안전/지원 범위 오류 메시지를 관리한다.

use std::{collections::{BTreeSet, HashSet}, path::PathBuf};

use gpui::{AppContext, Context, Entity, FocusHandle, Window};
use gpui_component::{input::{InputEvent, InputState}, notification::NotificationType};

use crate::virtual_disk::{
    ntfs::NtfsGuestFileSource, partition::discover_partitions, vdi::VdiReader, GuestFileAttributes,
    GuestFileEntry, GuestFileKind, GuestFileSource, GuestPath, UnsupportedFormatKind, VdiPartition,
    VirtualDiskError,
};

use super::virtual_disk_copy::VirtualDiskCopyState;
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
    pub(crate) selected_paths: HashSet<GuestPath>,
    pub(crate) focus_handle: Option<FocusHandle>,
    pub(crate) copy: VirtualDiskCopyState,
    pub(crate) source: Option<Box<dyn GuestFileSource>>,
    pub(crate) error: Option<String>,
    pub(crate) suppressed_issue_keys: BTreeSet<String>,
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
            selected_paths: HashSet::new(),
            focus_handle: None,
            copy: VirtualDiskCopyState::default(),
            source: None,
            error: None,
            suppressed_issue_keys: BTreeSet::new(),
        }
    }
}

impl AppRoot {
    pub(crate) fn toggle_virtual_disk_issue_suppression(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let suppressed = if !self.virtual_disk.suppressed_issue_keys.remove(key) {
            self.virtual_disk
                .suppressed_issue_keys
                .insert(key.to_string());
            true
        } else {
            false
        };
        let keys: Vec<String> = self
            .virtual_disk
            .suppressed_issue_keys
            .iter()
            .cloned()
            .collect();

        #[cfg(test)]
        let should_persist = self.sync.external_side_effects_enabled;
        #[cfg(not(test))]
        let should_persist = true;
        if should_persist {
            if let Err(err) = crate::config::update_config(|config| {
                config.virtual_disk_suppressed_issue_keys = keys;
            }) {
                self.push_log("ERROR", format!("VDI 알림 억제 설정 저장 실패: {err}"));
            }
        }

        self.push_log(
            "INFO",
            if suppressed {
                format!("VDI 오류 반복 알림을 억제했습니다: {key}")
            } else {
                format!("VDI 오류 반복 알림을 다시 표시합니다: {key}")
            },
        );
        self.notify_toast(
            if suppressed {
                "VDI 오류 반복 알림을 억제했습니다"
            } else {
                "VDI 오류 반복 알림을 다시 표시합니다"
            },
            NotificationType::Info,
            window,
            cx,
        );
        cx.notify();
    }

    /// 탐색기 목록이 키보드 단축키를 받을 수 있도록 포커스 핸들을 준비한다.
    pub(crate) fn ensure_virtual_disk_focus(&mut self, cx: &mut Context<Self>) {
        if self.virtual_disk.focus_handle.is_none() {
            self.virtual_disk.focus_handle = Some(cx.focus_handle());
        }
    }

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
        self.virtual_disk.selected_paths.clear();
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
                self.virtual_disk.source = Some(Box::new(source));
                self.virtual_disk.selected_partition = Some(index);
                self.virtual_disk.current_path = GuestPath::root();
                self.virtual_disk.selected_paths.clear();
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
                self.virtual_disk
                    .selected_paths
                    .retain(|path| entries.iter().any(|entry| &entry.path == path));
                self.virtual_disk.entries = entries;
                self.virtual_disk.error = None;
            }
            Err(error) => self.set_virtual_disk_error(format_virtual_disk_error(&error), cx),
        }
    }

    /// 항목을 선택하거나 Ctrl/Shift 선택으로 현재 선택 집합에 추가·제거한다.
    pub(crate) fn select_virtual_disk_entry(
        &mut self,
        index: usize,
        additive: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self
            .virtual_disk
            .entries
            .get(index)
            .map(|entry| entry.path.clone())
        else {
            return;
        };
        toggle_guest_selection(&mut self.virtual_disk.selected_paths, path, additive);
        cx.notify();
    }

    /// 폴더를 열어 현재 경로를 이동한다.
    pub(crate) fn enter_virtual_disk_directory(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(entry) = self.virtual_disk.entries.get(index).cloned() else {
            return;
        };
        if !entry.is_directory() {
            return;
        }
        self.virtual_disk.current_path = entry.path;
        self.virtual_disk.selected_paths.clear();
        self.refresh_virtual_disk_directory(cx);
        cx.notify();
    }

    /// 현재 게스트 경로의 부모로 이동한다. 루트에서는 아무 동작도 하지 않는다.
    pub(crate) fn go_to_virtual_disk_parent(&mut self, cx: &mut Context<Self>) {
        let Some(parent) = parent_guest_path(&self.virtual_disk.current_path) else {
            return;
        };
        self.virtual_disk.current_path = parent;
        self.virtual_disk.selected_paths.clear();
        self.refresh_virtual_disk_directory(cx);
        cx.notify();
    }

    /// 현재 폴더의 모든 항목을 선택한다. 숨김·시스템 항목도 포함한다.
    pub(crate) fn select_all_virtual_disk_entries(&mut self, cx: &mut Context<Self>) {
        self.virtual_disk.selected_paths = select_all_paths(&self.virtual_disk.entries);
        cx.notify();
    }

    /// 정확히 하나의 폴더가 선택된 경우 해당 폴더로 진입한다.
    pub(crate) fn enter_selected_virtual_disk_directory(&mut self, cx: &mut Context<Self>) {
        let Some(index) = selected_directory_index(
            &self.virtual_disk.entries,
            &self.virtual_disk.selected_paths,
        ) else {
            return;
        };
        self.enter_virtual_disk_directory(index, cx);
    }

    fn set_virtual_disk_error(&mut self, message: String, cx: &mut Context<Self>) {
        self.virtual_disk.error = Some(message.clone());
        self.push_log("ERROR", format!("VirtualBox 디스크 탐색: {message}"));
        cx.notify();
    }
}

fn format_virtual_disk_error(error: &VirtualDiskError) -> String {
    match error {
        VirtualDiskError::ReadOnlyViolation(detail) => format!(
            "접근 차단: 실행 중인 VM이 사용 중이거나 잠금 상태인 VDI는 직접 읽을 수 없습니다. VM을 완전히 종료한 뒤 다시 시도하세요. ({detail})"
        ),
        VirtualDiskError::UnsupportedFormat {
            kind: UnsupportedFormatKind::FileSystem,
            detail,
        } => format!(
            "지원하지 않는 파일시스템입니다. 현재 오프라인 탐색은 NTFS 3.1만 지원합니다. ({detail})"
        ),
        VirtualDiskError::UnsupportedFormat {
            kind: UnsupportedFormatKind::PartitionTable,
            detail,
        } => format!("지원하지 않는 파티션 테이블입니다. ({detail})"),
        VirtualDiskError::GuestEntry { path, source } => {
            format!("게스트 항목 {path} 읽기 실패: {}", format_virtual_disk_error(source))
        }
        _ => format!("읽기 실패: {error}"),
    }
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
        assert!(session.selected_paths.is_empty());
        assert!(session.focus_handle.is_none());
        assert!(session.source.is_none());
        assert!(session.error.is_none());
    }

    #[test]
    fn running_vm_error_explains_the_safe_offline_boundary() {
        let message = format_virtual_disk_error(&VirtualDiskError::ReadOnlyViolation(
            "실행 중인 VM이 VDI를 사용 중입니다".to_string(),
        ));

        assert!(message.contains("실행 중인 VM"));
        assert!(message.contains("VM을 완전히 종료"));
        assert!(message.contains("직접 읽을 수 없습니다"));
    }

    #[test]
    fn unsupported_filesystem_error_names_the_supported_version() {
        let message = format_virtual_disk_error(&VirtualDiskError::UnsupportedFormat {
            kind: UnsupportedFormatKind::FileSystem,
            detail: "ext4 파티션".to_string(),
        });

        assert!(message.contains("지원하지 않는 파일시스템"));
        assert!(message.contains("NTFS 3.1"));
    }
}

fn parent_guest_path(path: &GuestPath) -> Option<GuestPath> {
    if path.is_root() {
        return None;
    }
    let parent = path
        .as_str()
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or("");
    GuestPath::new(parent).ok()
}

fn toggle_guest_selection(
    selected_paths: &mut HashSet<GuestPath>,
    path: GuestPath,
    additive: bool,
) {
    if !additive {
        selected_paths.clear();
    }
    if !selected_paths.insert(path.clone()) {
        selected_paths.remove(&path);
    }
}

fn select_all_paths(entries: &[GuestFileEntry]) -> HashSet<GuestPath> {
    entries.iter().map(|entry| entry.path.clone()).collect()
}

fn selected_directory_index(
    entries: &[GuestFileEntry],
    selected_paths: &HashSet<GuestPath>,
) -> Option<usize> {
    if selected_paths.len() != 1 {
        return None;
    }
    entries
        .iter()
        .enumerate()
        .find(|(_, entry)| entry.is_directory() && selected_paths.contains(&entry.path))
        .map(|(index, _)| index)
}

#[cfg(test)]
mod path_tests {
    use super::*;

    #[test]
    fn parent_path_stops_at_guest_root() {
        let nested = GuestPath::new("Users/guest/Documents").unwrap();
        let top = GuestPath::new("Users").unwrap();
        let root = GuestPath::root();

        assert_eq!(parent_guest_path(&nested).unwrap().as_str(), "Users/guest");
        assert_eq!(parent_guest_path(&top).unwrap().as_str(), "");
        assert!(parent_guest_path(&root).is_none());
    }

    #[test]
    fn guest_selection_supports_single_and_additive_toggle() {
        let first = GuestPath::new("first.txt").unwrap();
        let second = GuestPath::new("second.txt").unwrap();
        let mut selected = HashSet::new();

        toggle_guest_selection(&mut selected, first.clone(), false);
        assert_eq!(selected.len(), 1);
        assert!(selected.contains(&first));

        toggle_guest_selection(&mut selected, second.clone(), true);
        assert_eq!(selected.len(), 2);

        toggle_guest_selection(&mut selected, second.clone(), true);
        assert_eq!(selected.len(), 1);
        assert!(selected.contains(&first));
    }

    #[test]
    fn select_all_includes_hidden_and_system_entries() {
        let entries = vec![
            GuestFileEntry {
                path: GuestPath::new("visible.txt").unwrap(),
                kind: GuestFileKind::File,
                size_bytes: 1,
                attributes: GuestFileAttributes::default(),
                times: Default::default(),
            },
            GuestFileEntry {
                path: GuestPath::new("hidden.sys").unwrap(),
                kind: GuestFileKind::File,
                size_bytes: 2,
                attributes: GuestFileAttributes::from_bits(
                    GuestFileAttributes::HIDDEN.bits() | GuestFileAttributes::SYSTEM.bits(),
                ),
                times: Default::default(),
            },
        ];

        let selected = select_all_paths(&entries);

        assert_eq!(selected.len(), 2);
        assert!(selected.contains(&entries[0].path));
        assert!(selected.contains(&entries[1].path));
    }

    #[test]
    fn enter_shortcut_requires_exactly_one_selected_directory() {
        let entries = vec![
            GuestFileEntry {
                path: GuestPath::new("folder").unwrap(),
                kind: GuestFileKind::Directory,
                size_bytes: 0,
                attributes: GuestFileAttributes::default(),
                times: Default::default(),
            },
            GuestFileEntry {
                path: GuestPath::new("note.txt").unwrap(),
                kind: GuestFileKind::File,
                size_bytes: 1,
                attributes: GuestFileAttributes::default(),
                times: Default::default(),
            },
        ];
        let folder = entries[0].path.clone();
        let file = entries[1].path.clone();

        assert_eq!(
            selected_directory_index(&entries, &HashSet::from([folder.clone()])),
            Some(0)
        );
        assert_eq!(
            selected_directory_index(&entries, &HashSet::from([folder, file])),
            None
        );
    }
}
