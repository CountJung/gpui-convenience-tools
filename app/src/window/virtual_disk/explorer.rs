//! 전체 높이를 채우는 폴더 트리·파일 목록과 열 너비 조절.
use super::super::{
    resizable_split, scroll_pane,
    ui::{self, ButtonStyle},
};
use crate::app::{
    AppRoot, CopySelected, EnterSelected, GuestDirectoryTreeNode, ParentDirectory, Refresh,
    SelectAll, VIRTUAL_DISK_KEY_CONTEXT,
};
use crate::config::{
    normalize_virtual_disk_attributes_width, normalize_virtual_disk_kind_width,
    normalize_virtual_disk_size_width, normalize_virtual_disk_tree_width,
};
use crate::virtual_disk::{GuestFileAttributes, GuestFileKind};
use gpui::{
    div, px, AnyElement, ClickEvent, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled,
};
use gpui_component::{h_flex, theme::ActiveTheme, v_flex, PixelsExt};

pub(super) fn render_directory_card(this: &mut AppRoot, cx: &mut Context<AppRoot>) -> AnyElement {
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
        .flex_1()
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
                let Some(width) = state.read(app).sizes().first().map(|size| size.as_f32()) else {
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
        .flex_1()
        .gap_2()
        .overflow_hidden()
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
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_color(theme.foreground)
                        .child(format!(
                            "현재 경로: {current_path} · {selected_count}개 선택"
                        )),
                )
                .child(
                    div()
                        .debug_selector(|| "virtual-disk-layout-toggle".to_string())
                        .child(ui::action_button(
                            "virtual-disk-layout-toggle-action",
                            "열 너비",
                            ui::Size::Sm,
                            ButtonStyle::neutral(cx),
                            cx.listener(|this, _event, _window, cx| {
                                this.virtual_disk.layout_controls_visible =
                                    !this.virtual_disk.layout_controls_visible;
                                cx.notify();
                            }),
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
        .children(
            this.virtual_disk
                .layout_controls_visible
                .then(|| render_directory_layout_controls(this, cx)),
        )
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
        .child(
            div()
                .debug_selector(|| "virtual-disk-layout-attributes-decrease".to_string())
                .child(ui::action_button(
                    "virtual-disk-layout-attributes-decrease-action",
                    format!("속성 - ({attributes_width:.0})"),
                    ui::Size::Sm,
                    ButtonStyle::neutral(cx),
                    cx.listener(|this, _event, _window, cx| {
                        this.adjust_virtual_disk_attributes_width(-8.0, cx);
                    }),
                )),
        )
        .child(
            div()
                .debug_selector(|| "virtual-disk-layout-attributes-increase".to_string())
                .child(ui::action_button(
                    "virtual-disk-layout-attributes-increase-action",
                    "속성 +",
                    ui::Size::Sm,
                    ButtonStyle::neutral(cx),
                    cx.listener(|this, _event, _window, cx| {
                        this.adjust_virtual_disk_attributes_width(8.0, cx);
                    }),
                )),
        )
        .child(
            div()
                .debug_selector(|| "virtual-disk-layout-size-decrease".to_string())
                .child(ui::action_button(
                    "virtual-disk-layout-size-decrease-action",
                    format!("크기 - ({size_width:.0})"),
                    ui::Size::Sm,
                    ButtonStyle::neutral(cx),
                    cx.listener(|this, _event, _window, cx| {
                        this.adjust_virtual_disk_size_width(-8.0, cx);
                    }),
                )),
        )
        .child(
            div()
                .debug_selector(|| "virtual-disk-layout-size-increase".to_string())
                .child(ui::action_button(
                    "virtual-disk-layout-size-increase-action",
                    "크기 +",
                    ui::Size::Sm,
                    ButtonStyle::neutral(cx),
                    cx.listener(|this, _event, _window, cx| {
                        this.adjust_virtual_disk_size_width(8.0, cx);
                    }),
                )),
        )
        .child(
            div()
                .debug_selector(|| "virtual-disk-layout-reset".to_string())
                .child(ui::action_button(
                    "virtual-disk-layout-reset-action",
                    "기본값",
                    ui::Size::Sm,
                    ButtonStyle::secondary(cx),
                    cx.listener(|this, _event, _window, cx| {
                        this.reset_virtual_disk_layout(cx);
                    }),
                )),
        )
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
    let mut result = v_flex().w_full().min_w_0().child(
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
    let entries = entries.id("virtual-disk-entry-list").on_click(cx.listener(
        move |_this, _event, window, _cx| {
            window.focus(&list_focus_handle);
        },
    ));
    let handle = this.virtual_disk.entries_scroll_handle.clone();
    scroll_pane(
        "virtual-disk-entry-scroll",
        &handle,
        entries.p_2().into_any_element(),
    )
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

pub(super) fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    if bytes < 1024 * 1024 {
        return format!("{:.1} KB", bytes as f64 / 1024.0);
    }
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}
