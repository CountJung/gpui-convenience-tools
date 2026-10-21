use super::ui::{self, ButtonStyle, Size};
use crate::app::AppRoot;
use gpui::{
    div, AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Styled, Window,
};
use gpui_component::{h_flex, input::Input, theme::ActiveTheme, v_flex};

pub fn render(this: &mut AppRoot, window: &mut Window, cx: &mut Context<AppRoot>) -> AnyElement {
    this.ensure_timestamp_inputs(window, cx);
    let state = &this.timestamp;
    let color = if state.failed || state.save_error {
        cx.theme().danger
    } else {
        cx.theme().muted_foreground
    };
    let content = v_flex().w_full().min_w_0().pr_4().gap_3().text_color(cx.theme().foreground)
        .child(div().text_lg().child("리눅스 타임스탬프 변환"))
        .child(div().text_sm().child("Unix 초 ↔ 날짜 · 최대 소수 9자리 · 입력과 결과는 저장하지 않습니다."))
        .child(h_flex().gap_2().flex_wrap()
            .child(div().debug_selector(|| "timestamp-zone".into()).flex_shrink_0().child(
                ui::toggle_switch("timestamp-utc", state.config.utc, cx)
                    .on_click(cx.listener(|this, value: &bool,_,cx| this.set_timestamp_utc(*value,cx)))))
            .child(div().child(if state.config.utc { "UTC (+00:00)" } else { "한국 시간 KST (+09:00)" })))
        .child(div().child("Unix 타임스탬프 (초 단위, 밀리초 정수가 아님)"))
        .child(div().debug_selector(|| "timestamp-epoch".into()).w_full().min_w_0().child(Input::new(state.epoch.as_ref().expect("입력 초기화"))))
        .child(h_flex().gap_2().flex_wrap()
            .child(div().debug_selector(|| "timestamp-to-date".into()).child(ui::action_button("timestamp-to-date-btn", "날짜로 변환 ↓", Size::Md, ButtonStyle::primary(cx), cx.listener(|this,_,w,cx|this.convert_timestamp(true,w,cx)))))
            .child(div().debug_selector(|| "timestamp-now".into()).child(ui::action_button("timestamp-now-btn", "현재 시간", Size::Md, ButtonStyle::secondary(cx), cx.listener(|this,_,w,cx|this.timestamp_now(w,cx)))))
            .child(ui::action_button("timestamp-copy-epoch", "초 복사", Size::Md, ButtonStyle::secondary(cx), cx.listener(|this,_,_,cx|this.copy_timestamp(true,cx)))))
        .child(div().child("날짜 (한국어 오전/오후, YYYY-MM-DD HH:MM:SS, ISO 8601)"))
        .child(div().debug_selector(|| "timestamp-date".into()).w_full().min_w_0().child(Input::new(state.date.as_ref().expect("입력 초기화"))))
        .child(h_flex().gap_2().flex_wrap()
            .child(div().debug_selector(|| "timestamp-to-epoch".into()).child(ui::action_button("timestamp-to-epoch-btn", "Unix 초로 변환 ↑", Size::Md, ButtonStyle::primary(cx), cx.listener(|this,_,w,cx|this.convert_timestamp(false,w,cx)))))
            .child(ui::action_button("timestamp-copy-date", "날짜 복사", Size::Md, ButtonStyle::secondary(cx), cx.listener(|this,_,_,cx|this.copy_timestamp(false,cx)))))
        .child(div().debug_selector(|| "timestamp-status".into()).text_sm().text_color(color).child(if state.save_error { "시간대 설정 저장 실패 · 설정의 저장 버튼으로 재시도하세요.".into() } else { state.status.clone() }))
        .child(div().text_sm().text_color(cx.theme().muted_foreground).child("시간대는 자동 저장됩니다. ISO 8601에 명시된 offset은 선택 시간대보다 우선합니다. 초 이하가 없으면 정수 초로 변환됩니다."));
    super::scroll_pane("timestamp-page", &state.scroll, content.into_any_element())
}
