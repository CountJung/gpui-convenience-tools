//! VirtualBox 탐색기 VDE-013~014 UI 회귀 테스트.

use super::*;
use crate::app::virtual_disk_copy::{VirtualDiskCopyProgress, VirtualDiskCopySummary};
use crate::virtual_disk::{
    copy::CopyIssue, issues::CopyIssueKind, GuestFileEntry, GuestFileKind, GuestFileSource,
    GuestFileSystem, VdiPartition, VirtualDiskError,
};

struct TestGuestSource {
    entries: Vec<GuestFileEntry>,
}

struct ErrorOnNestedDirectorySource;

struct TreeGuestSource;

impl GuestFileSource for TreeGuestSource {
    fn filesystem(&self) -> GuestFileSystem {
        GuestFileSystem::ntfs_3_1()
    }

    fn list_directory(
        &mut self,
        directory: &GuestFileEntry,
    ) -> Result<Vec<GuestFileEntry>, VirtualDiskError> {
        let file = |path: &str| GuestFileEntry {
            path: crate::virtual_disk::GuestPath::new(path).unwrap(),
            kind: GuestFileKind::File,
            size_bytes: 24,
            attributes: Default::default(),
            times: Default::default(),
        };
        let folder = |path: &str| GuestFileEntry {
            path: crate::virtual_disk::GuestPath::new(path).unwrap(),
            kind: GuestFileKind::Directory,
            size_bytes: 0,
            attributes: Default::default(),
            times: Default::default(),
        };

        Ok(match directory.path.as_str() {
            "" => vec![folder("Users")],
            "Users" => vec![folder("Users/Public")],
            "Users/Public" => vec![file("Users/Public/report.txt")],
            _ => Vec::new(),
        })
    }

    fn read_at(
        &mut self,
        _file: &GuestFileEntry,
        _offset: u64,
        buffer: &mut [u8],
    ) -> Result<usize, VirtualDiskError> {
        buffer.fill(0);
        Ok(buffer.len())
    }
}

impl GuestFileSource for ErrorOnNestedDirectorySource {
    fn filesystem(&self) -> GuestFileSystem {
        GuestFileSystem::ntfs_3_1()
    }

    fn list_directory(
        &mut self,
        directory: &GuestFileEntry,
    ) -> Result<Vec<GuestFileEntry>, VirtualDiskError> {
        if directory.path.is_root() {
            Ok(vec![GuestFileEntry {
                path: crate::virtual_disk::GuestPath::new("broken").unwrap(),
                kind: GuestFileKind::Directory,
                size_bytes: 0,
                attributes: Default::default(),
                times: Default::default(),
            }])
        } else {
            Err(VirtualDiskError::CorruptImage(
                "nested directory fixture failure".to_string(),
            ))
        }
    }

    fn read_at(
        &mut self,
        _file: &GuestFileEntry,
        _offset: u64,
        _buffer: &mut [u8],
    ) -> Result<usize, VirtualDiskError> {
        Ok(0)
    }
}

impl GuestFileSource for TestGuestSource {
    fn filesystem(&self) -> GuestFileSystem {
        GuestFileSystem::ntfs_3_1()
    }

    fn list_directory(
        &mut self,
        _directory: &GuestFileEntry,
    ) -> Result<Vec<GuestFileEntry>, VirtualDiskError> {
        Ok(self.entries.clone())
    }

    fn read_at(
        &mut self,
        _file: &GuestFileEntry,
        _offset: u64,
        buffer: &mut [u8],
    ) -> Result<usize, VirtualDiskError> {
        buffer.fill(0);
        Ok(buffer.len())
    }
}

fn loaded_entries() -> Vec<GuestFileEntry> {
    vec![
        GuestFileEntry {
            path: crate::virtual_disk::GuestPath::new(".hidden.sys").unwrap(),
            kind: GuestFileKind::File,
            size_bytes: 12,
            attributes: crate::virtual_disk::GuestFileAttributes::from_bits(
                crate::virtual_disk::GuestFileAttributes::HIDDEN.bits()
                    | crate::virtual_disk::GuestFileAttributes::SYSTEM.bits(),
            ),
            times: Default::default(),
        },
        GuestFileEntry {
            path: crate::virtual_disk::GuestPath::new("visible.txt").unwrap(),
            kind: GuestFileKind::File,
            size_bytes: 24,
            attributes: Default::default(),
            times: Default::default(),
        },
    ]
}

