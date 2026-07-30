//! 주기 선택 UI — 드롭다운 + 직접 추가 행.
//!
//! 예전에는 고정 프리셋 칩을 나열했지만, 원하는 주기가 목록에 없으면 방법이 없었다.
//! 지금은 드롭다운에서 고르고 (값, 단위)로 새 프리셋을 추가한다.
//! 프리셋 목록은 광고 차단과 파일 동기화가 공유한다([`crate::app::IntervalTarget`]).

use gpui::{
    div, px, AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{h_flex, input::Input, select::Select, theme::ActiveTheme, v_flex};

use crate::app::{AppRoot, IntervalTarget};
use crate::util::format_interval;
use crate::window::ui::{self, ButtonStyle};

/// 주기 드롭다운과 직접 추가 행을 함께 그린다.
pub fn render(
    this: &mut AppRoot,
    target: IntervalTarget,
    window: &mut Window,
    cx: &mut Context<AppRoot>,
) -> AnyElement {
    this.ensure_interval_widgets(target, window, cx);

    let theme = cx.theme();
    let muted_fg = theme.muted_foreground;
    let danger = theme.danger;
    let border = theme.border;

    let neutral_btn = ButtonStyle::neutral(cx).hover(border);
    let current = this.interval_value(target);
    let select = match target {
        IntervalTarget::Scan => this.interval_picker.scan_select.clone(),
        IntervalTarget::Sync => this.interval_picker.sync_select.clone(),
    };
    let amount_input = this.interval_picker.amount_input.clone();
    let unit_select = this.interval_picker.unit_select.clone();
    let error = this.interval_picker.error.clone();

    v_flex()
        .debug_selector(|| "interval-picker".to_string())
        .w_full()
        .gap_2()
        // ── 현재 주기 선택 ──
        .child(
            div()
                .debug_selector(|| "interval-select".to_string())
                .w_full()
                .children(select.as_ref().map(|state| {
                    Select::new(state).placeholder(format_interval(current))
                })),
        )
        // ── 직접 추가 ──
        .child(
            h_flex()
                .w_full()
                .gap_2()
                .items_center()
                .child(div().text_color(muted_fg).child("직접 추가"))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .children(amount_input.as_ref().map(Input::new)),
                )
                .child(
                    div()
                        .debug_selector(|| "interval-unit-select".to_string())
                        .w(px(96.0))
                        .children(unit_select.as_ref().map(Select::new)),
                )
                .child(
                    div()
                        .debug_selector(|| "interval-add".to_string())
                        .child(ui::action_button(
                            "interval-add",
                            "추가",
                            ui::Size::Md,
                            neutral_btn,
                            cx.listener(move |this, _ev, window, cx| {
                                this.add_interval_preset(target, window, cx);
                            }),
                        )),
                ),
        )
        .children(error.map(|reason| {
            div()
                .debug_selector(|| "interval-error".to_string())
                .text_color(danger)
                .child(reason)
        }))
        .child(render_preset_list(this, target, cx))
        .into_any_element()
}

/// 등록된 프리셋과 삭제 버튼.
fn render_preset_list(
    this: &mut AppRoot,
    target: IntervalTarget,
    cx: &mut Context<AppRoot>,
) -> AnyElement {
    // `cx.listener`가 `cx`를 가변으로 빌리므로 테마 색은 미리 복사해 둔다.
    let theme = cx.theme();
    let muted_fg = theme.muted_foreground;
    let border = theme.border;
    let list_bg = theme.list;
    let fg = theme.foreground;
    let danger = theme.danger;

    let presets = this.interval_picker.presets.clone();
    let removable = presets.len() > 1;

    let mut row = h_flex().gap_2().flex_wrap().items_center();
    row = row.child(div().text_color(muted_fg).child("등록된 주기"));

    for secs in presets {
        row = row.child(
            h_flex()
                .debug_selector(move || format!("interval-preset-{secs}"))
                .gap_1()
                .items_center()
                .rounded_md()
                .px_2()
                .py_1()
                .bg(list_bg)
                .border_1()
                .border_color(border)
                .text_color(fg)
                .child(format_interval(secs))
                .children(removable.then(|| {
                    div()
                        .id(("interval-preset-remove", secs as usize))
                        .cursor_pointer()
                        .text_color(muted_fg)
                        .hover(move |s| s.text_color(danger))
                        .on_click(cx.listener(move |this, _ev, window, cx| {
                            this.remove_interval_preset(secs, target, window, cx);
                        }))
                        .child("×")
                })),
        );
    }

    row.into_any_element()
}
