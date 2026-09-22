//! VirtualBox 오프라인 VDI 탐색 패널.
//!
//! VDE-013~019 범위는 VDI 경로 입력, 파티션 선택, 현재 게스트 경로, 목록 표시,
//! 폴더 이동·복사·키보드 단축키와 안전/지원 상태 안내를 제공한다. 실제 이미지 E2E는
//! VDE-019에서 검증한다.

use gpui::{
    div, px, AnyElement, ClickEvent, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{h_flex, input::Input, theme::ActiveTheme, v_flex, PixelsExt};

use crate::app::{
    AppRoot, CopySelected, EnterSelected, GuestDirectoryTreeNode, ParentDirectory, Refresh,
    SelectAll,
    VIRTUAL_DISK_KEY_CONTEXT,
};
use crate::config::{
    normalize_virtual_disk_attributes_width, normalize_virtual_disk_kind_width,
    normalize_virtual_disk_size_width, normalize_virtual_disk_tree_width,
};
use crate::virtual_disk::{
    GuestFileAttributes, GuestFileKind, GuestFileSystem, PartitionTableKind,
};

use super::{resizable_split, scroll_pane};
use super::ui::{self, ButtonStyle};

pub fn render(this: &mut AppRoot, window: &mut Window, cx: &mut Context<AppRoot>) -> AnyElement {
    this.ensure_virtual_disk_input(window, cx);
    this.ensure_virtual_disk_target_input(window, cx);
    this.ensure_virtual_disk_focus(cx);
    this.schedule_validation_virtual_disk_copy(window, cx);
    if this.virtual_disk.copy.validation_auto_copy_scheduled
        && this.virtual_disk.copy.summary.is_some()
    {
        this.virtual_disk_page_scroll.scroll_to_bottom();
    }

    let page_scroll = this.virtual_disk_page_scroll.clone();
    scroll_pane(
        "virtual-disk-page",
        &page_scroll,
        v_flex()
            .w_full()
            .gap_4()
            .p_1()
            .pr_3()
            .child(render_header(this, cx))
            .child(render_safety_notice(cx))
            .child(render_source_card(this, cx))
            .child(render_partition_card(this, cx))
            .child(render_directory_card(this, cx))
            .child(render_copy_card(this, cx))
            .into_any_element(),
    )
}

fn render_safety_notice(cx: &Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();
    v_flex()
        .debug_selector(|| "virtual-disk-safety-notice".to_string())
        .w_full()
        .min_w_0()
        .gap_1()
        .rounded_lg()
        .p_3()
        .bg(theme.warning)
        .text_color(theme.warning_foreground)
        .child(div().child("안전 경계: 원본 VDI는 항상 읽기 전용으로만 엽니다."))
        .child(div().child(
            "실행 중인 VM이 사용 중인 디스크, 잠금 표식이 있는 디스크는 직접 읽지 않습니다. VM을 종료한 뒤 다시 시도하세요.",
        ))
        .into_any_element()
}

fn render_header(this: &AppRoot, cx: &Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();
    h_flex()
        .w_full()
        .justify_between()
        .items_center()
        .child(
            v_flex()
                .gap_1()
                .min_w_0()
                .child(
                    div()
                        .text_color(theme.foreground)
                        .child("VirtualBox 디스크 탐색"),
                )
                .child(div().text_color(theme.muted_foreground).child(
                    if this.virtual_disk.vdi_path.is_some() {
                        "오프라인 · 읽기 전용으로 연결됨"
                    } else {
                        "종료된 VM의 VDI만 열 수 있습니다"
                    },
                )),
        )
        .child(ui::badge(
            "원본 쓰기 금지",
            ui::Tone::Warning,
            ui::Size::Sm,
            cx,
        ))
        .into_any_element()
}

fn render_source_card(this: &mut AppRoot, cx: &mut Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();
    let input = this.virtual_disk.path_input.as_ref().map(|input| {
        div()
            .debug_selector(|| "virtual-disk-path-input".to_string())
            .flex_1()
            .min_w_0()
            .child(Input::new(input))
    });

    let mut card = v_flex()
        .debug_selector(|| "virtual-disk-source-card".to_string())
        .w_full()
        .gap_3()
        .rounded_lg()
        .p_3()
        .bg(theme.secondary)
        .border_1()
        .border_color(theme.border)
        .child(div().text_color(theme.foreground).child("원본 VDI"))
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .gap_2()
                .items_center()
                .children(input)
                .child(
                    div()
                        .debug_selector(|| "virtual-disk-browse".to_string())
                        .child(ui::action_button(
                            "virtual-disk-browse-action",
                            "찾아보기",
                            ui::Size::Md,
                            ButtonStyle::neutral(cx),
                            cx.listener(|this, _event, window, cx| {
                                this.pick_virtual_disk_file(window, cx);
                            }),
                        )),
                )
                .child(ui::action_button(
                    "virtual-disk-open",
                    "VDI 열기",
                    ui::Size::Md,
                    ButtonStyle::primary(cx),
                    cx.listener(|this, _event, _window, cx| {
                        this.open_virtual_disk(cx);
                    }),
                )),
        )
        .child(
            div()
                .text_color(theme.muted_foreground)
                .child("VDI 파일은 읽기 전용으로 열고, 실행 중인 VM의 디스크는 거부합니다."),
        );

    if let Some(error) = &this.virtual_disk.error {
        card = card.child(
            div()
                .debug_selector(|| "virtual-disk-error".to_string())
                .rounded_md()
                .px_3()
                .py_2()
                .bg(theme.danger)
                .text_color(theme.danger_foreground)
                .child(error.clone()),
        );
    }

    card.into_any_element()
}

