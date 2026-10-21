//! 작업 설정 → 입력 → 변환 → 결과의 단일 흐름. 독립 설정 pane으로 쪼개지 않는다.
use super::ui::{self, ButtonStyle, Size};
use crate::app::AppRoot;
use gpui::{
    div, px, AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Styled, Window,
};
use gpui_component::{h_flex, input::Input, theme::ActiveTheme, v_flex};

pub fn render(this: &mut AppRoot, window: &mut Window, cx: &mut Context<AppRoot>) -> AnyElement {
    this.ensure_openssl_inputs(window, cx);
    let state = &this.openssl;
    let Some(passphrase) = state.passphrase.as_ref() else {
        return div().into_any_element();
    };
    let Some(input) = state.input.as_ref() else {
        return div().into_any_element();
    };
    let Some(output) = state.output.as_ref() else {
        return div().into_any_element();
    };
    let theme = cx.theme();
    let foreground = theme.foreground;
    let muted = theme.muted_foreground;
    let status_color = if state.failed {
        theme.danger
    } else {
        theme.foreground
    };
    let save_color = if state.save_error {
        theme.danger
    } else {
        muted
    };
    let content = v_flex().w_full().min_w_0().pr_4().gap_3().text_color(foreground)
        .child(div().text_lg().child("OpenSSL 암복호화 · PCF 호환"))
        .child(div().text_sm().text_color(muted)
            .child("AES-256-CBC · Salted__ / Base64 · 인코딩 SHA-256 / 디코딩 SHA-256 → MD5"))
        .child(v_flex().w_full().min_w_0().gap_2()
            .child(div().child("암호 문구 (-k)"))
            .child(div().debug_selector(|| "openssl-passphrase".into()).w_full().min_w_0()
                .child(Input::new(passphrase).mask_toggle()))
            .child(div().text_sm().text_color(save_color).child(if state.save_error {
                "설정 저장 실패 · 경로/쓰기 권한을 확인하고 설정의 저장 버튼으로 재시도하세요."
            } else { "암호 문구와 옵션은 자동 저장됩니다. 암호 문구는 settings.json에 평문 저장됩니다." }))
            .child(h_flex().w_full().min_w_0().gap_2()
                .child(div().debug_selector(|| "openssl-newline".into()).flex_shrink_0().child(
                    ui::toggle_switch("openssl-newline-switch", state.config.append_newline, cx)
                        .on_click(cx.listener(|this, enabled: &bool, window, cx| this.set_openssl_newline(*enabled, window, cx)))))
                .child(div().debug_selector(|| "openssl-newline-label".into()).flex_1().min_w_0().text_sm().child("인코딩 시 끝에 LF 추가 (Linux echo / PCF 기본값)"))))
        .child(div().text_sm().text_color(muted)
            .child("기존 PCF 호환용입니다. 인증되지 않은 CBC/레거시 키 유도 방식이므로 새 보안 설계용으로 사용하지 마세요."))
        .child(div().child("입력 · 평문 32 KiB / Base64 암호문 64 KiB (UTF-8)"))
        .child(div().debug_selector(|| "openssl-input".into()).w_full().min_w_0()
            .child(Input::new(input).h(px(124.0))))
        .child(h_flex().w_full().min_w_0().gap_2().flex_wrap()
            .child(div().debug_selector(|| "openssl-encode".into()).child(
                ui::action_button("openssl-encode-button", "인코딩 (암호화)", Size::Md, ButtonStyle::primary(cx),
                    cx.listener(|this, _, window, cx| this.run_openssl(true, window, cx)))))
            .child(div().debug_selector(|| "openssl-decode".into()).child(
                ui::action_button("openssl-decode-button", "디코딩 (복호화)", Size::Md, ButtonStyle::primary(cx),
                    cx.listener(|this, _, window, cx| this.run_openssl(false, window, cx)))))
            .child(div().debug_selector(|| "openssl-clear".into()).child(
                ui::action_button("openssl-clear-button", "작업 텍스트 지우기", Size::Md, ButtonStyle::secondary(cx),
                    cx.listener(|this, _, window, cx| this.clear_openssl_text(window, cx))))))
        .child(div().debug_selector(|| "openssl-status".into()).min_w_0().text_sm().text_color(status_color)
            .child(if state.status.is_empty() { "입력과 결과는 저장·로그 기록하지 않습니다.".to_string() } else { state.status.clone() }))
        .child(div().child("결과 · 선택 후 Ctrl+C로 복사 (편집 가능)"))
        .child(div().debug_selector(|| "openssl-output".into()).w_full().min_w_0()
            .child(Input::new(output).h(px(124.0))))
        .child(div().text_sm().text_color(muted).child("Salt는 매번 무작위라 같은 입력도 암호문은 달라집니다. 복호화 결과로 호환성을 확인하세요."));
    super::scroll_pane("openssl-page", &state.scroll, content.into_any_element())
}
