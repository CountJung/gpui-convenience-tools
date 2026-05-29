use gpui::{div, AnyElement, Context, IntoElement, ParentElement, Styled, Window};
use gpui_component::{v_flex, theme::ActiveTheme};

#[allow(dead_code)]
pub fn render<V>(_window: &mut Window, cx: &mut Context<V>) -> AnyElement {
    let theme = cx.theme();

    v_flex()
        .size_full()
        .gap_3()
        .child(
            div()
                .text_color(theme.foreground)
                .child("로그"),
        )
        .child(
            div()
                .rounded_lg()
                .p_4()
                .bg(theme.secondary)
                .border_1()
                .border_color(theme.border)
                .text_color(theme.muted_foreground)
                .child("아직 로그가 없습니다."),
        )
        .into_any_element()
}
