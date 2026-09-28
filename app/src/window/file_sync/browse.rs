//! 파일·폴더를 직접 보면서 제외 목록에 넣는 원본 탐색기.

use super::*;

const PAGE_SIZE: usize = 100;

pub(super) fn render(this: &AppRoot, cx: &mut Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();
    let fg = theme.foreground;
    let muted = theme.muted_foreground;
    let neutral = ButtonStyle::neutral(cx);
    let current = if this.sync.browse_relative.as_os_str().is_empty() {
        "/".to_string()
    } else {
        format!("/{}", this.sync.browse_relative.display())
    };
    let total = this.sync.browse_entries.len();
    let page = this
        .sync
        .browse_page
        .min(total.saturating_sub(1) / PAGE_SIZE);
    let selected_patterns = this
        .sync
        .exclude_input
        .as_ref()
        .map(|input| input.read(cx).value().to_string())
        .unwrap_or_default();
    let selected: Vec<_> = selected_patterns.lines().map(str::trim).collect();

    let mut rows = v_flex().w_full().gap_1();
    for (index, entry) in this
        .sync
        .browse_entries
        .iter()
        .enumerate()
        .skip(page * PAGE_SIZE)
        .take(PAGE_SIZE)
    {
        let path = entry.relative_path.clone();
        let name = entry.name.clone();
        let can_enter = entry.is_directory && !entry.is_link;
        let excluded = selected.contains(&path.as_str());
        let kind = if entry.is_link {
            "링크·정션"
        } else if entry.is_directory {
            "폴더"
        } else {
            "파일"
        };
        let detail = if entry.is_hidden_or_system {
            format!("{kind} · 숨김/시스템")
        } else {
            kind.to_string()
        };
        rows = rows.child(
            h_flex()
                .debug_selector(move || format!("sync-browse-row-{index}"))
                .w_full()
                .min_w_0()
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
                        .child(div().text_color(fg).overflow_hidden().child(name.clone()))
                        .child(div().text_color(muted).child(detail)),
                )
                .children(can_enter.then(|| {
                    div()
                        .debug_selector(move || format!("sync-browse-enter-{index}"))
                        .child(ui::action_button(
                            ("sync-browse-enter", index),
                            "열기",
                            ui::Size::Sm,
                            neutral,
                            cx.listener(move |this, _ev, _window, cx| {
                                this.enter_sync_browse_dir(&name, cx)
                            }),
                        ))
                }))
                .child(
                    div()
                        .debug_selector(move || format!("sync-browse-toggle-{index}"))
                        .child(ui::action_button(
                            ("sync-browse-toggle", index),
                            if excluded { "제외 해제" } else { "제외" },
                            ui::Size::Sm,
                            neutral,
                            cx.listener(move |this, _ev, window, cx| {
                                this.toggle_sync_exclusion(&path, window, cx)
                            }),
                        )),
                ),
        );
    }
    if let Some(error) = this.sync.browse_error.as_ref() {
        rows = rows.child(div().text_color(theme.warning).child(error.clone()));
    } else if total == 0 {
        rows = rows.child(div().text_color(muted).child("이 폴더는 비어 있습니다."));
    }

    v_flex()
        .min_w_0()
        .gap_2()
        .debug_selector(|| "sync-browse-card".to_string())
        .child(div().text_color(fg).child("원본 파일·폴더에서 제외 선택"))
        .child(div().text_color(muted).child(
            "숨김·시스템 항목도 모두 표시합니다. 폴더를 제외하면 하위 항목도 동기화하지 않습니다.",
        ))
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .gap_2()
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_color(fg)
                        .overflow_hidden()
                        .child(current),
                )
                .child(ui::action_button(
                    "sync-browse-parent",
                    "상위",
                    ui::Size::Sm,
                    neutral,
                    cx.listener(|this, _ev, _window, cx| this.parent_sync_browse_dir(cx)),
                ))
                .child(ui::action_button(
                    "sync-browse-refresh",
                    "새로고침",
                    ui::Size::Sm,
                    neutral,
                    cx.listener(|this, _ev, _window, cx| this.refresh_sync_browse(cx)),
                )),
        )
        .child(
            div()
                .h(px(232.0))
                .min_h_0()
                .w_full()
                .min_w_0()
                .child(scroll_pane(
                    "sync-browse-scroll",
                    &this.sync.browse_scroll,
                    rows.into_any_element(),
                )),
        )
        .children((total > PAGE_SIZE).then(|| {
            h_flex()
                .gap_2()
                .items_center()
                .child(ui::action_button(
                    "sync-browse-prev",
                    "이전 100개",
                    ui::Size::Sm,
                    neutral,
                    cx.listener(|this, _ev, _window, cx| this.change_sync_browse_page(-1, cx)),
                ))
                .child(div().text_color(muted).child(format!(
                    "{}/{} · 전체 {}개",
                    page + 1,
                    total.div_ceil(PAGE_SIZE),
                    total
                )))
                .child(ui::action_button(
                    "sync-browse-next",
                    "다음 100개",
                    ui::Size::Sm,
                    neutral,
                    cx.listener(|this, _ev, _window, cx| this.change_sync_browse_page(1, cx)),
                ))
        }))
        .into_any_element()
}