fn render_partition_card(this: &mut AppRoot, cx: &mut Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();
    let mut list = v_flex().w_full().gap_2();

    if this.virtual_disk.partitions.is_empty() {
        list = list.child(
            div()
                .text_color(theme.muted_foreground)
                .child("VDI를 열면 검색된 파티션이 여기에 표시됩니다."),
        );
    } else {
        for (index, partition) in this.virtual_disk.partitions.iter().enumerate() {
            let selected = this.virtual_disk.selected_partition == Some(index);
            let label = format_partition_label(partition);
            let selector_id = format!("virtual-disk-partition-{index}");
            let mut partition_row = v_flex().w_full().min_w_0().gap_1().child(
                div()
                    .debug_selector(move || selector_id.clone())
                    .child(ui::action_button(
                        ("virtual-disk-partition", index),
                        label,
                        ui::Size::Md,
                        if selected {
                            ButtonStyle::primary(cx)
                        } else {
                            ButtonStyle::neutral(cx)
                        },
                        cx.listener(move |this, _event, _window, cx| {
                            this.select_virtual_disk_partition(index, cx);
                        }),
                    )),
            );
            if let Some(message) = partition_support_message(partition) {
                partition_row = partition_row.child(
                    div()
                        .debug_selector(move || format!("virtual-disk-partition-warning-{index}"))
                        .text_color(theme.warning)
                        .child(message),
                );
            }
            list = list.child(partition_row);
        }
    }

    v_flex()
        .debug_selector(|| "virtual-disk-partitions-card".to_string())
        .w_full()
        .gap_3()
        .rounded_lg()
        .p_3()
        .bg(theme.secondary)
        .border_1()
        .border_color(theme.border)
        .child(div().text_color(theme.foreground).child("파티션 선택"))
        .child(list)
        .into_any_element()
}

