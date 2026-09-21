//! VirtualBox 탐색기 VDE-013~014 UI 회귀 테스트.

use super::*;

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
        cx.debug_bounds("virtual-disk-partitions-card").is_some(),
        "partition selection card should be rendered"
    );
    assert!(
        cx.debug_bounds("virtual-disk-current-path").is_some(),
        "current guest path should be rendered"
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
