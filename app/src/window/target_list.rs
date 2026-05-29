use gpui::{div, AnyElement, Context, IntoElement, ParentElement, Styled, Window};
use gpui_component::{v_flex, theme::ActiveTheme};

use crate::app::TargetApp;

#[allow(dead_code)]
pub fn render<V>(_window: &mut Window, cx: &mut Context<V>, targets: &[TargetApp]) -> AnyElement {
    let theme = cx.theme();

    let target_rows = if targets.is_empty() {
        v_flex()
            .gap_2()
            .child(div().text_color(theme.muted_foreground).child("등록된 타겟 앱이 없습니다."))
            .into_any_element()
    } else {
        targets
            .iter()
            .fold(v_flex().gap_2(), |list, target| {
                list.child(
                    div()
                        .rounded_md()
                        .px_3()
                        .py_2()
                        .bg(theme.list)
                        .border_1()
                        .border_color(theme.border)
                        .child(
                            v_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .text_color(theme.foreground)
                                        .child(format!("{} ({})", target.display_name, target.process_name)),
                                )
                                .child(
                                    div()
                                        .text_color(theme.muted_foreground)
                                        .child(format!(
                                            "enabled: {}, class: {}",
                                            target.enabled, target.ad_window_class
                                        )),
                                ),
                        ),
                )
            })
            .into_any_element()
    };

    v_flex()
        .size_full()
        .gap_3()
        .child(
            div()
                .text_color(theme.foreground)
                .child("타겟 앱 목록"),
        )
        .child(
            div()
                .rounded_lg()
                .p_4()
                .bg(theme.secondary)
                .border_1()
                .border_color(theme.border)
                .child(target_rows),
        )
        .into_any_element()
}