fn render_directory_card(this: &mut AppRoot, cx: &mut Context<AppRoot>) -> AnyElement {
    this.ensure_virtual_disk_tree_from_current_entries();
    let focus_handle = this
        .virtual_disk
        .focus_handle
        .clone()
        .expect("virtual disk focus handle must be initialized before rendering");
    let current_path = this.virtual_disk.current_path.to_string();
    let selected_count = this.virtual_disk.selected_paths.len();
    let tree = render_directory_tree(this, cx);
    let entries = render_directory_entries(this, &focus_handle, cx);
    let theme = cx.theme();
    let explorer_focus_handle = focus_handle.clone();
    let tree_width = normalize_virtual_disk_tree_width(this.virtual_disk.layout.tree_width);
    let layout_owner = cx.entity();
    let explorer = div()
        .id("virtual-disk-explorer-focus")
        .w_full()
        .min_w_0()
        .min_h_0()
        .on_click(cx.listener(move |_this, _event, window, _cx| {
            window.focus(&explorer_focus_handle);
        }))
        .child(resizable_split(
            "virtual-disk-explorer",
            px(tree_width),
            px(150.0)..px(240.0),
            px(440.0),
            tree,
            entries,
            move |state, _window, app| {
                let Some(width) = state.read(app).sizes().first().map(|size| size.as_f32())
                else {
                    return;
                };
                layout_owner.update(app, |this, cx| {
                    this.set_virtual_disk_tree_width(width, cx);
                });
            },
        ));

    v_flex()
        .debug_selector(|| "virtual-disk-directory-card".to_string())
        .key_context(VIRTUAL_DISK_KEY_CONTEXT)
        .track_focus(&focus_handle)
        .id("virtual-disk-directory-focus")
        .on_click({
            let click_focus_handle = focus_handle.clone();
            cx.listener(move |_this, _event, window, _cx| {
                window.focus(&click_focus_handle);
            })
        })
        .on_action(cx.listener(|this, _: &CopySelected, window, cx| {
            this.start_virtual_disk_copy(window, cx);
        }))
        .on_action(cx.listener(|this, _: &EnterSelected, _window, cx| {
            this.enter_selected_virtual_disk_directory(cx);
        }))
        .on_action(cx.listener(|this, _: &ParentDirectory, _window, cx| {
            this.go_to_virtual_disk_parent(cx);
        }))
        .on_action(cx.listener(|this, _: &SelectAll, _window, cx| {
            this.select_all_virtual_disk_entries(cx);
        }))
        .on_action(cx.listener(|this, _: &Refresh, _window, cx| {
            this.refresh_virtual_disk_directory(cx);
            cx.notify();
        }))
        .w_full()
        .min_w_0()
        .min_h_0()
        .h(px(260.0))
        .gap_3()
        .rounded_lg()
        .p_2()
        .bg(theme.secondary)
        .border_1()
        .border_color(theme.border)
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .debug_selector(|| "virtual-disk-current-path".to_string())
                        .flex_1()
                        .min_w_0()
                        .text_color(theme.foreground)
                        .child(format!(
                            "현재 경로: {current_path} · {selected_count}개 선택"
                        )),
                )
                .child(
                    div()
                        .debug_selector(|| "virtual-disk-parent".to_string())
                        .child(ui::action_button(
                            "virtual-disk-parent-action",
                            "상위",
                            ui::Size::Sm,
                            ButtonStyle::secondary(cx),
                            cx.listener(|this, _event, _window, cx| {
                                this.go_to_virtual_disk_parent(cx);
                            }),
                        )),
                )
                .child(
                    div()
                        .debug_selector(|| "virtual-disk-refresh".to_string())
                        .child(ui::action_button(
                            "virtual-disk-refresh-action",
                            "새로고침",
                            ui::Size::Sm,
                            ButtonStyle::neutral(cx),
                            cx.listener(|this, _event, _window, cx| {
                                this.refresh_virtual_disk_directory(cx);
                                cx.notify();
                            }),
                        )),
                ),
        )
        .child(
            div()
                .min_w_0()
                .text_color(theme.muted_foreground)
                .child("Enter 폴더 열기 · Backspace 상위 · Ctrl+A 전체 선택 · F5 새로고침"),
        )
        .child(render_directory_layout_controls(this, cx))
        .child(explorer)
        .into_any_element()
}

