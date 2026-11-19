//! 자동 시작 관리 뷰 (작업 스케줄러 방식)
//!
//! Windows 작업 스케줄러(Task Scheduler)를 통해 로그온 시 자동 시작을
//! UI에서 등록·삭제·즉시 실행할 수 있는 패널이다.
//!
//! ## Session 0 격리 문제와 해결책
//! SCM(Windows Service)은 Session 0(비대화형)에서 실행되어
//! 사용자 데스크톱 창(KakaoTalk 등)을 EnumWindows/ShowWindow로
//! 조작할 수 없다. 작업 스케줄러의 ONLOGON + /IT 트리거는
//! 사용자 세션(Session 1)에서 직접 실행되므로 이 문제가 없다.

use gpui::{div, AnyElement, Context, Hsla, IntoElement, ParentElement, Styled, Window};
use gpui_component::{h_flex, v_flex, theme::ActiveTheme};

use crate::app::AppRoot;
#[cfg(target_os = "windows")]
use crate::window::ui::{self, ButtonStyle, Tone};

#[cfg(target_os = "windows")]
use crate::platform::TaskState;
#[cfg(windows)]
mod controls;

// ─────────────────────────────────────────────
// 공통 색상 스냅샷
// ─────────────────────────────────────────────

/// 이 화면이 카드·배너에 직접 쓰는 색만 복사해 둔다.
///
/// 배지·버튼 색은 [`crate::window::ui`]가 `cx.theme()`에서 직접 읽으므로 여기에 두지 않는다.
#[cfg(target_os = "windows")]
struct ThemeSnap {
    foreground: Hsla,
    muted_foreground: Hsla,
    secondary: Hsla,
    border: Hsla,
    info: Hsla,
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
            info: t.info,
        }
    }
}

// ─────────────────────────────────────────────
// 상태 배지
// ─────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn state_badge(state: &TaskState, cx: &Context<AppRoot>) -> AnyElement {
    let (tone, label) = match state {
        TaskState::Ready        => (Tone::Success, "● 대기 중"),
        TaskState::Running      => (Tone::Info,    "▶ 실행 중"),
        TaskState::Disabled     => (Tone::Warning, "■ 비활성화"),
        TaskState::NotInstalled => (Tone::Muted,   "✕ 미등록"),
        TaskState::Unknown      => (Tone::Muted,   "? 알 수 없음"),
    };
    ui::badge(label, tone, ui::Size::Md, cx).into_any_element()
}

/// 자동 시작 패널이 열려 있는 동안에만 조회 결과와 삭제 확인 상태를 보관한다.
#[cfg(target_os = "windows")]
pub fn render(this: &mut AppRoot, window: &mut Window, cx: &mut Context<AppRoot>) -> AnyElement {
    let panel = window.use_keyed_state("auto-start-panel", cx, |_, cx| {
        controls::Panel::load(this.platform.clone(), cx)
    });
    let admin = this.platform.is_elevated();
    let state = panel.read(cx).state.as_ref();
    let state_desc = match state {
        None => "자동 시작 상태를 조회하고 있습니다.",
        Some(TaskState::NotInstalled) if !admin =>
            "자동 시작이 등록되지 않았습니다. 등록하려면 관리자 권한으로 앱을 다시 실행하세요.",
        Some(TaskState::NotInstalled) =>
            "자동 시작이 등록되지 않았습니다. '등록' 버튼으로 로그온 자동 시작을 설정하세요.",
        Some(TaskState::Ready) =>
            "로그온 시 자동 시작이 등록되어 있습니다. 다음 로그온 시 트레이 모드로 자동 시작됩니다.",
        Some(TaskState::Running) =>
            "작업이 현재 실행 중입니다.",
        Some(TaskState::Disabled) =>
            "작업이 비활성화되어 있습니다. 관리자 권한으로 삭제 후 다시 등록하세요.",
        Some(TaskState::Unknown) =>
            "작업 상태를 확인할 수 없습니다. 재조회 버튼을 눌러 주세요.",
    };

    let t = ThemeSnap::from_cx(cx);
    let badge = match state {
        Some(state) => state_badge(state, cx),
        None => ui::badge("조회 중", Tone::Muted, ui::Size::Md, cx).into_any_element(),
    };

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
                .child(controls::refresh_button(panel.clone(), cx)),
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
                        .child(controls::buttons(panel, admin, cx)),
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