fn loaded_virtual_disk_root() -> AppRoot {
    let mut root = test_app_root(ActivePanel::VirtualDisk);
    let entries = loaded_entries();
    root.virtual_disk.vdi_path = Some(std::path::PathBuf::from("fixture.vdi"));
    root.virtual_disk.partitions.push(VdiPartition {
        number: 1,
        table: crate::virtual_disk::PartitionTableKind::Mbr,
        start_lba: 1,
        sector_count: 4096,
        filesystem: Some(GuestFileSystem::ntfs_3_1()),
    });
    root.virtual_disk.selected_partition = Some(0);
    root.virtual_disk.entries = entries.clone();
    root.virtual_disk.source = Some(Box::new(TestGuestSource { entries }));
    root
}

#[gpui::test]
fn virtual_disk_panel_registers_navigation_and_renders_read_only_shell(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (_view, cx) = cx.add_window_view(|_, _| test_app_root(ActivePanel::VirtualDisk));

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    refresh(cx);

    assert!(
        cx.debug_bounds("nav-item-VirtualBox 디스크 탐색").is_some(),
        "VirtualBox explorer should be registered in the tools navigation"
    );
    assert!(
        cx.debug_bounds("virtual-disk-source-card").is_some(),
        "VDI source card should be rendered"
    );
    assert!(
        cx.debug_bounds("virtual-disk-path-input").is_some(),
        "VDI path input should be rendered"
    );
    assert!(
        cx.debug_bounds("virtual-disk-browse").is_some(),
        "native VDI file picker should be rendered"
    );
    assert!(
        cx.debug_bounds("virtual-disk-partitions-card").is_some(),
        "partition selection card should be rendered"
    );
    assert!(
        cx.debug_bounds("virtual-disk-current-path").is_some(),
        "current guest path should be rendered"
    );
    assert!(
        cx.debug_bounds("virtual-disk-tree-scroll").is_some(),
        "folder tree should be rendered"
    );
    assert!(
        cx.debug_bounds("virtual-disk-entry-scroll").is_some(),
        "file list should be rendered"
    );
    assert!(
        cx.debug_bounds("virtual-disk-layout-controls").is_some(),
        "file list layout controls should be rendered"
    );
    assert!(
        cx.debug_bounds("virtual-disk-refresh").is_some(),
        "directory refresh action should be rendered"
    );
    assert!(
        cx.debug_bounds("virtual-disk-parent").is_some(),
        "parent directory action should be rendered"
    );
    assert!(
        cx.debug_bounds("virtual-disk-copy-card").is_some(),
        "copy card should be rendered"
    );
    assert!(
        cx.debug_bounds("virtual-disk-target-input").is_some(),
        "copy target input should be rendered"
    );
    assert!(
        cx.debug_bounds("virtual-disk-copy-start").is_some(),
        "copy start action should be rendered"
    );
    assert!(
        cx.debug_bounds("virtual-disk-safety-notice").is_some(),
        "read-only safety boundary should be rendered"
    );
}

#[gpui::test]
fn virtual_disk_folder_tree_navigates_nested_paths_without_repeated_list_clicks(
    cx: &mut TestAppContext,
) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = test_app_root(ActivePanel::VirtualDisk);
        root.virtual_disk.vdi_path = Some(std::path::PathBuf::from("fixture.vdi"));
        root.virtual_disk.source = Some(Box::new(TreeGuestSource));
        root
    });

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    refresh(cx);
    cx.update(|_, app| {
        view.update(app, |root, cx| root.refresh_virtual_disk_directory(cx));
    });
    refresh(cx);

    wheel_to_end(cx, "virtual-disk-page", -600.0);
    assert!(cx.debug_bounds("virtual-disk-tree-node-Users").is_some());
    click_debug_element(cx, "virtual-disk-tree-node-Users");

    let (current_path, entries) = cx.update(|_, app| {
        let root = view.read(app);
        (
            root.virtual_disk.current_path.to_string(),
            root.virtual_disk
                .entries
                .iter()
                .map(|entry| entry.path.to_string())
                .collect::<Vec<_>>(),
        )
    });
    assert_eq!(current_path, "Users");
    assert_eq!(
        entries,
        vec!["Users/Public"],
        "tree navigation should load only immediate child folders"
    );
    assert!(
        !entries.iter().any(|path| path == "Users/Public/report.txt"),
        "tree navigation must not enumerate descendants before their folder is selected"
    );
    assert!(cx.debug_bounds("virtual-disk-tree-node-Users-Public").is_some());
}

