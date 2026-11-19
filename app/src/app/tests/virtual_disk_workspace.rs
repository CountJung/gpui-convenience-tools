//! VDE-026: 탐색 중심 화면의 공간 배분과 단계 왕복.
use super::virtual_disk::loaded_virtual_disk_root;
use super::*;
use crate::app::virtual_disk_copy::{VirtualDiskCopyProgress, VirtualDiskCopySummary};
use crate::virtual_disk::{copy::CopyIssue, issues::CopyIssueKind};

#[gpui::test]
fn virtual_disk_workspace_scrolls_columns_together_without_resizing_saved_layout(
    cx: &mut TestAppContext,
) {
    use crate::{config::VirtualDiskLayoutConfig, virtual_disk::GuestPath};
    let _dir = crate::config::tests::IsolatedDataDir::new("vdi-column-scroll");
    let expected = VirtualDiskLayoutConfig {
        tree_width: 240.,
        kind_width: 104.,
        attributes_width: 176.,
        size_width: 144.,
    };
    let long_path = GuestPath::new(format!(".{}.sys", "긴-숨김-파일명-".repeat(12))).unwrap();
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = loaded_virtual_disk_root();
        root.virtual_disk.entries[0].path = long_path.clone();
        root.virtual_disk.layout = expected.clone();
        root.sidebar_width = 360.;
        root.sync.external_side_effects_enabled = true;
        root
    });
    cx.simulate_resize(size(px(920.), px(480.)));
    refresh(cx);
    let viewport = cx.debug_bounds("virtual-disk-entry-scroll").unwrap();
    let tree = cx.debug_bounds("virtual-disk-tree-scroll").unwrap();
    let copy = cx.debug_bounds("virtual-disk-copy-card").unwrap();
    let name = cx.debug_bounds("virtual-disk-name-0").unwrap();
    assert!(
        name.size.width >= px(140.)
            && name.origin.x >= viewport.origin.x
            && name.right() <= viewport.right()
    );
    assert!(
        viewport.right()
            <= cx
                .debug_bounds("virtual-disk-directory-card")
                .unwrap()
                .right()
    );
    assert!(cx.debug_bounds("virtual-disk-column-size").unwrap().right() > viewport.right());
    cx.simulate_event(ScrollWheelEvent {
        position: viewport.center(),
        delta: ScrollDelta::Pixels(point(px(-2000.), px(0.))),
        ..Default::default()
    });
    refresh(cx);
    let header = cx.debug_bounds("virtual-disk-column-size").unwrap();
    let value = cx.debug_bounds("virtual-disk-size-0").unwrap();
    assert_eq!(
        (header.origin.x, header.size.width),
        (value.origin.x, px(144.))
    );
    assert_eq!(header.size.width, value.size.width);
    assert!(value.origin.x >= viewport.origin.x && value.right() <= viewport.right());
    assert_eq!(tree, cx.debug_bounds("virtual-disk-tree-scroll").unwrap());
    assert_eq!(copy, cx.debug_bounds("virtual-disk-copy-card").unwrap());
    click_debug_element(cx, "virtual-disk-size-0");
    assert!(view.read_with(cx, |root, _| root
        .virtual_disk
        .selected_paths
        .contains(&long_path)));
    cx.simulate_resize(size(px(1600.), px(900.)));
    refresh(cx);
    let wide = cx.debug_bounds("virtual-disk-entry-scroll").unwrap();
    let expanded = cx.debug_bounds("virtual-disk-name-0").unwrap();
    assert!(expanded.size.width > name.size.width && expanded.origin.x >= wide.origin.x);
    assert!(cx.debug_bounds("virtual-disk-size-0").unwrap().right() <= wide.right());
    view.read_with(cx, |root, _| assert_eq!(root.virtual_disk.layout, expected));
    view.update(cx, |root, _| root.save_current_config().unwrap());
    assert_eq!(
        crate::config::load_config()
            .unwrap()
            .unwrap()
            .virtual_disk
            .layout,
        expected
    );
}

#[gpui::test]
fn virtual_disk_workspace_prioritizes_files_at_all_supported_sizes(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (_view, cx) = cx.add_window_view(|_, _| loaded_virtual_disk_root());
    for (width, height) in [(920.0, 480.0), (1000.0, 700.0), (1600.0, 1000.0)] {
        cx.simulate_resize(size(px(width), px(height)));
        refresh(cx);
        let page = cx.debug_bounds("virtual-disk-page").unwrap();
        let explorer = cx.debug_bounds("virtual-disk-directory-card").unwrap();
        let list = cx.debug_bounds("virtual-disk-entry-scroll").unwrap();
        let footer = cx.debug_bounds("virtual-disk-copy-card").unwrap();
        assert!(cx.debug_bounds("virtual-disk-source-card").is_none());
        assert!(cx.debug_bounds("virtual-disk-partitions-card").is_none());
        assert!(cx.debug_bounds("virtual-disk-safety-notice").is_none());
        assert!(cx.debug_bounds("virtual-disk-layout-controls").is_none());
        assert!(
            explorer.size.height >= page.size.height * 0.65,
            "explorer must receive most of the main pane: page={page:?}, explorer={explorer:?}"
        );
        assert!(
            list.size.height >= page.size.height * 0.55,
            "visible file list must dominate: page={page:?}, list={list:?}"
        );
        assert!(list.origin.y >= page.origin.y);
        assert!(list.origin.y + list.size.height <= footer.origin.y);
        assert!(footer.origin.y + footer.size.height <= page.origin.y + page.size.height + px(1.0));
        assert!(explorer.size.width >= page.size.width - px(1.0));
    }
}