fn render_directory_layout_controls(this: &mut AppRoot, cx: &mut Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();
    let layout = &this.virtual_disk.layout;
    let tree_width = layout.tree_width.round();
    let attributes_width = layout.attributes_width.round();
    let size_width = layout.size_width.round();

    h_flex()
        .debug_selector(|| "virtual-disk-layout-controls".to_string())
        .w_full()
        .min_w_0()
        .gap_1()
        .items_center()
        .text_color(theme.muted_foreground)
        .child(div().child(format!("폭 · 트리 {tree_width}px")))
        .child(div().debug_selector(|| "virtual-disk-layout-attributes-decrease".to_string()).child(ui::action_button(
            "virtual-disk-layout-attributes-decrease-action",
            format!("속성 - ({attributes_width:.0})"),
            ui::Size::Sm,
            ButtonStyle::neutral(cx),
            cx.listener(|this, _event, _window, cx| {
                this.adjust_virtual_disk_attributes_width(-8.0, cx);
            }),
        )))
        .child(div().debug_selector(|| "virtual-disk-layout-attributes-increase".to_string()).child(ui::action_button(
            "virtual-disk-layout-attributes-increase-action",
            "속성 +",
            ui::Size::Sm,
            ButtonStyle::neutral(cx),
            cx.listener(|this, _event, _window, cx| {
                this.adjust_virtual_disk_attributes_width(8.0, cx);
            }),
        )))
        .child(div().debug_selector(|| "virtual-disk-layout-size-decrease".to_string()).child(ui::action_button(
            "virtual-disk-layout-size-decrease-action",
            format!("크기 - ({size_width:.0})"),
            ui::Size::Sm,
            ButtonStyle::neutral(cx),
            cx.listener(|this, _event, _window, cx| {
                this.adjust_virtual_disk_size_width(-8.0, cx);
            }),
        )))
        .child(div().debug_selector(|| "virtual-disk-layout-size-increase".to_string()).child(ui::action_button(
            "virtual-disk-layout-size-increase-action",
            "크기 +",
            ui::Size::Sm,
            ButtonStyle::neutral(cx),
            cx.listener(|this, _event, _window, cx| {
                this.adjust_virtual_disk_size_width(8.0, cx);
            }),
        )))
        .child(div().debug_selector(|| "virtual-disk-layout-reset".to_string()).child(ui::action_button(
            "virtual-disk-layout-reset-action",
            "기본값",
            ui::Size::Sm,
            ButtonStyle::secondary(cx),
            cx.listener(|this, _event, _window, cx| {
                this.reset_virtual_disk_layout(cx);
            }),
        )))
        .into_any_element()
}

fn render_directory_tree(this: &mut AppRoot, cx: &mut Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();
    let tree_content = if this.virtual_disk.source.is_none() {
        v_flex()
            .w_full()
            .p_2()
            .text_color(theme.muted_foreground)
            .child("NTFS 파티션을 선택하면 폴더 트리를 읽습니다.")
            .into_any_element()
    } else if let Some(root) = this.virtual_disk.directory_tree.first().cloned() {
        render_directory_tree_node(this, &root, 0, cx)
    } else {
        v_flex()
            .w_full()
            .p_2()
            .text_color(theme.muted_foreground)
            .child("폴더 트리가 비어 있습니다.")
            .into_any_element()
    };
    let handle = this.virtual_disk.tree_scroll_handle.clone();
    scroll_pane("virtual-disk-tree-scroll", &handle, tree_content)
}

fn render_directory_tree_node(
    this: &AppRoot,
    node: &GuestDirectoryTreeNode,
    depth: usize,
    cx: &Context<AppRoot>,
) -> AnyElement {
    let theme = cx.theme();
    let path = node.entry.path.clone();
    let is_current = this.virtual_disk.current_path == path;
    let label = if path.is_root() {
        "디스크 루트".to_string()
    } else {
        path.file_name().unwrap_or("/").to_string()
    };
    let indicator = if !node.loaded {
        "·"
    } else if node.children.is_empty() {
        " "
    } else if node.expanded {
        "▾"
    } else {
        "▸"
    };
    let selector = if path.is_root() {
        "virtual-disk-tree-root".to_string()
    } else {
        format!("virtual-disk-tree-node-{}", path.as_str().replace('/', "-"))
    };
    let debug_selector = selector.clone();
    let click_path = path.clone();
    let mut result = v_flex()
        .w_full()
        .min_w_0()
        .child(
            h_flex()
                .debug_selector(move || debug_selector.clone())
                .w_full()
                .min_w_0()
                .gap_1()
                .items_center()
                .cursor_pointer()
                .rounded_md()
                .bg(if is_current {
                    theme.list_active
                } else {
                    theme.secondary
                })
                .hover(|style| style.bg(theme.secondary_hover))
                .px_2()
                .py_1()
                .id(gpui::SharedString::from(selector))
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    this.select_virtual_disk_tree_directory(click_path.clone(), cx);
                }))
                .child(div().w(px(12.0 + depth as f32 * 14.0)).child(indicator))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_color(theme.foreground)
                        .child(label),
                ),
        );

    if node.expanded {
        for child in node.children.iter().cloned() {
            let child_node = this
                .virtual_disk
                .directory_tree
                .iter()
                .find(|candidate| candidate.entry.path == child.path)
                .cloned()
                .unwrap_or(GuestDirectoryTreeNode {
                    entry: child,
                    children: Vec::new(),
                    expanded: false,
                    loaded: false,
                });
            result = result.child(render_directory_tree_node(this, &child_node, depth + 1, cx));
        }
    }
    result.into_any_element()
}

