//! 로그 패널.
//!
//! 화면 로그를 가상 리스트로 표시하고, 롤링 로그 파일의 개수·용량·경로를 함께 안내한다.
//! 보관 정책 설정은 전역 설정 페이지(`window/settings.rs`)에 있다.

use gpui::{div, px, size, AnyElement, Context, IntoElement, ParentElement, Styled};
use gpui_component::{h_flex, theme::ActiveTheme, v_flex, v_virtual_list};
use std::{ops::Range, rc::Rc};

use crate::app::AppRoot;

pub fn render(root: &AppRoot, cx: &mut Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();

    if root.app_state.log_entries.is_empty() {
        return v_flex()
            .size_full()
            .gap_3()
            .child(div().text_color(theme.foreground).child("로그"))
            .child(
                div()
                    .rounded_lg()
                    .p_4()
                    .bg(theme.secondary)
                    .border_1()
                    .border_color(theme.border)
                    .text_color(theme.muted_foreground)
                    .child("기록된 로그가 없습니다."),
            )
            .into_any_element();
    }

    let item_sizes = Rc::new(
        root.app_state
            .log_entries
            .iter()
            .map(|_| size(px(0.), px(30.0)))
            .collect::<Vec<_>>(),
    );

    let scroll = root.log_scroll_handle.clone();
    let log_path = crate::logging::current_log_file();
    let (file_count, total_bytes) = crate::logging::log_dir_stats();

    v_flex()
        .size_full()
        .gap_3()
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .child(div().text_color(theme.foreground).child("로그"))
                .child(
                    div().text_color(theme.muted_foreground).child(format!(
                        "파일 {file_count}개 · {:.1} MB · {}",
                        total_bytes as f64 / (1024.0 * 1024.0),
                        log_path.display()
                    )),
                ),
        )
        .child(
            div()
                .rounded_lg()
                .p_2()
                .bg(theme.secondary)
                .border_1()
                .border_color(theme.border)
                .size_full()
                .child(
                    v_virtual_list(
                        cx.entity(),
                        "event-log-virtual-list",
                        item_sizes,
                        move |this, visible_range: Range<usize>, _window, cx| {
                            visible_range
                                .map(|ix| {
                                    let Some(entry) = this.app_state.log_entries.get(ix) else {
                                        return div();
                                    };

                                    let level_color = match entry.level.as_str() {
                                        "SUCCESS" => cx.theme().success,
                                        "WARN" => cx.theme().warning,
                                        "ERROR" => cx.theme().danger,
                                        _ => cx.theme().info,
                                    };

                                    div()
                                        .h(px(28.0))
                                        .px_2()
                                        .py_1()
                                        .border_b_1()
                                        .border_color(cx.theme().border)
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .w(px(72.0))
                                                        .text_color(level_color)
                                                        .child(entry.level.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .text_color(cx.theme().foreground)
                                                        .child(entry.message.clone()),
                                                ),
                                        )
                                })
                                .collect::<Vec<_>>()
                        },
                    )
                    .track_scroll(&scroll),
                ),
        )
        .into_any_element()
}
