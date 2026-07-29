//! 웹뷰 광고 차단 패널.
//!
//! 좌측(기능 영역)은 차단 상태와 등록된 타겟 앱 목록을,
//! 우측(설정 영역)은 스캔 주기와 실행 중인 프로세스 추가 UI를 담당한다.
//! 두 영역은 스플리터([`h_resizable`])로 나뉜다.
//!
//! ## 동작 원리
//! 타겟 프로세스의 최상위 창을 `EnumWindows`로 찾은 뒤, 자식 창 중
//! WebView 계열 클래스(`Chrome_WidgetWin_1` 등)를 `ShowWindow(SW_HIDE)`로 숨긴다.

use gpui::{
    div, px, AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{
    h_flex,
    resizable::{h_resizable, resizable_panel},
    switch::Switch,
    theme::ActiveTheme,
    v_flex,
};

use crate::app::AppRoot;
use crate::window::scroll_pane;
use crate::window::ui::{self, Tone};

const INTERVAL_PRESETS: [u32; 5] = [5, 10, 30, 60, 120];

pub fn render(this: &mut AppRoot, _window: &mut Window, cx: &mut Context<AppRoot>) -> AnyElement {
    let left_scroll = this.ad_left_scroll.clone();
    let right_scroll = this.ad_right_scroll.clone();

    let feature = render_status_and_targets(this, cx);
    let settings = render_settings(this, cx);

    h_resizable("ad-block-split")
        .child(
            resizable_panel()
                .size(px(560.0))
                .size_range(px(340.0)..px(1000.0))
                .child(scroll_pane("ad-block-left", &left_scroll, feature)),
        )
        .child(scroll_pane("ad-block-right", &right_scroll, settings))
        .into_any_element()
}

// ─────────────────────────────────────────────
// 좌측: 상태 + 타겟 목록
// ─────────────────────────────────────────────

fn render_status_and_targets(this: &mut AppRoot, cx: &mut Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();
    let fg = theme.foreground;
    let muted_fg = theme.muted_foreground;
    let border = theme.border;
    let card = theme.secondary;

    let state = this.app_state();
    let is_active = state.is_active;
    let is_running = state.is_target_running;
    let blocked_count = state.blocked_count;
    let total_targets = state.targets.len();
    let active_targets = state.targets.iter().filter(|t| t.enabled).count();

    let (svc_label, svc_tone) = if is_active {
        ("차단 동작 중", Tone::Success)
    } else {
        ("차단 중지됨", Tone::Warning)
    };
    let (tgt_label, tgt_tone) = if is_running {
        ("타겟 실행 중", Tone::Success)
    } else {
        ("타겟 미실행", Tone::Muted)
    };

    // ── 타겟 목록 ──
    let mut target_rows = v_flex();
    if state.targets.is_empty() {
        target_rows = target_rows.child(
            div()
                .px_3()
                .py_4()
                .text_color(muted_fg)
                .child("등록된 타겟이 없습니다. 우측에서 실행 중인 프로세스를 추가하세요."),
        );
    } else {
        for (ix, target) in state.targets.iter().enumerate() {
            let enabled = target.enabled;
            let display = target.display_name.clone();
            let process = target.process_name.clone();
            let class = target.ad_window_class.clone();

            target_rows = target_rows.child(
                h_flex()
                    .h(px(48.0))
                    .px_3()
                    .gap_2()
                    .items_center()
                    .border_b_1()
                    .border_color(border)
                    .hover(|s| s.bg(theme.secondary_hover))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_color(fg).child(display))
                            .child(div().text_color(muted_fg).child(process)),
                    )
                    .child(div().w(px(150.0)).text_color(muted_fg).child(class))
                    .child(
                        div()
                            .w(px(56.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Switch::new(("target-switch", ix))
                                    .checked(enabled)
                                    .on_click(cx.listener(
                                        move |this, checked: &bool, window, cx| {
                                            this.set_target_enabled(ix, *checked, window, cx);
                                        },
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .id(("target-remove", ix))
                            .w(px(36.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .py_1()
                            .cursor_pointer()
                            .text_color(theme.danger)
                            .hover(|s| s.bg(theme.danger).text_color(theme.danger_foreground))
                            .on_click(cx.listener(move |this, _ev, window, cx| {
                                this.remove_target(ix, window, cx);
                            }))
                            .child("×"),
                    ),
            );
        }
    }

    v_flex()
        .w_full()
        .gap_3()
        .p_1()
        .child(div().text_color(fg).child("웹뷰 광고 차단"))
        // ── 상태 카드 ──
        .child(
            div()
                .rounded_lg()
                .p_4()
                .bg(card)
                .border_1()
                .border_color(border)
                .child(
                    v_flex()
                        .gap_3()
                        .child(
                            h_flex()
                                .gap_2()
                                .child(ui::badge(svc_label, svc_tone, ui::Size::Md, cx))
                                .child(ui::badge(tgt_label, tgt_tone, ui::Size::Md, cx)),
                        )
                        .child(
                            h_flex()
                                .gap_3()
                                .child(stat_card(
                                    "누적 차단",
                                    &blocked_count.to_string(),
                                    theme.list,
                                    border,
                                    fg,
                                    muted_fg,
                                ))
                                .child(stat_card(
                                    "활성 타겟",
                                    &format!("{active_targets} / {total_targets}"),
                                    theme.list,
                                    border,
                                    fg,
                                    muted_fg,
                                ))
                                .child(stat_card(
                                    "스캔 주기",
                                    &format!("{}초", this.scan_interval_secs),
                                    theme.list,
                                    border,
                                    fg,
                                    muted_fg,
                                )),
                        ),
                ),
        )
        // ── 타겟 목록 카드 ──
        .child(
            div()
                .rounded_lg()
                .bg(card)
                .border_1()
                .border_color(border)
                .child(
                    v_flex()
                        .child(
                            h_flex()
                                .px_3()
                                .py_2()
                                .border_b_1()
                                .border_color(border)
                                .gap_2()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_color(muted_fg)
                                        .child("표시 이름 / 프로세스"),
                                )
                                .child(div().w(px(150.0)).text_color(muted_fg).child("광고 창 클래스"))
                                .child(div().w(px(56.0)).text_color(muted_fg).child("활성"))
                                .child(div().w(px(36.0))),
                        )
                        .child(target_rows),
                ),
        )
        .into_any_element()
}

// ─────────────────────────────────────────────
// 우측: 광고 차단 설정
// ─────────────────────────────────────────────

fn render_settings(this: &mut AppRoot, cx: &mut Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();
    let fg = theme.foreground;
    let muted_fg = theme.muted_foreground;
    let border = theme.border;
    let card = theme.secondary;
    let selected_bg = theme.primary;
    let selected_fg = theme.primary_foreground;
    let current_interval = this.scan_interval_secs;
    let is_active = this.app_state().is_active;

    let mut interval_row = h_flex().gap_2().flex_wrap();
    for secs in INTERVAL_PRESETS {
        let is_selected = current_interval == secs;
        interval_row = interval_row.child(
            div()
                .id(("ad-interval-preset", secs as usize))
                .rounded_md()
                .px_3()
                .py_2()
                .cursor_pointer()
                .bg(if is_selected { selected_bg } else { theme.list })
                .text_color(if is_selected { selected_fg } else { fg })
                .border_1()
                .border_color(if is_selected { theme.primary_hover } else { border })
                .hover(|s| s.bg(theme.secondary_hover))
                .on_click(cx.listener(move |this, _ev, _window, cx| {
                    this.set_scan_interval(secs, cx);
                }))
                .child(format!("{secs}초")),
        );
    }

    // ── 실행 중인 프로세스 목록 ──
    let targets = this.app_state().targets.clone();
    let mut proc_rows = v_flex();
    if this.running_processes.is_empty() {
        proc_rows = proc_rows.child(
            div()
                .px_3()
                .py_3()
                .text_color(muted_fg)
                .child("표시할 프로세스가 없습니다. 새로고침을 눌러보세요."),
        );
    }
    for (ix, process_name) in this.running_processes.clone().iter().enumerate() {
        let exists = targets
            .iter()
            .any(|t| t.process_name.eq_ignore_ascii_case(process_name));
        let name = process_name.clone();

        let action: AnyElement = if exists {
            div()
                .w(px(64.0))
                .rounded_md()
                .px_2()
                .py_1()
                .bg(theme.muted)
                .text_color(muted_fg)
                .child("등록됨")
                .into_any_element()
        } else {
            div()
                .id(("proc-add", ix))
                .w(px(64.0))
                .rounded_md()
                .px_2()
                .py_1()
                .cursor_pointer()
                .bg(theme.list)
                .border_1()
                .border_color(border)
                .text_color(fg)
                .hover(|s| s.bg(theme.secondary_hover))
                .on_click(cx.listener(move |this, _ev, window, cx| {
                    this.add_target_process(&name, window, cx);
                }))
                .child("+ 추가")
                .into_any_element()
        };

        proc_rows = proc_rows.child(
            h_flex()
                .h(px(34.0))
                .px_3()
                .gap_2()
                .items_center()
                .border_b_1()
                .border_color(border)
                .hover(|s| s.bg(theme.secondary_hover))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_color(if exists { muted_fg } else { fg })
                        .child(process_name.clone()),
                )
                .child(action),
        );
    }

    v_flex()
        .w_full()
        .gap_3()
        .p_1()
        .child(div().text_color(fg).child("광고 차단 설정"))
        // ── 전역 활성 + 스캔 주기 ──
        .child(
            div()
                .rounded_lg()
                .bg(card)
                .border_1()
                .border_color(border)
                .p_4()
                .child(
                    v_flex()
                        .gap_3()
                        .child(
                            h_flex()
                                .gap_3()
                                .items_center()
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .child(div().text_color(fg).child("광고 차단 사용"))
                                        .child(
                                            div()
                                                .text_color(muted_fg)
                                                .child("끄면 스캔과 창 숨김을 모두 중단합니다."),
                                        ),
                                )
                                .child(
                                    Switch::new("ad-block-enable")
                                        .checked(is_active)
                                        .on_click(cx.listener(
                                            |this, checked: &bool, window, cx| {
                                                this.set_service_enabled(*checked, window, cx);
                                            },
                                        )),
                                ),
                        )
                        .child(div().text_color(fg).child("스캔 주기"))
                        .child(
                            div()
                                .text_color(muted_fg)
                                .child(format!("광고 창 감지 주기입니다. 현재: {current_interval}초")),
                        )
                        .child(interval_row),
                ),
        )
        // ── 실행 중인 프로세스 ──
        .child(
            div()
                .rounded_lg()
                .bg(card)
                .border_1()
                .border_color(border)
                .child(
                    v_flex()
                        .child(
                            h_flex()
                                .px_3()
                                .py_2()
                                .border_b_1()
                                .border_color(border)
                                .justify_between()
                                .items_center()
                                .child(div().text_color(fg).child("실행 중인 프로세스"))
                                .child(
                                    div()
                                        .id("refresh-running-processes")
                                        .rounded_md()
                                        .px_3()
                                        .py_1()
                                        .cursor_pointer()
                                        .bg(theme.list)
                                        .border_1()
                                        .border_color(border)
                                        .text_color(fg)
                                        .hover(|s| s.bg(theme.secondary_hover))
                                        .on_click(cx.listener(|this, _ev, _window, cx| {
                                            this.refresh_running_processes();
                                            cx.notify();
                                        }))
                                        .child("새로고침"),
                                ),
                        )
                        .child(
                            div()
                                .px_3()
                                .py_2()
                                .text_color(muted_fg)
                                .child("창을 가진 프로세스만 표시됩니다."),
                        )
                        .child(proc_rows),
                ),
        )
        .into_any_element()
}

// ─────────────────────────────────────────────
// 공통 조각
// ─────────────────────────────────────────────

fn stat_card(
    label: &'static str,
    value: &str,
    bg: gpui::Hsla,
    border: gpui::Hsla,
    fg: gpui::Hsla,
    muted_fg: gpui::Hsla,
) -> AnyElement {
    div()
        .flex_1()
        .rounded_md()
        .px_3()
        .py_3()
        .bg(bg)
        .border_1()
        .border_color(border)
        .child(
            v_flex()
                .gap_1()
                .child(div().text_color(muted_fg).child(label))
                .child(div().text_color(fg).child(value.to_string())),
        )
        .into_any_element()
}
