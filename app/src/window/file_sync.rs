//! 파일 동기화 패널.
//!
//! 좌측(기능 영역)은 동기화 작업 목록과 실행 상태·실패 기록을,
//! 우측(설정 영역)은 선택한 작업의 원본/대상/주기/옵션을 편집한다.
//! 두 영역은 스플리터([`h_resizable`])로 나뉜다.

use gpui::{
    div, px, AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{
    h_flex,
    input::Input,
    resizable::{h_resizable, resizable_panel},
    switch::Switch,
    theme::ActiveTheme,
    v_flex,
};

use crate::app::AppRoot;
use crate::window::scroll_pane;

const INTERVAL_PRESETS: [u32; 5] = [30, 60, 300, 900, 3600];

pub fn render(this: &mut AppRoot, window: &mut Window, cx: &mut Context<AppRoot>) -> AnyElement {
    this.ensure_sync_inputs(window, cx);

    let left_scroll = this.sync_left_scroll.clone();
    let right_scroll = this.sync_right_scroll.clone();

    let feature = render_job_list(this, cx);
    let settings = render_job_settings(this, cx);

    h_resizable("file-sync-split")
        .child(
            resizable_panel()
                .size(px(520.0))
                .size_range(px(320.0)..px(900.0))
                .child(scroll_pane("file-sync-left", &left_scroll, feature)),
        )
        .child(scroll_pane("file-sync-right", &right_scroll, settings))
        .into_any_element()
}

// ─────────────────────────────────────────────
// 좌측: 기능 영역 (작업 목록 + 상태 + 실패 기록)
// ─────────────────────────────────────────────

fn render_job_list(this: &mut AppRoot, cx: &mut Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();
    let fg = theme.foreground;
    let muted_fg = theme.muted_foreground;
    let border = theme.border;
    let card = theme.secondary;
    let selected_idx = this.selected_sync_job;

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
                                        div()
                                            .text_color(muted_fg)
                                            .child(format!("{interval}초")),
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

    // ── 실패 기록 ──
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
                            .child(
                                div()
                                    .text_color(theme.danger)
                                    .child(failure.reason.clone()),
                            ),
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
                            .child(if suppressed { "알림 꺼짐" } else { "알림 끄기" }),
                    ),
            );
        }
    }

    let notify_enabled = this.sync_notify_enabled;
    let has_failures = !this.sync_failures.is_empty();

    v_flex()
        .size_full()
        .gap_3()
        .p_1()
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .child(div().text_color(fg).child("파일 동기화"))
                .child(
                    h_flex()
                        .gap_2()
                        .child(action_button(
                            "sync-new-job",
                            "새 작업",
                            theme.list,
                            fg,
                            border,
                            cx.listener(|this, _ev, window, cx| {
                                this.add_sync_job(window, cx);
                            }),
                        ))
                        .child(action_button(
                            "sync-run-all",
                            "전체 지금 동기화",
                            theme.primary,
                            theme.primary_foreground,
                            theme.primary_hover,
                            cx.listener(|this, _ev, window, cx| {
                                this.request_sync_all(window, cx);
                            }),
                        )),
                ),
        )
        // ── 작업 목록 카드 ──
        .child(
            div()
                .rounded_lg()
                .bg(card)
                .border_1()
                .border_color(border)
                .child(v_flex().child(rows)),
        )
        // ── 실패 기록 카드 ──
        .child(
            div()
                .flex_1()
                .min_h_0()
                .rounded_lg()
                .bg(card)
                .border_1()
                .border_color(border)
                .p_3()
                .child(
                    v_flex()
                        .gap_2()
                        .size_full()
                        .child(
                            h_flex()
                                .justify_between()
                                .items_center()
                                .child(div().text_color(fg).child("동기화 실패 기록"))
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(
                                            div()
                                                .text_color(muted_fg)
                                                .child("실패 알림 표시"),
                                        )
                                        .child(
                                            Switch::new("sync-notify-toggle")
                                                .checked(notify_enabled)
                                                .on_click(cx.listener(
                                                    |this, checked: &bool, _window, cx| {
                                                        this.sync_notify_enabled = *checked;
                                                        cx.notify();
                                                    },
                                                )),
                                        )
                                        .children(has_failures.then(|| {
                                            action_button(
                                                "sync-clear-failures",
                                                "기록 지우기",
                                                theme.list,
                                                fg,
                                                border,
                                                cx.listener(|this, _ev, _window, cx| {
                                                    this.sync_failures.clear();
                                                    cx.notify();
                                                }),
                                            )
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .text_color(muted_fg)
                                .child(
                                    "복사할 수 없는 파일은 건너뛰고 사유를 남깁니다. \
                                     '알림 끄기'를 누르면 같은 항목의 토스트가 더 이상 뜨지 않습니다.",
                                ),
                        )
                        .child(failures),
                ),
        )
        .into_any_element()
}

// ─────────────────────────────────────────────
// 우측: 설정 영역 (선택한 작업 편집)
// ─────────────────────────────────────────────

fn render_job_settings(this: &mut AppRoot, cx: &mut Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();
    let fg = theme.foreground;
    let muted_fg = theme.muted_foreground;
    let border = theme.border;
    let card = theme.secondary;
    let selected_bg = theme.primary;
    let selected_fg = theme.primary_foreground;

    let Some(selected) = this.selected_sync_job else {
        return v_flex()
            .size_full()
            .p_1()
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
                    .child("좌측에서 작업을 선택하거나 '새 작업'을 추가하세요."),
            )
            .into_any_element();
    };

    let Some(job) = this.sync_jobs.get(selected).cloned() else {
        return div().into_any_element();
    };

    let name_input = this.sync_name_input.clone();
    let source_input = this.sync_source_input.clone();
    let target_input = this.sync_target_input.clone();

    let mut interval_row = h_flex().gap_2().flex_wrap();
    for secs in INTERVAL_PRESETS {
        let is_selected = job.interval_secs == secs;
        interval_row = interval_row.child(
            div()
                .id(("sync-interval", secs as usize))
                .rounded_md()
                .px_3()
                .py_2()
                .cursor_pointer()
                .bg(if is_selected { selected_bg } else { theme.list })
                .text_color(if is_selected { selected_fg } else { fg })
                .border_1()
                .border_color(if is_selected { theme.primary_hover } else { border })
                .hover(|s| s.bg(theme.secondary_hover))
                .on_click(cx.listener(move |this, _ev, window, cx| {
                    this.update_selected_sync_job(window, cx, |job| job.interval_secs = secs);
                }))
                .child(format_interval(secs)),
        );
    }

    v_flex()
        .size_full()
        .p_1()
        .gap_3()
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .child(div().text_color(fg).child("작업 설정"))
                .child(
                    h_flex()
                        .gap_2()
                        .child(action_button(
                            "sync-run-one",
                            "지금 동기화",
                            theme.primary,
                            theme.primary_foreground,
                            theme.primary_hover,
                            cx.listener(move |this, _ev, window, cx| {
                                this.request_sync_job(selected, window, cx);
                            }),
                        ))
                        .child(action_button(
                            "sync-delete-job",
                            "작업 삭제",
                            theme.danger,
                            theme.danger_foreground,
                            theme.danger_active,
                            cx.listener(move |this, _ev, window, cx| {
                                this.remove_sync_job(selected, window, cx);
                            }),
                        )),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
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
                                .child(action_button(
                                    "sync-pick-source",
                                    "찾아보기",
                                    theme.list,
                                    fg,
                                    border,
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
                                .child(action_button(
                                    "sync-pick-target",
                                    "찾아보기",
                                    theme.list,
                                    fg,
                                    border,
                                    cx.listener(|this, _ev, window, cx| {
                                        this.pick_sync_folder(false, window, cx);
                                    }),
                                )),
                        )
                        .child(
                            div()
                                .text_color(muted_fg)
                                .child("경로를 직접 입력했다면 '경로 적용'을 눌러 저장하세요."),
                        )
                        .child(action_button(
                            "sync-apply-paths",
                            "경로 적용",
                            theme.list,
                            fg,
                            border,
                            cx.listener(|this, _ev, window, cx| {
                                this.apply_sync_inputs(window, cx);
                            }),
                        ))
                        // ── 감시 주기 ──
                        .child(div().text_color(fg).child("감시 주기"))
                        .child(interval_row)
                        // ── 옵션 ──
                        .child(div().text_color(fg).child("옵션"))
                        .child(option_row(
                            "sync-opt-enabled",
                            "자동 동기화 사용",
                            "감시 주기마다 자동으로 실행합니다.",
                            job.enabled,
                            muted_fg,
                            fg,
                            cx.listener(|this, checked: &bool, window, cx| {
                                let checked = *checked;
                                this.update_selected_sync_job(window, cx, move |job| {
                                    job.enabled = checked
                                });
                            }),
                        ))
                        .child(option_row(
                            "sync-opt-hidden",
                            "숨김·시스템 파일 포함",
                            "끄면 숨김 속성 파일을 건너뜁니다. 기본값은 전체 포함입니다.",
                            job.include_hidden,
                            muted_fg,
                            fg,
                            cx.listener(|this, checked: &bool, window, cx| {
                                let checked = *checked;
                                this.update_selected_sync_job(window, cx, move |job| {
                                    job.include_hidden = checked
                                });
                            }),
                        ))
                        .child(option_row(
                            "sync-opt-mirror",
                            "원본에서 삭제된 항목 반영",
                            "대상에만 있는 파일을 삭제합니다. 되돌릴 수 없으니 주의하세요.",
                            job.mirror_deletes,
                            muted_fg,
                            fg,
                            cx.listener(|this, checked: &bool, window, cx| {
                                let checked = *checked;
                                this.update_selected_sync_job(window, cx, move |job| {
                                    job.mirror_deletes = checked
                                });
                            }),
                        )),
                ),
        )
        .into_any_element()
}

// ─────────────────────────────────────────────
// 공통 조각
// ─────────────────────────────────────────────

fn action_button(
    id: &'static str,
    label: &'static str,
    bg: gpui::Hsla,
    fg: gpui::Hsla,
    accent: gpui::Hsla,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .rounded_md()
        .px_3()
        .py_1()
        .cursor_pointer()
        .bg(bg)
        .border_1()
        .border_color(accent)
        .text_color(fg)
        .hover(|s| s.bg(accent))
        .on_click(on_click)
        .child(label)
        .into_any_element()
}

fn option_row(
    id: &'static str,
    title: &'static str,
    description: &'static str,
    checked: bool,
    muted_fg: gpui::Hsla,
    fg: gpui::Hsla,
    on_click: impl Fn(&bool, &mut Window, &mut gpui::App) + 'static,
) -> AnyElement {
    h_flex()
        .gap_3()
        .items_center()
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(div().text_color(fg).child(title))
                .child(div().text_color(muted_fg).child(description)),
        )
        .child(Switch::new(id).checked(checked).on_click(on_click))
        .into_any_element()
}

fn format_interval(secs: u32) -> String {
    if secs >= 3600 {
        format!("{}시간", secs / 3600)
    } else if secs >= 60 {
        format!("{}분", secs / 60)
    } else {
        format!("{secs}초")
    }
}