fn render_directory_entries(
    this: &AppRoot,
    focus_handle: &gpui::FocusHandle,
    cx: &mut Context<AppRoot>,
) -> AnyElement {
    let theme = cx.theme();
    let kind_width = normalize_virtual_disk_kind_width(this.virtual_disk.layout.kind_width);
    let attributes_width =
        normalize_virtual_disk_attributes_width(this.virtual_disk.layout.attributes_width);
    let size_width = normalize_virtual_disk_size_width(this.virtual_disk.layout.size_width);
    let mut entries = v_flex().w_full().min_w_0().gap_1();

    if this.virtual_disk.source.is_none() {
        entries = entries.child(
            div()
                .p_2()
                .text_color(theme.muted_foreground)
                .child("NTFS 파티션을 선택하면 게스트 파일 목록을 읽습니다."),
        );
    } else if this.virtual_disk.entries.is_empty() {
        entries = entries.child(
            div()
                .p_2()
                .text_color(theme.muted_foreground)
                .child("현재 폴더에 표시할 항목이 없습니다."),
        );
    } else {
        entries = entries.child(
            h_flex()
                .w_full()
                .min_w_0()
                .gap_2()
                .px_2()
                .py_1()
                .text_color(theme.muted_foreground)
                .child(div().flex_1().min_w(px(140.0)).child("이름"))
                .child(
                    div()
                        .debug_selector(|| "virtual-disk-column-kind".to_string())
                        .w(px(kind_width))
                        .min_w(px(kind_width))
                        .flex_shrink_0()
                        .child("종류"),
                )
                .child(
                    div()
                        .debug_selector(|| "virtual-disk-column-attributes".to_string())
                        .w(px(attributes_width))
                        .min_w(px(attributes_width))
                        .flex_shrink_0()
                        .child("속성"),
                )
                .child(
                    div()
                        .debug_selector(|| "virtual-disk-column-size".to_string())
                        .w(px(size_width))
                        .min_w(px(size_width))
                        .flex_shrink_0()
                        .child("크기"),
                ),
        );
        for (index, entry) in this.virtual_disk.entries.iter().enumerate() {
            let is_directory = matches!(entry.kind, GuestFileKind::Directory);
            let selected = this.virtual_disk.selected_paths.contains(&entry.path);
            let kind_label = if is_directory { "폴더" } else { "파일" };
            let attribute_label = format_attribute_label(entry.attributes);
            let size_label = if is_directory {
                String::new()
            } else {
                format_bytes(entry.size_bytes)
            };
            let name = entry.path.file_name().unwrap_or("/").to_string();
            let row_focus_handle = focus_handle.clone();
            entries = entries.child(
                h_flex()
                    .debug_selector(move || format!("virtual-disk-entry-{index}"))
                    .w_full()
                    .min_w_0()
                    .gap_2()
                    .items_center()
                    .cursor_pointer()
                    .bg(if selected {
                        theme.list_active
                    } else {
                        theme.secondary
                    })
                    .hover(|style| style.bg(theme.secondary_hover))
                    .border_b_1()
                    .border_color(theme.border)
                    .px_2()
                    .py_1()
                    .id(("virtual-disk-entry", index))
                    .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                        window.focus(&row_focus_handle);
                        if is_directory && event.click_count() >= 2 {
                            this.enter_virtual_disk_directory(index, cx);
                        } else {
                            this.select_virtual_disk_entry(
                                index,
                                event.modifiers().control,
                                event.modifiers().shift,
                                cx,
                            );
                        }
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(140.0))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_color(theme.foreground)
                            .child(name),
                    )
                    .child(
                        div()
                            .w(px(kind_width))
                            .min_w(px(kind_width))
                            .flex_shrink_0()
                            .overflow_hidden()
                            .child(ui::badge(
                                kind_label,
                                if is_directory {
                                    ui::Tone::Info
                                } else {
                                    ui::Tone::Muted
                                },
                                ui::Size::Sm,
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .w(px(attributes_width))
                            .min_w(px(attributes_width))
                            .flex_shrink_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_color(theme.muted_foreground)
                            .child(attribute_label),
                    )
                    .child(
                        div()
                            .w(px(size_width))
                            .min_w(px(size_width))
                            .flex_shrink_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_color(theme.muted_foreground)
                            .child(size_label),
                    ),
            );
        }
    }

    let list_focus_handle = focus_handle.clone();
    let entries = entries
        .id("virtual-disk-entry-list")
        .on_click(cx.listener(move |_this, _event, window, _cx| {
            window.focus(&list_focus_handle);
        }));
    let handle = this.virtual_disk.entries_scroll_handle.clone();
    scroll_pane(
        "virtual-disk-entry-scroll",
        &handle,
        entries.p_2().into_any_element(),
    )
}