#[gpui::test]
fn virtual_disk_explorer_keeps_tree_and_file_list_inside_compact_card(
    cx: &mut TestAppContext,
) {
    initialize_components(cx);
    let (_view, cx) = cx.add_window_view(|_, _| loaded_virtual_disk_root());

    for width in [MIN_SUPPORTED_WINDOW_WIDTH, DEFAULT_WINDOW_WIDTH, 1280.0] {
        cx.simulate_resize(size(px(width), px(DEFAULT_WINDOW_HEIGHT)));
        refresh(cx);

        let card = cx
            .debug_bounds("virtual-disk-directory-card")
            .expect("directory card should be rendered");
        let tree = cx
            .debug_bounds("virtual-disk-tree-scroll")
            .expect("tree scroll should be rendered");
        let entries = cx
            .debug_bounds("virtual-disk-entry-scroll")
            .expect("entry scroll should be rendered");
        let card_right = card.origin.x + card.size.width;
        let card_bottom = card.origin.y + card.size.height;

        assert!(
            tree.origin.x >= card.origin.x
                && tree.origin.x + tree.size.width <= card_right
                && tree.origin.y >= card.origin.y
                && tree.origin.y + tree.size.height <= card_bottom,
            "folder tree should stay inside compact directory card: card={card:?}, tree={tree:?}"
        );
        assert!(
            entries.origin.x >= card.origin.x
                && entries.origin.x + entries.size.width <= card_right
                && entries.origin.y >= card.origin.y
                && entries.origin.y + entries.size.height <= card_bottom,
            "file list should stay inside compact directory card: card={card:?}, entries={entries:?}"
        );
        let tree_width = tree.size.width;
        assert!(
            tree_width <= px(220.0),
            "default folder tree should prioritize the file list: tree={tree:?}"
        );
    }
}

#[gpui::test]
fn virtual_disk_layout_controls_update_and_reset_readable_column_widths(
    cx: &mut TestAppContext,
) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| loaded_virtual_disk_root());

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    refresh(cx);

    let before = cx.update(|_, app| {
        let root = view.read(app);
        (
            root.virtual_disk.layout.attributes_width,
            root.virtual_disk.layout.size_width,
        )
    });
    click_debug_element(cx, "virtual-disk-layout-attributes-increase");
    click_debug_element(cx, "virtual-disk-layout-size-increase");
    let increased = cx.update(|_, app| {
        let root = view.read(app);
        (
            root.virtual_disk.layout.attributes_width,
            root.virtual_disk.layout.size_width,
        )
    });
    assert_eq!(increased.0, before.0 + 8.0);
    assert_eq!(increased.1, before.1 + 8.0);

    click_debug_element(cx, "virtual-disk-layout-reset");
    let reset = cx.update(|_, app| view.read(app).virtual_disk.layout.clone());
    assert_eq!(reset, Default::default());
}

#[gpui::test]
fn virtual_disk_tree_width_setting_is_clamped_and_persisted(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| loaded_virtual_disk_root());

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    refresh(cx);

    cx.update(|_, app| {
        view.update(app, |root, cx| {
            root.set_virtual_disk_tree_width(228.0, cx);
        });
    });
    refresh(cx);

    let width = cx.update(|_, app| view.read(app).virtual_disk.layout.tree_width);
    assert!(
        (width - 228.0).abs() < f32::EPSILON,
        "tree divider width should be retained in the session layout: {width}"
    );

    cx.update(|_, app| {
        view.update(app, |root, cx| {
            root.set_virtual_disk_tree_width(999.0, cx);
        });
    });
    let clamped = cx.update(|_, app| view.read(app).virtual_disk.layout.tree_width);
    assert_eq!(clamped, 240.0);
}

