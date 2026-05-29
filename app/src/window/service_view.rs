/// 자동 시작 관리 뷰 (작업 스케줄러 방식)
///
/// Windows 작업 스케줄러(Task Scheduler)를 통해 로그온 시 자동 시작을
/// UI에서 등록·삭제·즉시 실행할 수 있는 패널이다.
///
/// ## Session 0 격리 문제와 해결책
/// SCM(Windows Service)은 Session 0(비대화형)에서 실행되어
/// 사용자 데스크톱 창(KakaoTalk 등)을 EnumWindows/ShowWindow로
/// 조작할 수 없다. 작업 스케줄러의 ONLOGON + /IT 트리거는
/// 사용자 세션(Session 1)에서 직접 실행되므로 이 문제가 없다.

use gpui::{div, AnyElement, Context, Hsla, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window};
use gpui_component::{h_flex, v_flex, theme::ActiveTheme};

use crate::app::AppRoot;

#[cfg(target_os = "windows")]
use crate::platform::{
    TaskState, install_task, uninstall_task, run_task_now, query_task_state,
};

// ─────────────────────────────────────────────
// 공통 색상 스냅샷
// ─────────────────────────────────────────────

#[cfg(target_os = "windows")]
struct ThemeSnap {
    foreground: Hsla,
    muted_foreground: Hsla,
    secondary: Hsla,
    border: Hsla,
    muted: Hsla,
    primary: Hsla,
    primary_fg: Hsla,
    danger: Hsla,
    danger_fg: Hsla,
    success: Hsla,
    success_fg: Hsla,
    warning: Hsla,
    warning_fg: Hsla,
    info: Hsla,
    info_fg: Hsla,
}

#[cfg(target_os = "windows")]
impl ThemeSnap {
    fn from_cx(cx: &Context<AppRoot>) -> Self {
        let t = cx.theme();
        Self {
            foreground: t.foreground,
            muted_foreground: t.muted_foreground,
            secondary: t.secondary,
            border: t.border,
            muted: t.muted,
            primary: t.sidebar_primary,
            primary_fg: t.sidebar_primary_foreground,
            danger: t.danger,
            danger_fg: t.danger_foreground,
            success: t.success,
            success_fg: t.success_foreground,
            warning: t.warning,
            warning_fg: t.warning_foreground,
            info: t.info,
            info_fg: t.info_foreground,
        }
    }
}

// ─────────────────────────────────────────────
// 상태 배지
// ─────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn state_badge(state: &TaskState, t: &ThemeSnap) -> AnyElement {
    let (bg, fg, label) = match state {
        TaskState::Ready      => (t.success, t.success_fg, "● 대기 중"),
        TaskState::Running    => (t.info,    t.info_fg,    "▶ 실행 중"),
        TaskState::Disabled   => (t.warning, t.warning_fg, "■ 비활성화"),
        TaskState::NotInstalled => (t.muted, t.muted_foreground, "✕ 미등록"),
        TaskState::Unknown    => (t.muted,   t.muted_foreground, "? 알 수 없음"),
    };
    div()
        .rounded_md()
        .px_3()
        .py_1()
        .bg(bg)
        .text_color(fg)
        .child(label)
        .into_any_element()
}

// ─────────────────────────────────────────────
// 액션 버튼
// ─────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn action_button(
    id: &'static str,
    label: &'static str,
    enabled: bool,
    danger: bool,
    t: &ThemeSnap,
    cx: &mut Context<AppRoot>,
    on_click: impl Fn(&mut AppRoot, &gpui::ClickEvent, &mut Window, &mut Context<AppRoot>) + 'static,
) -> AnyElement {
    let (bg, fg) = if danger {
        (t.danger, t.danger_fg)
    } else {
        (t.primary, t.primary_fg)
    };
    let muted_bg = t.muted;
    let muted_fg = t.muted_foreground;
    let border = t.border;

    div()
        .rounded_md()
        .px_4()
        .py_2()
        .cursor_pointer()
        .bg(if enabled { bg } else { muted_bg })
        .text_color(if enabled { fg } else { muted_fg })
        .border_1()
        .border_color(border)
        .id(id)
        .on_click(cx.listener(on_click))
        .child(label)
        .into_any_element()
}

// ─────────────────────────────────────────────
// 메인 렌더 함수 (Windows)
// ─────────────────────────────────────────────

