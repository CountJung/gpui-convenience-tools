//! VirtualBox 탐색기 VDE-013 UI 회귀 테스트.

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
}