#[gpui::test]
fn virtual_disk_panel_explains_unsupported_partition_state(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (_view, cx) = cx.add_window_view(|_, _| {
        let mut root = test_app_root(ActivePanel::VirtualDisk);
        root.virtual_disk.error = Some(
            "접근 차단: 실행 중인 VM이 사용 중이거나 잠금 상태인 VDI는 직접 읽을 수 없습니다."
                .to_string(),
        );
        root.virtual_disk
            .partitions
            .push(crate::virtual_disk::VdiPartition {
                number: 1,
                table: crate::virtual_disk::PartitionTableKind::Gpt,
                start_lba: 2048,
                sector_count: 4096,
                filesystem: None,
            });
        root
    });

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    refresh(cx);

    assert!(
        cx.debug_bounds("virtual-disk-safety-notice").is_some(),
        "safety boundary should remain visible when an error is shown"
    );
    assert!(
        cx.debug_bounds("virtual-disk-error").is_some(),
        "categorized access error should be visible"
    );
    assert!(
        cx.debug_bounds("virtual-disk-partition-warning-0")
            .is_some(),
        "unsupported filesystem guidance should be visible"
    );
}

#[gpui::test]
fn virtual_disk_panel_renders_loaded_hidden_entries_and_selects_all_with_ctrl_a(
    cx: &mut TestAppContext,
) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| loaded_virtual_disk_root());

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    refresh(cx);

    wheel_to_end(cx, "virtual-disk-page", -600.0);
    assert!(cx.debug_bounds("virtual-disk-entry-0").is_some());
    assert!(cx.debug_bounds("virtual-disk-entry-1").is_some());
    assert!(cx.debug_bounds("virtual-disk-column-attributes").is_some());

    click_debug_element(cx, "virtual-disk-entry-0");
    cx.simulate_keystrokes("ctrl-a");
    refresh(cx);

    let selected_count = cx.update(|_, app| view.read(app).virtual_disk.selected_paths.len());
    assert_eq!(
        selected_count, 2,
        "Ctrl+A should include hidden/system entries"
    );
}

#[gpui::test]
fn virtual_disk_directory_row_double_click_enters_directory_and_refreshes_entries(
    cx: &mut TestAppContext,
) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = test_app_root(ActivePanel::VirtualDisk);
        root.virtual_disk.entries = vec![GuestFileEntry {
            path: crate::virtual_disk::GuestPath::new("broken").unwrap(),
            kind: GuestFileKind::Directory,
            size_bytes: 0,
            attributes: Default::default(),
            times: Default::default(),
        }];
        root.virtual_disk.source = Some(Box::new(ErrorOnNestedDirectorySource));
        root
    });

    cx.simulate_resize(size(px(1200.0), px(1000.0)));
    refresh(cx);
    let bounds = cx
        .debug_bounds("virtual-disk-entry-0")
        .expect("directory row should be rendered");
    let position = point(
        bounds.origin.x + bounds.size.width / 2.0,
        bounds.origin.y + bounds.size.height / 2.0,
    );

    cx.simulate_event(gpui::MouseDownEvent {
        position,
        button: gpui::MouseButton::Left,
        modifiers: gpui::Modifiers::none(),
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_event(gpui::MouseUpEvent {
        position,
        button: gpui::MouseButton::Left,
        modifiers: gpui::Modifiers::none(),
        click_count: 2,
    });
    refresh(cx);

    let (entry_count, error, current_path) = cx.update(|_, app| {
        let root = view.read(app);
        (
            root.virtual_disk.entries.len(),
            root.virtual_disk.error.clone(),
            root.virtual_disk.current_path.to_string(),
        )
    });
    assert_eq!(current_path, "broken");
    assert_eq!(entry_count, 0, "double-click entry should refresh the folder");
    assert!(error
        .as_deref()
        .is_some_and(|message| message.contains("손상")));
}

#[gpui::test]
fn virtual_disk_clears_stale_entries_when_directory_refresh_fails(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = test_app_root(ActivePanel::VirtualDisk);
        root.virtual_disk.entries = vec![GuestFileEntry {
            path: crate::virtual_disk::GuestPath::new("broken").unwrap(),
            kind: GuestFileKind::Directory,
            size_bytes: 0,
            attributes: Default::default(),
            times: Default::default(),
        }];
        root.virtual_disk.source = Some(Box::new(ErrorOnNestedDirectorySource));
        root
    });

    cx.simulate_resize(size(px(1200.0), px(1000.0)));
    refresh(cx);
    cx.update(|_, app| {
        view.update(app, |root, cx| {
            root.enter_virtual_disk_directory(0, cx);
        });
    });
    refresh(cx);

    let (entry_count, error, current_path) = cx.update(|_, app| {
        let root = view.read(app);
        (
            root.virtual_disk.entries.len(),
            root.virtual_disk.error.clone(),
            root.virtual_disk.current_path.to_string(),
        )
    });
    assert_eq!(entry_count, 0, "failed directory must not keep stale rows");
    assert!(error
        .as_deref()
        .is_some_and(|message| message.contains("손상")));
    assert_eq!(current_path, "broken");
}

