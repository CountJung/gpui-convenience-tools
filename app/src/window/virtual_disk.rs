//! VirtualBox 디스크 선택 단계와 탐색 작업 화면.
mod copy;
mod explorer;

use super::{
    scroll_pane,
    ui::{self, ButtonStyle},
};
use crate::app::AppRoot;
use crate::virtual_disk::{GuestFileSystem, PartitionTableKind};
use gpui::{
    div, px, AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{h_flex, input::Input, theme::ActiveTheme, v_flex};

pub fn render(this: &mut AppRoot, window: &mut Window, cx: &mut Context<AppRoot>) -> AnyElement {
    this.ensure_virtual_disk_input(window, cx);
    this.ensure_virtual_disk_target_input(window, cx);
    this.ensure_virtual_disk_focus(cx);
    this.schedule_validation_virtual_disk_copy(window, cx);

    if this.virtual_disk.source.is_none() || this.virtual_disk.setup_visible {
        let page_scroll = this.virtual_disk_page_scroll.clone();
        let mut setup = v_flex()
            .w_full()
            .gap_3()
            .p_1()
            .pr_3()
            .child(render_header(this, cx))
            .child(render_safety_notice(cx))
            .child(render_source_card(this, cx))
            .child(render_partition_card(this, cx));
        if this.virtual_disk.source.is_some() {
            setup = setup.child(
                div()
                    .debug_selector(|| "virtual-disk-resume".to_string())
                    .child(ui::action_button(
                        "virtual-disk-resume-action",
                        "파일 탐색으로 돌아가기",
                        ui::Size::Md,
                        ButtonStyle::primary(cx),
                        cx.listener(|this, _event, _window, cx| {
                            this.virtual_disk.setup_visible = false;
                            cx.notify();
                        }),
                    )),
            );
        }
        return scroll_pane("virtual-disk-page", &page_scroll, setup.into_any_element());
    }

    let mut workspace = v_flex()
        .debug_selector(|| "virtual-disk-page".to_string())
        .size_full()
        .min_w_0()
        .min_h_0()
        .gap_2()
        .overflow_hidden()
        .child(render_connection_bar(this, cx));
    if let Some(error) = &this.virtual_disk.error {
        workspace = workspace.child(
            div()
                .id("virtual-disk-error-scroll")
                .debug_selector(|| "virtual-disk-error".to_string())
                .w_full()
                .min_w_0()
                .max_h(px(48.0))
                .overflow_y_scroll()
                .track_scroll(&this.virtual_disk.error_scroll)
                .flex_shrink_0()
                .text_sm()
                .text_color(cx.theme().danger)
                .child(error.clone()),
        );
    }
    workspace
        .child(explorer::render_directory_card(this, cx))
        .child(copy::render(this, cx))
        .into_any_element()
}

fn render_connection_bar(this: &AppRoot, cx: &Context<AppRoot>) -> AnyElement {
    let disk = this
        .virtual_disk
        .vdi_path
        .as_ref()
        .and_then(|path| path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "VDI".to_string());
    let partition = this
        .virtual_disk
        .selected_partition
        .and_then(|index| this.virtual_disk.partitions.get(index))
        .map(|partition| format!(" · 파티션 {}", partition.number))
        .unwrap_or_default();
    h_flex()
        .debug_selector(|| "virtual-disk-connection-bar".to_string())
        .w_full()
        .min_w_0()
        .flex_shrink_0()
        .gap_2()
        .items_center()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_color(cx.theme().foreground)
                .child(format!("{disk}{partition}")),
        )
        .child(ui::badge("읽기 전용", ui::Tone::Warning, ui::Size::Sm, cx))
        .child(
            div()
                .debug_selector(|| "virtual-disk-change-source".to_string())
                .child(ui::action_button(
                    "virtual-disk-change-source-action",
                    if this.virtual_disk.copy.progress.is_some() {
                        "복사 중"
                    } else {
                        "디스크 변경"
                    },
                    ui::Size::Sm,
                    ButtonStyle::neutral(cx),
                    cx.listener(|this, _event, _window, cx| {
                        if this.virtual_disk.copy.progress.is_some() {
                            return;
                        }
                        this.virtual_disk.setup_visible = true;
                        cx.notify();
                    }),
                )),
        )
        .into_any_element()
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
