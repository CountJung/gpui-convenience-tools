//! 로그 패널.
//!
//! 화면 로그를 가상 리스트로 표시하고, 롤링 로그 파일의 개수·용량·경로를 함께 안내한다.
//! 보관 정책 설정은 전역 설정 페이지(`window/settings.rs`)에 있다.

use gpui::{
    div, px, size, AnyElement, ClipboardItem, Context, Div, InteractiveElement, IntoElement,
    ParentElement, StatefulInteractiveElement, Styled,
};
use gpui_component::{h_flex, theme::ActiveTheme, v_flex, v_virtual_list};
use std::{path::Path, rc::Rc};

use crate::app::{AppRoot, LogEntry};
use crate::window::ui::{self, ButtonStyle};

const ROW_HEIGHT: f32 = 30.0;

fn row_preview(message: &str) -> String {
    match message.find(['\r', '\n', '\u{2028}', '\u{2029}']) {
        Some(end) => format!("{}…", &message[..end]),
        None => message.to_owned(),
    }
}

fn display_message(original: &str) -> String {
    original
        .replace("\r\n", "\n")
        .replace(['\r', '\u{2028}', '\u{2029}'], "\n")
}

struct LogPathText {
    display: String,
    copy: Option<String>,
}

impl LogPathText {
    fn new(path: &Path) -> Self {
        let copy = path.to_str().map(str::to_owned);
        Self {
            display: path.to_string_lossy().into_owned(),
            copy,
        }
    }

    fn field(&self, cx: &Context<AppRoot>) -> Div {
        let label = if self.copy.is_some() {
            "현재 로그 파일 경로"
        } else {
            "현재 로그 파일 경로 (일부 문자를 대체해 표시했습니다. 원래 경로는 텍스트로 복사할 수 없습니다.)"
        };
        v_flex()
            .gap_1()
            .child(div().text_color(cx.theme().muted_foreground).child(label))
            .child(
                div()
                    .debug_selector(|| "log-detail-path".into())
                    .child(self.display.clone()),
            )
    }
}

fn detail(entry: &LogEntry, cx: &Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();
    let path = LogPathText::new(&crate::logging::current_log_file());
    let path_field = path.field(cx);
    let button = ButtonStyle::neutral(cx);
    v_flex()
        .size_full()
        .min_w_0()
        .min_h_0()
        .gap_2()
        .child(
            h_flex()
                .gap_2()
                .child("로그 상세")
                .child(ui::log_level_label(&entry.level, cx)),
        )
        .child(
            h_flex()
                .gap_2()
                .flex_shrink_0()
                .child(
                    ui::action_button(
                        "log-copy-message",
                        "원문 복사",
                        ui::Size::Md,
                        button,
                        cx.listener(|this, _, _, cx| {
                            if let Some(entry) = &this.app_state.selected_log_entry {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    entry.message.clone(),
                                ));
                            }
                        }),
                    )
                    .debug_selector(|| "log-copy-message".into()),
                )
                .children(path.copy.map(|copy_path| {
                    ui::action_button(
                        "log-copy-path",
                        "경로 복사",
                        ui::Size::Md,
                        button,
                        move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(copy_path.clone()))
                        },
                    )
                    .debug_selector(|| "log-copy-path".into())
                }))
                .child(
                    ui::action_button(
                        "log-close-detail",
                        "닫기",
                        ui::Size::Md,
                        button,
                        cx.listener(|this, _, _, cx| {
                            this.app_state.selected_log_entry = None;
                            cx.notify();
                        }),
                    )
                    .debug_selector(|| "log-close-detail".into()),
                ),
        )
        .child(
            div()
                .id("log-detail-body")
                .debug_selector(|| "log-detail-body".into())
                .w_full()
                .flex_1()
                .min_h_0()
                .min_w_0()
                .overflow_x_hidden()
                .overflow_y_scroll()
                .bg(theme.secondary)
                .rounded_lg()
                .child(
                    v_flex()
                        .w_full()
                        .min_w_0()
                        .p_3()
                        .gap_2()
                        .child(path_field)
                        .child(
                            div()
                                .text_color(theme.muted_foreground)
                                .child("메시지 원문"),
                        )
                        .child(
                            div()
                                .debug_selector(|| "log-detail-message".into())
                                .child(display_message(&entry.message)),
                        ),
                ),
        )
        .into_any_element()
}

