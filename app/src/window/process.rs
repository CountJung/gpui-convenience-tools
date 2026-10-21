//! Full-width process monitor; long identity text is clipped within a measured name column.
use super::ui::{self, ButtonStyle, Size};
use crate::app::AppRoot;
use crate::config::ProcessSort;
use crate::process_monitor::visible_rows;
use gpui::{
    div, px, AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{h_flex, input::Input, theme::ActiveTheme, v_flex};

pub fn render(this: &mut AppRoot, window: &mut Window, cx: &mut Context<AppRoot>) -> AnyElement {
    this.ensure_process_inputs(window, cx);
    let state = &this.process_monitor;
    let (Some(search), Some(interval)) = (&state.search_input, &state.interval_input) else {
        return div().into_any_element();
    };
    let theme = cx.theme();
    let foreground = theme.foreground;
    let muted = theme.muted_foreground;
    let danger = theme.danger;
    let border = theme.border;
    let selected_bg = theme.secondary;
    let rows = visible_rows(
        &state.rows,
        &state.query,
        state.config.sort,
        state.config.top_count,
    );
    let mut content = v_flex().w_full().min_w_0().pr_4().gap_3().text_color(foreground)
        .child(div().text_lg().child("프로세스 모니터"))
        .child(div().text_sm().text_color(muted)
            .child("CPU/메모리 내림차순 · 검색 후 상위 목록 표시 · 자동 갱신은 이 패널이 열려 있을 때만 실행"))
        .child(div().debug_selector(|| "process-search".into()).w_full().min_w_0().child(Input::new(search)))
        .child(h_flex().w_full().min_w_0().gap_2().flex_wrap()
            .child(div().debug_selector(|| "process-sort-cpu".into()).child(ui::action_button(
                "process-sort-cpu-button", if state.config.sort == ProcessSort::Cpu { "CPU 순 ✓" } else { "CPU 순" },
                Size::Md, ButtonStyle::secondary(cx), cx.listener(|this, _, _, cx| this.set_process_sort(ProcessSort::Cpu, cx)))))
            .child(div().debug_selector(|| "process-sort-memory".into()).child(ui::action_button(
                "process-sort-memory-button", if state.config.sort == ProcessSort::Memory { "메모리 순 ✓" } else { "메모리 순" },
                Size::Md, ButtonStyle::secondary(cx), cx.listener(|this, _, _, cx| this.set_process_sort(ProcessSort::Memory, cx)))))
            .child(div().debug_selector(|| "process-count".into()).child(ui::action_button(
                "process-count-button", format!("상위 {}개", state.config.top_count), Size::Md,
                ButtonStyle::secondary(cx), cx.listener(|this, _, _, cx| this.cycle_process_count(cx)))))
            .child(div().debug_selector(|| "process-refresh".into()).child(ui::action_button(
                "process-refresh-button", if state.busy { "갱신/처리 중…" } else { "지금 새로고침" }, Size::Md,
                ButtonStyle::primary(cx), cx.listener(|this, _, _, cx| this.refresh_process_monitor(cx))))))
        .child(h_flex().debug_selector(|| "process-interval-row".into()).w_full().min_w_0().gap_2()
            .child(div().flex_shrink_0().whitespace_nowrap().child("주기 (초)"))
            .child(div().debug_selector(|| "process-interval".into()).w(px(90.0)).flex_shrink_0().child(Input::new(interval))))
        .child(div().debug_selector(|| "process-interval-help".into()).w_full().min_w_0().flex_shrink_0()
            .text_sm().text_color(muted).child("1–60초 · 정렬·주기·표시 개수만 자동 저장"))
        .child(div().debug_selector(|| "process-save-status".into()).w_full().min_w_0().flex_shrink_0().text_sm()
            .text_color(if state.save_error { danger } else { muted })
            .child(if state.save_error { "설정 저장 실패 · 쓰기 권한을 확인하고 설정 저장을 재시도하세요." }
                else { "검색어·프로세스 목록·실행 경로는 저장하거나 로그에 기록하지 않습니다." }))
        .child(div().debug_selector(|| "process-status".into()).w_full().min_w_0().text_sm()
            .text_color(if state.failed { danger } else { muted })
            .child(if state.status.is_empty() { "프로세스 정보를 기다리는 중입니다.".into() } else { state.status.clone() }));

    if let Some(selected) = &state.selected {
        let identity = &selected.identity;
        let mut confirmation = v_flex()
            .debug_selector(|| "process-confirmation".into())
            .w_full()
            .min_w_0()
            .p_3()
            .gap_2()
            .border_1()
            .border_color(border)
            .child(
                div()
                    .text_sm()
                    .child("종료 대상 재확인 · 저장하지 않은 작업이 손실될 수 있습니다."),
            )
            .child(
                h_flex()
                    .debug_selector(|| "process-selected-name".into())
                    .w_full()
                    .min_w_0()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(selected.name.clone()),
                    )
                    .child(
                        div()
                            .debug_selector(|| "process-selected-pid".into())
                            .min_w(px(90.0))
                            .flex_shrink_0()
                            .whitespace_nowrap()
                            .child(format!("PID {}", identity.pid)),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .child(format!("시작 시각 (Unix 초): {}", identity.start_time)),
            )
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_sm()
                    .child(identity.path.to_string_lossy().into_owned()),
            );
        if let Some(reason) = selected.blocked_reason {
            confirmation = confirmation.child(div().text_sm().text_color(danger).child(reason));
        }
        let mut actions = h_flex().w_full().min_w_0().gap_2();
        if selected.blocked_reason.is_none() {
            actions = actions.child(
                div()
                    .debug_selector(|| "process-confirm-kill".into())
                    .child(ui::action_button(
                        "process-confirm-kill-button",
                        "확인 · 이 프로세스 종료",
                        Size::Md,
                        ButtonStyle::danger_outline(cx),
                        cx.listener(|this, _, _, cx| this.confirm_process_termination(cx)),
                    )),
            );
        }
        actions = actions.child(div().debug_selector(|| "process-cancel-kill".into()).child(
            ui::action_button(
                "process-cancel-kill-button",
                "취소",
                Size::Md,
                ButtonStyle::secondary(cx),
                cx.listener(|this, _, _, cx| this.cancel_process_termination(cx)),
            ),
        ));
        content = content.child(confirmation.child(actions));
    }

    content = content
        .child(div().text_sm().text_color(muted).child(format!(
            "표시 {}개 / 전체 {}개 · 행을 클릭해 대상 확인",
            rows.len(),
            state.rows.len()
        )))
        .child(
            h_flex()
                .debug_selector(|| "process-table-header".into())
                .w_full()
                .min_w_0()
                .gap_2()
                .px_2()
                .border_1()
                .border_color(border)
                .text_sm()
                .child(
                    div()
                        .flex_1()
                        .min_w(px(160.0))
                        .child("이름 / PID / 실행 경로"),
                )
                .child(metric_cell("CPU".into(), "process-cpu-header".into()))
                .child(metric_cell("메모리".into(), "process-memory-header".into())),
        );
    for row in rows {
        let pid = row.identity.pid;
        let selected = state
            .selected
            .as_ref()
            .is_some_and(|selected| selected.identity == row.identity);
        let captured = row.clone();
        content = content.child(
            h_flex()
                .id(("process-row", pid as usize))
                .debug_selector(move || format!("process-row-{pid}"))
                .w_full()
                .min_w_0()
                .gap_2()
                .p_2()
                .border_1()
                .border_color(border)
                .bg(if selected {
                    selected_bg
                } else {
                    theme.background
                })
                .cursor_pointer()
                .on_click(
                    cx.listener(move |this, _, _, cx| this.select_process(captured.clone(), cx)),
                )
                .child(
                    v_flex()
                        .debug_selector(move || format!("process-name-{pid}"))
                        .flex_1()
                        .min_w(px(160.0))
                        .overflow_hidden()
                        .gap_1()
                        .child(
                            h_flex()
                                .w_full()
                                .min_w_0()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .whitespace_nowrap()
                                        .overflow_hidden()
                                        .child(row.name.clone()),
                                )
                                .child(
                                    div()
                                        .debug_selector(move || format!("process-pid-{pid}"))
                                        .min_w(px(90.0))
                                        .flex_shrink_0()
                                        .whitespace_nowrap()
                                        .child(format!("PID {pid}")),
                                ),
                        )
                        .child(
                            div()
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_sm()
                                .text_color(muted)
                                .child(if row.identity.path.as_os_str().is_empty() {
                                    "경로 조회 불가".into()
                                } else {
                                    row.identity.path.to_string_lossy().into_owned()
                                }),
                        )
                        .child(
                            div()
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_sm()
                                .text_color(muted)
                                .child(
                                    row.blocked_reason.unwrap_or(
                                        "현재 사용자 프로세스 · 클릭 후 종료 대상 재확인",
                                    ),
                                ),
                        ),
                )
                .child(metric_cell(
                    format!("{:.1}%", row.cpu_percent),
                    format!("process-cpu-{pid}"),
                ))
                .child(metric_cell(
                    format!("{:.1} MiB", row.memory_bytes as f64 / 1_048_576.0),
                    format!("process-memory-{pid}"),
                )),
        );
    }
    content = content.child(div().debug_selector(|| "process-list-end".into()).text_sm().text_color(muted)
        .child("시스템/critical/다른 사용자/자기 프로세스는 종료 차단 · 관리자 권한 자동 요청 없음"));
    super::scroll_pane("process-page", &state.scroll, content.into_any_element())
}

/// Shared widths for the process table's two numeric columns (header and rows).
fn metric_cell(text: String, selector: String) -> impl IntoElement {
    div()
        .debug_selector(move || selector.clone())
        .w(px(98.0))
        .min_w(px(98.0))
        .flex_shrink_0()
        .whitespace_nowrap()
        .overflow_hidden()
        .text_sm()
        .child(text)
}
