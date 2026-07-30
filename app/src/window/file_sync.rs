//! 파일 동기화 패널.
//!
//! 작업 선택, 실행, 설정, 실패 기록을 하나의 세로 흐름으로 배치한다.
//! 페이지 전체가 하나의 스크롤 영역을 사용하므로 좁은 창에서도 설정 pane이 눌리지 않는다.
//!
//! 진행 상태 표시줄은 스크롤 영역 **밖** 하단에 고정한다. 스크롤되는 컨텐츠 안에 두면
//! 실행 중에 아래로 내려가야 현재 파일이 보이므로 진행 표시의 의미가 없다.

use gpui::{
    div, AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{h_flex, input::Input, theme::ActiveTheme, v_flex};

use crate::app::{AppRoot, IntervalTarget};
use crate::util::format_interval;
use crate::window::scroll_pane;
use crate::window::ui::{self, ButtonStyle};

pub fn render(this: &mut AppRoot, window: &mut Window, cx: &mut Context<AppRoot>) -> AnyElement {
    this.ensure_sync_inputs(window, cx);

    let page_scroll = this.sync_page_scroll.clone();
    let jobs = render_job_list(this, cx);
    let settings = render_job_settings(this, window, cx);
    let failures = render_failures(this, cx);
    let status_bar = render_status_bar(this, cx);

    v_flex()
        .size_full()
        .min_h_0()
        .gap_2()
        .child(
            // 스크롤 영역만 남은 높이를 가져가고, 아래 표시줄은 자연 높이를 유지한다.
            div().flex_1().min_h_0().child(scroll_pane(
                "file-sync-page",
                &page_scroll,
                v_flex()
                    .w_full()
                    .gap_4()
                    .p_1()
                    // 세로 스크롤바가 오른쪽 위에 겹쳐 그려지므로, 그만큼 비워 두지 않으면
                    // 행 끝의 버튼(중지·작업 삭제) 테두리가 가려진다.
                    .pr_3()
                    .child(jobs)
                    .child(settings)
                    .child(failures)
                    .into_any_element(),
            )),
        )
        .child(status_bar)
        .into_any_element()
}

// ─────────────────────────────────────────────
// 하단 진행 상태 표시줄
// ─────────────────────────────────────────────

fn render_status_bar(this: &mut AppRoot, cx: &mut Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();
    let fg = theme.foreground;
    let muted_fg = theme.muted_foreground;
    let border = theme.border;

    let bar = h_flex()
        .debug_selector(|| "file-sync-status-bar".to_string())
        .w_full()
        .gap_3()
        .items_center()
        .rounded_lg()
        .bg(theme.secondary)
        .border_1()
        .border_color(border)
        .px_3()
        .py_2();

    let Some(running) = this.sync_running.clone() else {
        return bar
            .child(ui::badge("대기 중", ui::Tone::Muted, ui::Size::Sm, cx))
            .child(
                div()
                    .debug_selector(|| "file-sync-current-file".to_string())
                    .flex_1()
                    .min_w_0()
                    .text_color(muted_fg)
                    .child("실행 중인 동기화가 없습니다."),
            )
            .into_any_element();
    };

    let (tone, state_label) = if running.stopping {
        (ui::Tone::Warning, "중지 중")
    } else {
        (ui::Tone::Info, "동기화 중")
    };

    bar.child(ui::badge(state_label, tone, ui::Size::Sm, cx))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .debug_selector(|| "file-sync-current-file".to_string())
                        .text_color(fg)
                        .child(running.display_path()),
                )
                .child(
                    div()
                        .text_color(muted_fg)
                        .child(format!("{} — {}", running.label, running.counters())),
                ),
        )
        .into_any_element()
}

// ─────────────────────────────────────────────
// 작업 선택·전체 실행
// ─────────────────────────────────────────────