pub fn render(root: &AppRoot, cx: &mut Context<AppRoot>) -> AnyElement {
    if let Some(entry) = &root.app_state.selected_log_entry {
        return detail(entry, cx);
    }
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
                    .child(
                        v_flex()
                            .debug_selector(|| "log-list-empty".into())
                            .gap_2()
                            .child("기록된 로그가 없습니다.")
                            .child(LogPathText::new(&crate::logging::current_log_file()).field(cx)),
                    ),
            )
            .into_any_element();
    }

    let item_sizes = Rc::new(vec![
        size(px(0.), px(ROW_HEIGHT));
        root.app_state.log_entries.len()
    ]);

    let scroll = root.log_scroll_handle.clone();
    let (file_count, total_bytes) = crate::logging::log_dir_stats();

    v_flex()
        .size_full()
        .gap_3()
        .child(
            h_flex()
                .w_full()
                .flex_wrap()
                .gap_2()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .debug_selector(|| "log-list-heading".into())
                        .flex_shrink_0()
                        .text_color(theme.foreground)
                        .child("로그 · 행을 선택하면 원문 보기"),
                )
                .child(
                    div()
                        .debug_selector(|| "log-list-stats".into())
                        .whitespace_nowrap()
                        .text_color(theme.muted_foreground)
                        .child(format!(
                            "파일 {file_count}개 · {:.1} MB",
                            total_bytes as f64 / (1024.0 * 1024.0)
                        )),
                ),
        )
        .child(
            div()
                .debug_selector(|| "log-list-card".into())
                .rounded_lg()
                .p_2()
                .bg(theme.secondary)
                .border_1()
                .border_color(theme.border)
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .child(
                    v_virtual_list(
                        cx.entity(),
                        "event-log-virtual-list",
                        item_sizes,
                        move |this, visible_range, _window, cx| {
                            visible_range
                                .filter_map(|ix| {
                                    let entry = this.app_state.log_entries.get(ix)?.clone();
                                    let row = h_flex()
                                        .id(("log-row", ix))
                                        .debug_selector(move || format!("log-row-{ix}"))
                                        .w_full()
                                        .h(px(ROW_HEIGHT))
                                        .gap_2()
                                        .px_2()
                                        .items_center()
                                        .overflow_hidden()
                                        .cursor_pointer()
                                        .border_b_1()
                                        .border_color(cx.theme().border)
                                        .hover(|s| s.bg(cx.theme().secondary_hover))
                                        .child(ui::log_level_label(&entry.level, cx))
                                        .child(
                                            div()
                                                .debug_selector(move || format!("log-preview-{ix}"))
                                                .flex_1()
                                                .min_w_0()
                                                .truncate()
                                                .child(row_preview(&entry.message)),
                                        )
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.app_state.selected_log_entry = Some(entry.clone());
                                            cx.notify();
                                        }));
                                    Some(row)
                                })
                                .collect::<Vec<_>>()
                        },
                    )
                    .track_scroll(&scroll),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod log_detail_tests {
    use super::*;

    #[test]
    fn log_detail_normalizes_display_breaks_without_losing_blank_lines_or_tail() {
        let original = "a\r\n\r\nb\rc\u{2028}d\u{2029}끝";
        assert_eq!(display_message(original), "a\n\nb\nc\nd\n끝");
        for sep in ["\r\n", "\r", "\n", "\u{2028}", "\u{2029}"] {
            assert_eq!(row_preview(&format!("head{sep}tail")), "head…");
        }
    }

    #[cfg(any(windows, unix))]
    #[test]
    fn log_detail_unrepresentable_native_path_is_display_only() {
        #[cfg(windows)]
        let invalid = {
            use std::os::windows::ffi::OsStringExt;
            std::ffi::OsString::from_wide(&[b'x' as u16, 0xD800])
        };
        #[cfg(unix)]
        let invalid = {
            use std::os::unix::ffi::OsStringExt;
            std::ffi::OsString::from_vec(vec![b'x', 0xFF])
        };
        let path = LogPathText::new(Path::new(&invalid));
        assert_eq!(path.display, "x\u{FFFD}");
        assert!(path.copy.is_none());
        // A literal replacement character is valid and must remain copyable.
        let literal = LogPathText::new(Path::new("x\u{FFFD}"));
        assert_eq!(literal.copy.as_deref(), Some("x\u{FFFD}"));
    }
}