#[cfg(target_os = "windows")]
pub fn render(_this: &mut AppRoot, _window: &mut Window, cx: &mut Context<AppRoot>) -> AnyElement {
    let state = query_task_state();

    let is_installed = !matches!(state, TaskState::NotInstalled);
    let is_not_installed = !is_installed;

    let state_desc = match &state {
        TaskState::NotInstalled =>
            "자동 시작이 등록되지 않았습니다. '등록' 버튼으로 로그온 자동 시작을 설정하세요.",
        TaskState::Ready =>
            "로그온 시 자동 시작이 등록되어 있습니다. 다음 로그온 시 트레이 모드로 자동 시작됩니다.",
        TaskState::Running =>
            "작업이 현재 실행 중입니다.",
        TaskState::Disabled =>
            "작업이 비활성화되어 있습니다. 삭제 후 다시 등록하세요.",
        TaskState::Unknown =>
            "작업 상태를 조회할 수 없습니다.",
    };

    let t = ThemeSnap::from_cx(cx);
    let badge = state_badge(&state, &t);

    v_flex()
        .w_full()
        .min_w_0()
        .gap_4()
        // ── 헤더 ──
        .child(
            h_flex()
                .w_full()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_color(t.foreground)
                        .child("자동 시작 (Task Scheduler)"),
                )
                .child(
                    div()
                        .rounded_md()
                        .px_3()
                        .py_1()
                        .cursor_pointer()
                        .bg(t.secondary)
                        .text_color(t.foreground)
                        .border_1()
                        .border_color(t.border)
                        .id("task-refresh")
                        .on_click(cx.listener(|_this, _ev, _window, cx| {
                            cx.notify();
                        }))
                        .child("↺ 새로고침"),
                ),
        )
        // ── 상태 카드 ──
        .child(
            div()
                .w_full()
                .rounded_lg()
                .p_4()
                .bg(t.secondary)
                .border_1()
                .border_color(t.border)
                .child(
                    v_flex()
                        .w_full()
                        .gap_3()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_3()
                                .child(div().text_color(t.muted_foreground).child("상태"))
                                .child(badge),
                        )
                        .child(div().text_color(t.muted_foreground).child(state_desc)),
                ),
        )
        // ── 작업 정보 카드 ──
        .child(
            div()
                .w_full()
                .rounded_lg()
                .p_4()
                .bg(t.secondary)
                .border_1()
                .border_color(t.border)
                .child(
                    v_flex()
                        .w_full()
                        .gap_2()
                        .child(div().text_color(t.foreground).child("작업 정보"))
                        .child(
                            h_flex().gap_2()
                                .child(div().text_color(t.muted_foreground).child("작업 이름:"))
                                .child(div().text_color(t.foreground).child("gpui-convenience-tools")),
                        )
                        .child(
                            h_flex().gap_2()
                                .child(div().text_color(t.muted_foreground).child("트리거:"))
                                .child(div().text_color(t.foreground).child("로그온 시 (ONLOGON)")),
                        )
                        .child(
                            h_flex().gap_2()
                                .child(div().text_color(t.muted_foreground).child("실행 인수:"))
                                .child(div().text_color(t.foreground).child("--tray (트레이 최소화 시작)")),
                        )
                        .child(
                            h_flex().gap_2()
                                .child(div().text_color(t.muted_foreground).child("세션:"))
                                .child(div().text_color(t.foreground).child("사용자 세션 (Session 1)")),
                        ),
                ),
        )
        // ── 컨트롤 버튼 ──
        .child(
            div()
                .w_full()
                .rounded_lg()
                .p_4()
                .bg(t.secondary)
                .border_1()
                .border_color(t.border)
                .child(
                    v_flex()
                        .w_full()
                        .gap_3()
                        .child(div().text_color(t.foreground).child("작업 제어"))
                        .child(
                            h_flex()
                                .gap_3()
                                .flex_wrap()
                                .child(action_button(
                                    "task-install",
                                    "등록",
                                    is_not_installed,
                                    false,
                                    &t,
                                    cx,
                                    |this, _ev, window, cx| {
                                        match install_task() {
                                            Ok(()) => this.push_service_log(
                                                "자동 시작 작업을 등록했습니다. 다음 로그온 시 자동으로 시작됩니다.",
                                                window, cx,
                                            ),
                                            Err(e) => this.push_service_log(
                                                &format!("등록 실패: {e}"), window, cx,
                                            ),
                                        }
                                        cx.notify();
                                    },
                                ))
                                .child(action_button(
                                    "task-run-now",
                                    "지금 실행",
                                    is_installed,
                                    false,
                                    &t,
                                    cx,
                                    |this, _ev, window, cx| {
                                        match run_task_now() {
                                            Ok(()) => this.push_service_log(
                                                "앱을 트레이 모드로 즉시 실행했습니다.",
                                                window, cx,
                                            ),
                                            Err(e) => this.push_service_log(
                                                &format!("실행 실패: {e}"), window, cx,
                                            ),
                                        }
                                        cx.notify();
                                    },
                                ))
                                .child(action_button(
                                    "task-uninstall",
                                    "삭제",
                                    is_installed,
                                    true,
                                    &t,
                                    cx,
                                    |this, _ev, window, cx| {
                                        match uninstall_task() {
                                            Ok(()) => this.push_service_log(
                                                "자동 시작 작업을 삭제했습니다.",
                                                window, cx,
                                            ),
                                            Err(e) => this.push_service_log(
                                                &format!("삭제 실패: {e}"), window, cx,
                                            ),
                                        }
                                        cx.notify();
                                    },
                                )),
                        ),
                ),
        )
        // ── Session 0 해결 안내 ──
        .child(
            div()
                .w_full()
                .rounded_lg()
                .p_4()
                .bg(t.info.opacity(0.12))
                .border_1()
                .border_color(t.info.opacity(0.4))
                .child(
                    v_flex()
                        .w_full()
                        .gap_2()
                        .child(div().text_color(t.info).child("ℹ  작업 스케줄러 방식 안내"))
                        .child(
                            div().text_color(t.foreground).child(
                                "Windows 서비스(SCM)는 Session 0에서 실행되어 \
                                사용자 세션의 창(KakaoTalk 등)을 조작할 수 없습니다. \
                                작업 스케줄러의 ONLOGON + /IT 트리거는 \
                                사용자 세션(Session 1)에서 직접 실행되므로 \
                                재부팅 후에도 광고 차단이 정상 동작합니다.",
                            ),
                        ),
                ),
        )
        .into_any_element()
}

/// Windows 외 플랫폼에서는 미지원 메시지를 표시한다.
#[cfg(not(target_os = "windows"))]
pub fn render(_this: &mut AppRoot, _window: &mut Window, cx: &mut Context<AppRoot>) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .child(
            div()
                .text_color(muted)
                .child("자동 시작 관리는 Windows 플랫폼에서만 지원됩니다."),
        )
        .into_any_element()
}