fn render_copy_card(this: &mut AppRoot, cx: &mut Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();
    let input = this
        .virtual_disk
        .copy
        .target_path_input
        .as_ref()
        .map(|input| {
            div()
                .debug_selector(|| "virtual-disk-target-input".to_string())
                .flex_1()
                .min_w_0()
                .child(Input::new(input))
        });

    let mut card =
        v_flex()
            .debug_selector(|| "virtual-disk-copy-card".to_string())
            .w_full()
            .min_w_0()
            .gap_3()
            .rounded_lg()
            .p_3()
            .bg(theme.secondary)
            .border_1()
            .border_color(theme.border)
            .child(div().text_color(theme.foreground).child("호스트 대상 폴더"))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .gap_2()
                    .items_center()
                    .children(input)
                    .child(ui::action_button(
                        "virtual-disk-target-pick",
                        "폴더 선택",
                        ui::Size::Md,
                        ButtonStyle::neutral(cx),
                        cx.listener(|this, _event, window, cx| {
                            this.pick_virtual_disk_target(window, cx);
                        }),
                    )),
            )
            .child(div().text_color(theme.muted_foreground).child(
                "선택한 게스트 항목을 대상 폴더 아래에 복사합니다. 기존 파일은 건너뜁니다.",
            ));

    if let Some(progress) = &this.virtual_disk.copy.progress {
        let status = format!(
            "{} / {}개 항목 · 파일 {}개 · {} · 건너뜀 {} · 실패 {}",
            progress.completed_entries,
            progress.total_entries,
            progress.copied_files,
            format_bytes(progress.copied_bytes),
            progress.skipped_entries,
            progress.failed_entries,
        );
        card = card.child(
            v_flex()
                .debug_selector(|| "virtual-disk-copy-progress".to_string())
                .gap_1()
                .rounded_md()
                .p_2()
                .bg(theme.background)
                .child(
                    div()
                        .text_color(theme.foreground)
                        .child(if progress.stopping {
                            "중지 요청을 처리하는 중입니다…"
                        } else {
                            "복사 중…"
                        }),
                )
                .child(div().text_color(theme.muted_foreground).child(status))
                .child(div().text_color(theme.muted_foreground).child(
                    if progress.current_path.is_empty() {
                        "준비 중…".to_string()
                    } else {
                        progress.current_path.clone()
                    },
                )),
        );
        card = card.child(
            div()
                .debug_selector(|| "virtual-disk-copy-stop".to_string())
                .child(ui::action_button(
                    "virtual-disk-copy-stop-action",
                    "복사 중지",
                    ui::Size::Md,
                    ButtonStyle::danger_outline(cx),
                    cx.listener(|this, _event, window, cx| {
                        this.stop_virtual_disk_copy(window, cx);
                    }),
                )),
        );
    } else {
        card = card.child(
            div()
                .debug_selector(|| "virtual-disk-copy-start".to_string())
                .child(ui::action_button(
                    "virtual-disk-copy-start-action",
                    "선택 항목 복사",
                    ui::Size::Md,
                    ButtonStyle::primary(cx),
                    cx.listener(|this, _event, window, cx| {
                        this.start_virtual_disk_copy(window, cx);
                    }),
                )),
        );
    }

    if let Some(summary) = &this.virtual_disk.copy.summary {
        let summary_tone = if summary.error.is_some() || summary.failed_entries > 0 {
            ui::Tone::Warning
        } else if summary.cancelled {
            ui::Tone::Info
        } else {
            ui::Tone::Success
        };
        let mut summary_view = v_flex()
            .debug_selector(|| "virtual-disk-copy-summary".to_string())
            .gap_1()
            .rounded_md()
            .p_2()
            .bg(theme.background)
            .child(ui::badge(summary.line(), summary_tone, ui::Size::Sm, cx));
        for (index, issue) in summary.issues.iter().take(5).enumerate() {
            let key = issue.key();
            let suppressed = this.virtual_disk.suppressed_issue_keys.contains(&key);
            let selector = format!("virtual-disk-issue-suppress-{index}");
            let key_for_click = key.clone();
            summary_view = summary_view.child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_color(theme.muted_foreground)
                            .child(format!("{} · {}", issue.path, issue.detail)),
                    )
                    .child(
                        ui::action_button(
                                ("virtual-disk-issue-action", index),
                                if suppressed { "알림 해제" } else { "다음부터 숨기기" },
                                ui::Size::Sm,
                                ButtonStyle::secondary(cx),
                                cx.listener(move |this, _event, window, cx| {
                                    this.toggle_virtual_disk_issue_suppression(
                                        &key_for_click,
                                        window,
                                        cx,
                                    );
                                }),
                            )
                            .debug_selector(move || selector.clone()),
                    ),
            );
        }
        card = card.child(summary_view);
    }

    card.into_any_element()
}