fn render_job_list(this: &mut AppRoot, cx: &mut Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();
    let fg = theme.foreground;
    let muted_fg = theme.muted_foreground;
    let border = theme.border;
    let card = theme.secondary;
    let selected_idx = this.selected_sync_job;

    // 이 패널의 버튼은 테두리 색을 hover 색과 같이 쓴다(승격 전 시각 유지).
    let neutral_btn = ButtonStyle::neutral(cx).hover(border);
    let primary_btn = ButtonStyle::primary(cx).border(theme.primary_hover);
    let is_running = this.sync_running.is_some();
    let stopping = this
        .sync_running
        .as_ref()
        .is_some_and(|running| running.stopping);
    let stop_btn = if is_running {
        ButtonStyle::danger(cx).border(theme.danger_active)
    } else {
        ButtonStyle::muted(cx).border(border)
    };

    // ── 작업 행 ──
    let mut rows = v_flex();
    if this.sync_jobs.is_empty() {
        rows = rows.child(
            div()
                .px_3()
                .py_4()
                .text_color(muted_fg)
                .child("등록된 동기화 작업이 없습니다. '새 작업'으로 추가하세요."),
        );
    } else {
        for (ix, job) in this.sync_jobs.iter().enumerate() {
            let is_selected = selected_idx == Some(ix);
            let label = job.label();
            let source = job.source.clone();
            let target = job.target.clone();
            let enabled = job.enabled;
            let interval = job.interval_secs;
            let status = this.sync_status.get(&job.id).cloned().unwrap_or_default();

            rows = rows.child(
                div()
                    .id(("sync-job-row", ix))
                    .px_3()
                    .py_2()
                    .cursor_pointer()
                    .border_b_1()
                    .border_color(border)
                    .bg(if is_selected { theme.list_active } else { card })
                    .hover(|s| s.bg(theme.secondary_hover))
                    .on_click(cx.listener(move |this, _ev, window, cx| {
                        this.select_sync_job(ix, window, cx);
                    }))
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(div().flex_1().min_w_0().text_color(fg).child(label))
                                    .child(
                                        div()
                                            .rounded_md()
                                            .px_2()
                                            .py_1()
                                            .bg(if enabled { theme.success } else { theme.muted })
                                            .text_color(if enabled {
                                                theme.success_foreground
                                            } else {
                                                theme.muted_foreground
                                            })
                                            .child(if enabled { "자동" } else { "수동" }),
                                    )
                                    .child(
                                        div().text_color(muted_fg).child(format_interval(interval)),
                                    ),
                            )
                            .child(
                                div()
                                    .text_color(muted_fg)
                                    .child(format!("{source}  →  {target}")),
                            )
                            .child(
                                div()
                                    .text_color(if status.failed {
                                        theme.danger
                                    } else {
                                        muted_fg
                                    })
                                    .child(status.line()),
                            ),
                    ),
            );
        }
    }

    v_flex()
        .w_full()
        .gap_3()
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .child(div().text_color(fg).child("파일 동기화"))
                .child(
                    h_flex()
                        .gap_2()
                        .child(ui::action_button(
                            "sync-new-job",
                            "새 작업",
                            ui::Size::Md,
                            neutral_btn,
                            cx.listener(|this, _ev, window, cx| {
                                this.add_sync_job(window, cx);
                            }),
                        ))
                        .child(ui::action_button(
                            "sync-run-all",
                            "전체 지금 동기화",
                            ui::Size::Md,
                            primary_btn,
                            cx.listener(|this, _ev, window, cx| {
                                this.request_sync_all(window, cx);
                            }),
                        ))
                        // 중지는 실행 중일 때만 위험 색으로 강조한다. 유휴 상태에서도
                        // 버튼을 숨기지 않아 실행 직후 위치가 밀리지 않게 한다.
                        .child(
                            div()
                                .debug_selector(|| "sync-stop".to_string())
                                .child(ui::action_button(
                                    "sync-stop",
                                    if stopping { "중지 중…" } else { "중지" },
                                    ui::Size::Md,
                                    stop_btn,
                                    cx.listener(|this, _ev, window, cx| {
                                        this.request_sync_stop(window, cx);
                                    }),
                                )),
                        ),
                ),
        )
        // ── 작업 목록 카드 ──
        .child(
            div()
                .debug_selector(|| "file-sync-job-list-card".to_string())
                .rounded_lg()
                .bg(card)
                .border_1()
                .border_color(border)
                .child(v_flex().child(rows)),
        )
        .into_any_element()
}

// ─────────────────────────────────────────────
// 선택한 작업 설정
// ─────────────────────────────────────────────