#[gpui::test]
fn virtual_disk_workspace_returns_to_setup_without_losing_selection(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| loaded_virtual_disk_root());
    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    refresh(cx);
    click_debug_element(cx, "virtual-disk-entry-0");
    let selected = cx.update(|_, app| view.read(app).virtual_disk.selected_paths.clone());
    assert_eq!(selected.len(), 1);
    click_debug_element(cx, "virtual-disk-change-source");
    assert!(cx.debug_bounds("virtual-disk-source-card").is_some());
    // GPUI 0.2.2는 제거된 selector bounds를 다음 프레임에도 남긴다.
    // 클릭으로 바뀐 상태를 단언하고, 최초 렌더의 부재는 별도 신규 창에서 검증한다.
    assert!(cx.update(|_, app| view.read(app).virtual_disk.setup_visible));
    assert!(cx.update(|_, app| view.read(app).virtual_disk.source.is_some()));
    click_debug_element(cx, "virtual-disk-resume");
    assert!(!cx.update(|_, app| view.read(app).virtual_disk.setup_visible));
    assert!(cx.debug_bounds("virtual-disk-directory-card").is_some());
    assert_eq!(
        cx.update(|_, app| view.read(app).virtual_disk.selected_paths.clone()),
        selected
    );
    click_debug_element(cx, "virtual-disk-layout-toggle");
    assert!(cx.debug_bounds("virtual-disk-layout-controls").is_some());
    click_debug_element(cx, "virtual-disk-layout-toggle");
    assert!(!cx.update(|_, app| view.read(app).virtual_disk.layout_controls_visible));
}

#[gpui::test]
fn virtual_disk_workspace_error_and_copy_details_remain_accessible(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = loaded_virtual_disk_root();
        root.virtual_disk.error = Some("폴더 읽기 실패".to_string());
        root.virtual_disk.copy.summary = Some(VirtualDiskCopySummary {
            failed_entries: 10,
            issues: (0..10)
                .map(|index| CopyIssue {
                    path: format!("file-{index}.txt"),
                    kind: CopyIssueKind::Unsupported,
                    detail: "지원하지 않는 스트림".to_string(),
                })
                .collect(),
            ..Default::default()
        });
        root
    });
    cx.simulate_resize(size(
        px(MIN_SUPPORTED_WINDOW_WIDTH),
        px(COMPACT_WINDOW_HEIGHT),
    ));
    refresh(cx);
    assert!(cx.debug_bounds("virtual-disk-error").is_some());
    assert!(cx.debug_bounds("virtual-disk-copy-details").is_none());
    click_debug_element(cx, "virtual-disk-copy-details-toggle");
    let page = cx.debug_bounds("virtual-disk-page").unwrap();
    let details = cx.debug_bounds("virtual-disk-copy-details").unwrap();
    let list = cx.debug_bounds("virtual-disk-entry-scroll").unwrap();
    assert!(details.origin.y + details.size.height <= page.origin.y + page.size.height + px(1.0));
    assert!(
        list.size.height >= px(120.0),
        "details must not replace the explorer: {list:?}"
    );
    let before = cx.debug_bounds("virtual-disk-issue-suppress-9").unwrap();
    wheel_to_end(cx, "virtual-disk-copy-details-scroll", -900.0);
    let last = cx.debug_bounds("virtual-disk-issue-suppress-9").unwrap();
    assert!(last.origin.y < before.origin.y);
    assert!(last.origin.y >= details.origin.y);
    assert!(last.origin.y + last.size.height <= details.origin.y + details.size.height);
    click_debug_element(cx, "virtual-disk-copy-details-toggle");
    assert!(!cx.update(|_, app| view.read(app).virtual_disk.copy_details_visible));
}

#[gpui::test]
fn virtual_disk_workspace_keeps_stop_visible_with_long_error_while_copying(
    cx: &mut TestAppContext,
) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = loaded_virtual_disk_root();
        root.virtual_disk.error = Some(
            (0..30)
                .map(|index| format!("긴 경로/{index}/폴더 읽기 실패\n"))
                .collect(),
        );
        root.virtual_disk.copy.progress = Some(VirtualDiskCopyProgress::default());
        root
    });
    cx.simulate_resize(size(
        px(MIN_SUPPORTED_WINDOW_WIDTH),
        px(COMPACT_WINDOW_HEIGHT),
    ));
    refresh(cx);
    let page = cx.debug_bounds("virtual-disk-page").unwrap();
    let error = cx.debug_bounds("virtual-disk-error").unwrap();
    let list = cx.debug_bounds("virtual-disk-entry-scroll").unwrap();
    let stop = cx.debug_bounds("virtual-disk-copy-stop").unwrap();
    assert!(error.size.height <= px(48.0));
    assert!(
        list.size.height >= px(120.0),
        "error must not squeeze out files: {list:?}"
    );
    assert!(stop.origin.y + stop.size.height <= page.origin.y + page.size.height);
    let error_before = cx.update(|_, app| view.read(app).virtual_disk.error_scroll.offset());
    wheel_to_end(cx, "virtual-disk-error", -900.0);
    let error_after = cx.update(|_, app| view.read(app).virtual_disk.error_scroll.offset());
    assert!(
        error_after.y < error_before.y,
        "full error must remain scrollable"
    );
    click_debug_element(cx, "virtual-disk-change-source");
    assert!(!cx.update(|_, app| view.read(app).virtual_disk.setup_visible));
    click_debug_element(cx, "virtual-disk-copy-stop");
    assert!(cx.update(|_, app| view
        .read(app)
        .virtual_disk
        .copy
        .cancel
        .load(AtomicOrdering::SeqCst)));
    assert!(cx.update(|_, app| view
        .read(app)
        .virtual_disk
        .copy
        .progress
        .as_ref()
        .unwrap()
        .stopping));
    assert!(!cx.update(|_, app| view.read(app).virtual_disk.setup_visible));
}
