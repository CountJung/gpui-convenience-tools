//! 탐색 영역을 밀어내지 않는 고정 복사줄과 펼칠 수 있는 결과 상세.
use super::super::{
    scroll_pane,
    ui::{self, ButtonStyle},
};
use super::explorer::format_bytes;
use crate::app::AppRoot;
use gpui::{div, px, AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Styled};
use gpui_component::{h_flex, input::Input, theme::ActiveTheme, v_flex};

pub(super) fn render(this: &mut AppRoot, cx: &mut Context<AppRoot>) -> AnyElement {
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
    let copying = this.virtual_disk.copy.progress.is_some();
    let mut toolbar = h_flex()
        .w_full()
        .min_w_0()
        .gap_2()
        .items_center()
        .child(
            div()
                .flex_shrink_0()
                .text_color(theme.muted_foreground)
                .child("복사 대상"),
        )
        .children(input)
        .child(ui::action_button(
            "virtual-disk-target-pick",
            "폴더 선택",
            ui::Size::Sm,
            ButtonStyle::neutral(cx),
            cx.listener(|this, _event, window, cx| {
                this.pick_virtual_disk_target(window, cx);
            }),
        ));
    toolbar = toolbar.child(if copying {
        div()
            .debug_selector(|| "virtual-disk-copy-stop".to_string())
            .child(ui::action_button(
                "virtual-disk-copy-stop-action",
                "복사 중지",
                ui::Size::Sm,
                ButtonStyle::danger_outline(cx),
                cx.listener(|this, _event, window, cx| {
                    this.stop_virtual_disk_copy(window, cx);
                }),
            ))
    } else {
        div()
            .debug_selector(|| "virtual-disk-copy-start".to_string())
            .child(ui::action_button(
                "virtual-disk-copy-start-action",
                "선택 항목 복사",
                ui::Size::Sm,
                ButtonStyle::primary(cx),
                cx.listener(|this, _event, window, cx| {
                    this.start_virtual_disk_copy(window, cx);
                }),
            ))
    });
    let mut footer = v_flex()
        .debug_selector(|| "virtual-disk-copy-card".to_string())
        .relative()
        .w_full()
        .min_w_0()
        .flex_shrink_0()
        .gap_1()
        .pt_2()
        .border_t_1()
        .border_color(theme.border)
        .child(toolbar);
    if let Some(progress) = &this.virtual_disk.copy.progress {
        footer = footer.child(
            div()
                .debug_selector(|| "virtual-disk-copy-progress".to_string())
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(format!(
                    "{} · {} / {}개 · {} · 실패 {} · {}",
                    if progress.stopping {
                        "중지 처리 중"
                    } else {
                        "복사 중"
                    },
                    progress.completed_entries,
                    progress.total_entries,
                    format_bytes(progress.copied_bytes),
                    progress.failed_entries,
                    progress.current_path
                )),
        );
    } else if let Some(summary) = &this.virtual_disk.copy.summary {
        let tone = if summary.error.is_some() || summary.failed_entries > 0 {
            ui::Tone::Warning
        } else if summary.cancelled {
            ui::Tone::Info
        } else {
            ui::Tone::Success
        };
        footer = footer.child(
            h_flex()
                .debug_selector(|| "virtual-disk-copy-summary".to_string())
                .w_full()
                .min_w_0()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_sm()
                        .child(ui::badge(summary.line(), tone, ui::Size::Sm, cx)),
                )
                .child(
                    div()
                        .debug_selector(|| "virtual-disk-copy-details-toggle".to_string())
                        .child(ui::action_button(
                            "virtual-disk-copy-details-toggle-action",
                            if this.virtual_disk.copy_details_visible {
                                "결과 접기"
                            } else {
                                "결과 상세"
                            },
                            ui::Size::Sm,
                            ButtonStyle::neutral(cx),
                            cx.listener(|this, _event, _window, cx| {
                                this.virtual_disk.copy_details_visible =
                                    !this.virtual_disk.copy_details_visible;
                                cx.notify();
                            }),
                        )),
                ),
        );
        if this.virtual_disk.copy_details_visible {
            footer = footer.child(render_details(this, cx));
        }
    } else {
        footer = footer.child(div().text_sm().text_color(theme.muted_foreground).child(
            "Enter 폴더 열기 · Backspace 상위 · Ctrl+A 전체 선택 · Ctrl+C 복사 · 기존 파일 건너뜀",
        ));
    }
    footer.into_any_element()
}

fn render_details(this: &AppRoot, cx: &Context<AppRoot>) -> AnyElement {
    let summary = this
        .virtual_disk
        .copy
        .summary
        .as_ref()
        .expect("copy summary is available");
    let mut details = v_flex().w_full().min_w_0().gap_2().p_2();
    if let Some(error) = &summary.error {
        details = details.child(div().text_color(cx.theme().danger).child(error.clone()));
    }
    if summary.issues.is_empty() && summary.error.is_none() {
        details = details.child(
            div()
                .text_color(cx.theme().muted_foreground)
                .child("항목별 오류 없음"),
        );
    }
    for (index, issue) in summary.issues.iter().enumerate() {
        let key = issue.key();
        let suppressed = this.virtual_disk.suppressed_issue_keys.contains(&key);
        details = details.child(
            h_flex()
                .w_full()
                .min_w_0()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{} · {}", issue.path, issue.detail)),
                )
                .child(
                    ui::action_button(
                        ("virtual-disk-issue-action", index),
                        if suppressed {
                            "알림 해제"
                        } else {
                            "다음부터 숨기기"
                        },
                        ui::Size::Sm,
                        ButtonStyle::secondary(cx),
                        cx.listener(move |this, _event, window, cx| {
                            this.toggle_virtual_disk_issue_suppression(&key, window, cx);
                        }),
                    )
                    .debug_selector(move || format!("virtual-disk-issue-suppress-{index}")),
                ),
        );
    }
    div()
        .debug_selector(|| "virtual-disk-copy-details".to_string())
        .absolute()
        .left_0()
        .right_0()
        .bottom(px(76.0))
        .h(px(112.0))
        .bg(cx.theme().secondary)
        .border_1()
        .border_color(cx.theme().border)
        .rounded_md()
        .child(scroll_pane(
            "virtual-disk-copy-details-scroll",
            &this.virtual_disk.copy_details_scroll,
            details.into_any_element(),
        ))
        .into_any_element()
}
