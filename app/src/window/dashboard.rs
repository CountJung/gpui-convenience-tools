use gpui::{div, AnyElement, Context, IntoElement, ParentElement, Styled, Window};
use gpui_component::{v_flex, theme::ActiveTheme};

use crate::app::AppState;

#[allow(dead_code)]
pub fn render<V>(_window: &mut Window, cx: &mut Context<V>, app_state: &AppState) -> AnyElement {
    let theme = cx.theme();
    let status_text = if app_state.is_active {
        "KakaoTalk.exe 실행 감지됨"
    } else {
        "KakaoTalk.exe 미실행 또는 광고 창 미감지"
    };
    let status_bg = if app_state.is_active {
        theme.success
    } else {
        theme.warning
    };
    let status_fg = if app_state.is_active {
        theme.success_foreground
    } else {
        theme.warning_foreground
    };

    v_flex()
        .size_full()
        .gap_3()
        .child(
            div()
                .text_color(theme.foreground)
                .child("대시보드"),
        )
        .child(
            div()
                .rounded_lg()
                .p_4()
                .bg(theme.secondary)
                .border_1()
                .border_color(theme.border)
                .child(
                    v_flex()
                        .gap_2()
                        .child(
                            div()
                                .rounded_md()
                                .px_3()
                                .py_2()
                                .bg(status_bg)
                                .text_color(status_fg)
                                .child(status_text),
                        )
                        .child(
                            div()
                                .text_color(theme.foreground)
                                .child(format!("설정된 타겟 앱 수: {}", app_state.targets.len())),
                        )
                        .child(
                            div()
                                .text_color(theme.foreground)
                                .child(format!("누적 차단 횟수: {}", app_state.blocked_count)),
                        ),
                ),
        )
        .into_any_element()
}