fn render_job_settings(
    this: &mut AppRoot,
    window: &mut Window,
    cx: &mut Context<AppRoot>,
) -> AnyElement {
    let theme = cx.theme();
    let fg = theme.foreground;
    let muted_fg = theme.muted_foreground;
    let border = theme.border;
    let card = theme.secondary;

    // 이 패널의 버튼은 테두리 색을 hover 색과 같이 쓴다(승격 전 시각 유지).
    let neutral_btn = ButtonStyle::neutral(cx).hover(border);
    let primary_btn = ButtonStyle::primary(cx).border(theme.primary_hover);
    let danger_btn = ButtonStyle::danger(cx).border(theme.danger_active);

    let Some(selected) = this.selected_sync_job else {
        return v_flex()
            .w_full()
            .gap_3()
            .child(div().text_color(fg).child("작업 설정"))
            .child(
                div()
                    .rounded_lg()
                    .bg(card)
                    .border_1()
                    .border_color(border)
                    .p_4()
                    .text_color(muted_fg)
                    .child("위 작업 목록에서 작업을 선택하거나 '새 작업'을 추가하세요."),
            )
            .into_any_element();
    };

    let Some(job) = this.sync_jobs.get(selected).cloned() else {
        return div().into_any_element();
    };

    let name_input = this.sync_name_input.clone();
    let source_input = this.sync_source_input.clone();
    let target_input = this.sync_target_input.clone();

    let interval_row = crate::window::interval::render(this, IntervalTarget::Sync, window, cx);

    v_flex()
        .w_full()
        .gap_3()
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .child(div().text_color(fg).child("작업 설정"))
                .child(
                    h_flex()
                        .gap_2()
                        .child(div().debug_selector(|| "sync-run-one".to_string()).child(
                            ui::action_button(
                                "sync-run-one",
                                "저장하고 지금 동기화",
                                ui::Size::Md,
                                primary_btn,
                                cx.listener(move |this, _ev, window, cx| {
                                    this.request_sync_job(selected, window, cx);
                                }),
                            ),
                        ))
                        .child(ui::action_button(
                            "sync-delete-job",
                            "작업 삭제",
                            ui::Size::Md,
                            danger_btn,
                            cx.listener(move |this, _ev, window, cx| {
                                this.remove_sync_job(selected, window, cx);
                            }),
                        )),
                ),
        )
        .child(
            div()
                .debug_selector(|| "file-sync-settings-card".to_string())
                .rounded_lg()
                .bg(card)
                .border_1()
                .border_color(border)
                .p_4()
                .child(
                    v_flex()
                        .gap_3()
                        // ── 이름 ──
                        .child(div().text_color(fg).child("작업 이름"))
                        .children(name_input.as_ref().map(Input::new))
                        // ── 원본 폴더 ──
                        .child(div().text_color(fg).child("원본 폴더"))
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .children(source_input.as_ref().map(Input::new)),
                                )
                                .child(ui::action_button(
                                    "sync-pick-source",
                                    "찾아보기",
                                    ui::Size::Md,
                                    neutral_btn,
                                    cx.listener(|this, _ev, window, cx| {
                                        this.pick_sync_folder(true, window, cx);
                                    }),
                                )),
                        )
                        // ── 대상 폴더 ──
                        .child(div().text_color(fg).child("대상 폴더"))
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .children(target_input.as_ref().map(Input::new)),
                                )
                                .child(ui::action_button(
                                    "sync-pick-target",
                                    "찾아보기",
                                    ui::Size::Md,
                                    neutral_btn,
                                    cx.listener(|this, _ev, window, cx| {
                                        this.pick_sync_folder(false, window, cx);
                                    }),
                                )),
                        )
                        .child(div().text_color(muted_fg).child(
                            "직접 입력한 값은 '설정 저장' 또는 실행 버튼을 누를 때 저장됩니다.",
                        ))
                        .child(ui::action_button(
                            "sync-apply-paths",
                            "설정 저장",
                            ui::Size::Md,
                            neutral_btn,
                            cx.listener(|this, _ev, window, cx| {
                                this.apply_sync_inputs(window, cx);
                            }),
                        ))
                        // ── 감시 주기 ──
                        .child(div().text_color(fg).child("감시 주기"))
                        .child(interval_row)
                        // ── 옵션 ──
                        .child(div().text_color(fg).child("옵션"))
                        .child(ui::option_row(
                            "sync-opt-enabled",
                            "자동 동기화 사용",
                            "감시 주기마다 자동으로 실행합니다.",
                            job.enabled,
                            cx.listener(|this, checked: &bool, window, cx| {
                                let checked = *checked;
                                this.update_selected_sync_job(window, cx, move |job| {
                                    job.enabled = checked
                                });
                            }),
                            cx,
                        ))
                        .child(ui::option_row(
                            "sync-opt-hidden",
                            "숨김·시스템 파일 포함",
                            "끄면 숨김 속성 파일을 건너뜁니다. 기본값은 전체 포함입니다.",
                            job.include_hidden,
                            cx.listener(|this, checked: &bool, window, cx| {
                                let checked = *checked;
                                this.update_selected_sync_job(window, cx, move |job| {
                                    job.include_hidden = checked
                                });
                            }),
                            cx,
                        ))
                        .child(ui::option_row(
                            "sync-opt-mirror",
                            "원본에서 삭제된 항목 반영",
                            "대상에만 있는 파일을 삭제합니다. 되돌릴 수 없으니 주의하세요.",
                            job.mirror_deletes,
                            cx.listener(|this, checked: &bool, window, cx| {
                                let checked = *checked;
                                this.update_selected_sync_job(window, cx, move |job| {
                                    job.mirror_deletes = checked
                                });
                            }),
                            cx,
                        )),
                ),
        )
        .into_any_element()
}