#[gpui::test]
fn virtual_disk_panel_renders_copy_progress_and_issue_summary(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = loaded_virtual_disk_root();
        root.virtual_disk.copy.progress = Some(VirtualDiskCopyProgress {
            total_entries: 2,
            completed_entries: 1,
            current_path: ".hidden.sys".to_string(),
            copied_files: 1,
            copied_bytes: 12,
            skipped_entries: 0,
            failed_entries: 0,
            stopping: true,
        });
        root
    });

    cx.simulate_resize(size(px(1200.0), px(1000.0)));
    refresh(cx);
    assert!(cx.debug_bounds("virtual-disk-copy-progress").is_some());
    assert!(cx.debug_bounds("virtual-disk-copy-stop").is_some());

    cx.update(|_, app| {
        view.update(app, |root, cx| {
            root.virtual_disk.copy.progress = None;
            root.virtual_disk.copy.summary = Some(VirtualDiskCopySummary {
                failed_entries: 1,
                issues: vec![CopyIssue {
                    path: ".hidden.sys".to_string(),
                    kind: CopyIssueKind::Unsupported,
                    detail: "압축된 스트림".to_string(),
                }],
                ..VirtualDiskCopySummary::default()
            });
            cx.notify();
        });
    });
    refresh(cx);
    wheel_to_end(cx, "virtual-disk-page", -1200.0);

    assert!(cx.debug_bounds("virtual-disk-copy-summary").is_some());
    assert!(cx.debug_bounds("virtual-disk-copy-start").is_some());
    assert!(
        cx.debug_bounds("virtual-disk-issue-suppress-0").is_some(),
        "each copy issue should expose a repeat-notification suppression action"
    );
    click_debug_element(cx, "virtual-disk-issue-suppress-0");
    let suppressed = cx.update(|_, app| {
        view.read(app)
            .virtual_disk
            .suppressed_issue_keys
            .contains("지원 불가:.hidden.sys")
    });
    assert!(
        suppressed,
        "clicking the issue action should suppress repeat notifications"
    );

    click_debug_element(cx, "virtual-disk-issue-suppress-0");
    let unsuppressed = cx.update(|_, app| {
        !view
            .read(app)
            .virtual_disk
            .suppressed_issue_keys
            .contains("지원 불가:.hidden.sys")
    });
    assert!(
        unsuppressed,
        "clicking the issue action again should restore repeat notifications"
    );
}

#[gpui::test]
fn virtual_disk_panel_dispatches_explorer_shortcuts_when_directory_is_focused(
    cx: &mut TestAppContext,
) {
    initialize_components(cx);
    let (_view, cx) = cx.add_window_view(|_, _| test_app_root(ActivePanel::VirtualDisk));

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    refresh(cx);
    click_debug_element(cx, "virtual-disk-directory-card");

    cx.simulate_keystrokes("ctrl-a f5 enter backspace ctrl-c");
    refresh(cx);

    assert!(
        cx.debug_bounds("virtual-disk-directory-card").is_some(),
        "directory container should remain available after shortcut dispatch"
    );
    assert!(
        cx.debug_bounds("virtual-disk-copy-card").is_some(),
        "copy panel should remain available after Ctrl+C shortcut dispatch"
    );
}