fn format_partition_label(partition: &crate::virtual_disk::VdiPartition) -> String {
    let table = match partition.table {
        PartitionTableKind::Mbr => "MBR",
        PartitionTableKind::Gpt => "GPT",
    };
    let filesystem = match partition.filesystem {
        Some(GuestFileSystem::Ntfs { major, minor }) => format!("NTFS {major}.{minor}"),
        Some(GuestFileSystem::BitLocker) => "BitLocker 암호화".to_string(),
        Some(GuestFileSystem::MicrosoftReserved) => "Microsoft Reserved(MSR)".to_string(),
        None => "미지원/미확인 파일시스템".to_string(),
    };
    format!("파티션 {} · {table} · {filesystem}", partition.number)
}

fn partition_support_message(partition: &crate::virtual_disk::VdiPartition) -> Option<String> {
    match partition.filesystem {
        Some(GuestFileSystem::Ntfs { major: 3, minor: 1 }) => None,
        Some(GuestFileSystem::Ntfs { major, minor }) => Some(format!(
            "지원하지 않는 파일시스템 버전입니다: NTFS {major}.{minor}. 현재 NTFS 3.1만 탐색할 수 있습니다."
        )),
        Some(GuestFileSystem::BitLocker) => Some(
            "BitLocker로 암호화된 파티션입니다. 복구 키를 저장하거나 우회하지 않으며, 복호화된 사본 또는 잠금 해제된 NTFS VDI를 선택해야 합니다."
                .to_string(),
        ),
        Some(GuestFileSystem::MicrosoftReserved) => Some(
            "Microsoft Reserved(MSR) 영역은 Windows GPT 예약 영역이라 탐색할 게스트 파일이 없습니다."
                .to_string(),
        ),
        None => Some(
            "지원하지 않거나 확인할 수 없는 파일시스템입니다. 현재 NTFS 3.1만 탐색할 수 있습니다."
                .to_string(),
        ),
    }
}

fn format_attribute_label(attributes: GuestFileAttributes) -> String {
    let mut labels = Vec::new();
    if attributes.contains(GuestFileAttributes::HIDDEN) {
        labels.push("숨김");
    }
    if attributes.contains(GuestFileAttributes::SYSTEM) {
        labels.push("시스템");
    }
    if attributes.contains(GuestFileAttributes::READ_ONLY) {
        labels.push("읽기 전용");
    }
    if labels.is_empty() {
        "기본".to_string()
    } else {
        labels.join(" · ")
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    if bytes < 1024 * 1024 {
        return format!("{:.1} KB", bytes as f64 / 1024.0);
    }
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}