// ─────────────────────────────────────────────
// 실패 기록
// ─────────────────────────────────────────────

fn render_failures(this: &mut AppRoot, cx: &mut Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();
    let fg = theme.foreground;
    let muted_fg = theme.muted_foreground;
    let border = theme.border;
    let card = theme.secondary;
    let neutral_btn = ButtonStyle::neutral(cx).hover(border);

    let mut failures = v_flex().gap_1();
    if this.sync_failures.is_empty() {
        failures = failures.child(
            div()
                .text_color(muted_fg)
                .child("동기화 실패 기록이 없습니다."),
        );
    } else {
        for (ix, failure) in this.sync_failures.iter().enumerate().take(50) {
            let key = failure.key();
            let suppressed = this.suppressed_sync_failures.contains(&key);
            failures = failures.child(
                h_flex()
                    .debug_selector(move || format!("sync-failure-row-{ix}"))
                    .gap_2()
                    .items_center()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(theme.list)
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_color(fg).child(failure.path.clone()))
                            .child(div().text_color(theme.danger).child(failure.reason.clone())),
                    )
                    .child(
                        div()
                            .id(("sync-failure-mute", ix))
                            .rounded_md()
                            .px_2()
                            .py_1()
                            .cursor_pointer()
                            .bg(if suppressed { theme.muted } else { theme.list })
                            .border_1()
                            .border_color(border)
                            .text_color(if suppressed { muted_fg } else { fg })
                            .hover(|s| s.bg(theme.secondary_hover))
                            .on_click(cx.listener(move |this, _ev, _window, cx| {
                                this.toggle_sync_failure_suppression(&key);
                                cx.notify();
                            }))
                            .child(if suppressed {
                                "알림 꺼짐"
                            } else {
                                "알림 끄기"
                            }),
                    ),
            );
        }
    }

    let notify_enabled = this.sync_notify_enabled;
    let has_failures = !this.sync_failures.is_empty();

    div()
        .debug_selector(|| "file-sync-failures-card".to_string())
        .rounded_lg()
        .bg(card)
        .border_1()
        .border_color(border)
        .p_3()
        .child(
            v_flex()
                .gap_2()
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(div().text_color(fg).child("동기화 실패 기록"))
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(div().text_color(muted_fg).child("실패 알림 표시"))
                                .child(
                                    div()
                                        .debug_selector(|| "sync-notify-toggle".to_string())
                                        .child(
                                            ui::toggle_switch(
                                                "sync-notify-toggle",
                                                notify_enabled,
                                                cx,
                                            )
                                            .on_click(
                                                cx.listener(|this, checked: &bool, _window, cx| {
                                                    this.sync_notify_enabled = *checked;
                                                    cx.notify();
                                                }),
                                            ),
                                        ),
                                )
                                .children(has_failures.then(|| {
                                    ui::action_button(
                                        "sync-clear-failures",
                                        "기록 지우기",
                                        ui::Size::Md,
                                        neutral_btn,
                                        cx.listener(|this, _ev, _window, cx| {
                                            this.sync_failures.clear();
                                            cx.notify();
                                        }),
                                    )
                                })),
                        ),
                )
                .child(div().text_color(muted_fg).child(
                    "복사할 수 없는 파일은 건너뛰고 사유를 남깁니다. \
                         '알림 끄기'를 누르면 같은 항목의 토스트가 더 이상 뜨지 않습니다.",
                ))
                .child(failures),
        )
        .into_any_element()
}

// ─────────────────────────────────────────────
// 공통 조각
// ─────────────────────────────────────────────

