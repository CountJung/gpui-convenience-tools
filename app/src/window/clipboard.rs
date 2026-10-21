//! 수집 동의·정책·검색·선택·최근 항목을 한 스크롤 흐름으로 제공한다.
use super::ui::{self, ButtonStyle, Size};
use crate::app::AppRoot;
use gpui::{
    div, img, px, AnyElement, Context, InteractiveElement, IntoElement, ObjectFit, ParentElement,
    Styled, StyledImage, Window,
};
use gpui_component::{h_flex, input::Input, theme::ActiveTheme, v_flex};

pub fn render(this: &mut AppRoot, window: &mut Window, cx: &mut Context<AppRoot>) -> AnyElement {
    this.ensure_clipboard_inputs(window, cx);
    let state = &this.clipboard;
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let foreground = theme.foreground;
    let border = theme.border;
    let selected_background = theme.secondary;
    let status_color = if state.failed { theme.danger } else { muted };
    let save_color = if state.save_error {
        theme.danger
    } else {
        muted
    };
    let mut content = v_flex().w_full().min_w_0().pr_4().gap_3().text_color(foreground)
        .child(div().text_lg().child("클립보드 히스토리"))
        .child(div().text_sm().text_color(muted).child(
            "기본 OFF · 이 기기에 평문/PNG로만 보관 · 켜면 패널을 닫아도 앱 실행 중 수집합니다. 시스템 클립보드·Windows 기록은 초기화하지 않습니다."))
        .child(h_flex().w_full().min_w_0().gap_4().flex_wrap()
            .child(h_flex().gap_2()
                .child(div().debug_selector(|| "clipboard-text-switch".into()).child(
                    ui::toggle_switch("clipboard-text-toggle", state.config.collect_text, cx)
                        .on_click(cx.listener(|this, value: &bool, _, cx| this.set_clipboard_collection(false, *value, cx)))))
                .child(div().child("텍스트 수집")))
            .child(h_flex().gap_2()
                .child(div().debug_selector(|| "clipboard-image-switch".into()).child(
                    ui::toggle_switch("clipboard-image-toggle", state.config.collect_images, cx)
                        .on_click(cx.listener(|this, value: &bool, _, cx| this.set_clipboard_collection(true, *value, cx)))))
                .child(div().child("PNG 이미지 수집"))))
        .child(div().debug_selector(|| "clipboard-collection-state".into()).text_sm().child(
            if state.config.enabled() { "수집 허용됨 · 750ms마다 변경 감지 · 재시작 시 설정 복원" } else { "수집 중지 · 현재 클립보드를 읽지 않습니다." }))
        .child(div().text_sm().text_color(save_color).child(if state.save_error {
            "설정 저장 실패 · 다음 실행에 반영되지 않을 수 있습니다. 쓰기 권한을 확인하고 설정의 저장 버튼으로 재시도하세요."
        } else { "수집/제외 설정은 자동 저장됩니다. 검색어와 선택 상태는 저장하지 않습니다." }))
        .child(div().debug_selector(|| "clipboard-status".into()).text_sm().text_color(status_color).child(
            if state.busy { "이미지·이력 처리 중 · 추가 수집은 대기하며 작업을 쌓지 않습니다.".to_string() }
            else if state.status.is_empty() { "명시적으로 켜기 전에는 수집하지 않습니다.".to_string() } else { state.status.clone() }));
    if let Some(search) = &state.search {
        content = content.child(
            div()
                .debug_selector(|| "clipboard-search".into())
                .w_full()
                .min_w_0()
                .child(Input::new(search)),
        );
    }
    content = content.child(
        h_flex()
            .w_full()
            .min_w_0()
            .gap_3()
            .flex_wrap()
            .child(
                div()
                    .debug_selector(|| "clipboard-count".into())
                    .text_sm()
                    .child(format!(
                        "검색 {} / 전체 {}개 · {} KiB",
                        state.visible_entries().count(),
                        state.history.entries.len(),
                        state.history.total_bytes().div_ceil(1024)
                    ))
                    .child(div().debug_selector({
                        let selector =
                            format!("clipboard-result-count-{}", state.visible_entries().count());
                        move || selector.clone()
                    })),
            )
            .child(
                div()
                    .debug_selector(|| "clipboard-clear".into())
                    .child(ui::action_button(
                        "clipboard-clear-button",
                        if state.confirm_clear {
                            "고정 포함 전체 삭제 확인"
                        } else {
                            "로컬 이력 초기화"
                        },
                        Size::Md,
                        ButtonStyle::danger_outline(cx),
                        cx.listener(|this, _, _, cx| this.clear_clipboard_history(cx)),
                    )),
            ),
    );
    if let Some(selected) = state
        .selected
        .and_then(|id| state.history.entries.iter().find(|entry| entry.id == id))
    {
        let id = selected.id;
        content = content.child(
            v_flex()
                .w_full()
                .min_w_0()
                .gap_2()
                .p_3()
                .border_1()
                .border_color(border)
                .rounded_md()
                .child(
                    div()
                        .text_sm()
                        .child(format!("선택한 항목 · {}바이트", selected.bytes())),
                )
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .overflow_hidden()
                        .child(selected.preview()),
                )
                .child(
                    div()
                        .debug_selector(|| "clipboard-copy-selected".into())
                        .child(ui::action_button(
                            "clipboard-copy-selected-button",
                            "선택 항목 다시 복사",
                            Size::Md,
                            ButtonStyle::primary(cx),
                            cx.listener(move |this, _, _, cx| this.copy_clipboard_entry(id, cx)),
                        )),
                ),
        );
    }
    if let Some(image) = &state.selected_image {
        content = content.child(
            div()
                .debug_selector(|| "clipboard-image-preview".into())
                .w_full()
                .min_w_0()
                .child(
                    img(image.clone())
                        .w_full()
                        .h(px(160.0))
                        .object_fit(ObjectFit::Contain),
                ),
        );
    }
    let mut entries: Vec<_> = state.visible_entries().collect();
    entries.sort_by_key(|entry| !entry.pinned);
    if entries.is_empty() {
        content = content.child(
            div()
                .debug_selector(|| "clipboard-empty".into())
                .text_sm()
                .text_color(muted)
                .child("보관 항목이 없거나 검색 결과가 없습니다."),
        );
    }
    for entry in entries {
        let id = entry.id;
        let row_selector = format!("clipboard-entry-{id}");
        let select_selector = format!("clipboard-select-{id}");
        let pin_selector = format!("clipboard-pin-{id}");
        let delete_selector = format!("clipboard-delete-{id}");
        let selected = state.selected == Some(id);
        content = content.child(
            v_flex()
                .debug_selector(move || row_selector.clone())
                .w_full()
                .min_w_0()
                .gap_2()
                .p_3()
                .border_1()
                .border_color(border)
                .rounded_md()
                .bg(if selected {
                    selected_background
                } else {
                    theme.background
                })
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .overflow_hidden()
                        .text_sm()
                        .child(entry.preview()),
                )
                .child(
                    h_flex()
                        .w_full()
                        .min_w_0()
                        .gap_2()
                        .flex_wrap()
                        .child(div().text_xs().text_color(muted).child(format!(
                            "{} · {}바이트 · Unix {}",
                            if entry.pinned { "고정" } else { "최근" },
                            entry.bytes(),
                            entry.created_at
                        )))
                        .child(div().debug_selector(move || select_selector.clone()).child(
                            ui::action_button(
                                ("clipboard-select", id),
                                if selected { "선택됨" } else { "선택" },
                                Size::Sm,
                                ButtonStyle::secondary(cx),
                                cx.listener(move |this, _, _, cx| {
                                    this.select_clipboard_entry(id, cx)
                                }),
                            ),
                        ))
                        .child(div().debug_selector(move || pin_selector.clone()).child(
                            ui::action_button(
                                ("clipboard-pin", id),
                                if entry.pinned {
                                    "고정 해제"
                                } else {
                                    "고정"
                                },
                                Size::Sm,
                                ButtonStyle::secondary(cx),
                                cx.listener(move |this, _, _, cx| this.pin_clipboard_entry(id, cx)),
                            ),
                        ))
                        .child(div().debug_selector(move || delete_selector.clone()).child(
                            ui::action_button(
                                ("clipboard-delete", id),
                                "삭제",
                                Size::Sm,
                                ButtonStyle::danger_outline(cx),
                                cx.listener(move |this, _, _, cx| {
                                    this.delete_clipboard_entry(id, cx)
                                }),
                            ),
                        )),
                ),
        );
    }
    content = content.child(div().text_lg().child("보관·제외 설정"))
        .child(h_flex().w_full().min_w_0().gap_2()
            .child(div().debug_selector(|| "clipboard-sensitive-switch".into()).flex_shrink_0().child(
                ui::toggle_switch("clipboard-sensitive-toggle", state.config.exclude_sensitive, cx)
                    .on_click(cx.listener(|this, value: &bool, _, cx| this.set_clipboard_sensitive(*value, cx)))))
            .child(div().flex_1().min_w_0().text_sm().child("비밀 패턴 제외 (개인 키·password=·Bearer·JWT 등)")))
        .child(div().text_sm().text_color(muted).child(
            "Windows 표준 암호 입력창과 제외 마커는 항상 차단합니다. CanIncludeInClipboardHistory 마커는 값과 무관하게 보수적으로 제외합니다. 표시 없는 일반 암호·이미지 속 비밀은 자동 판별할 수 없습니다."))
        .child(div().text_sm().child("보관 한도: 항목 수 (1~500) · 시간 (1~720) · 총 KiB (64~16384), 공백으로 구분"));
    if let Some(limits) = &state.limits {
        content = content.child(
            div()
                .debug_selector(|| "clipboard-limits".into())
                .w_full()
                .min_w_0()
                .child(Input::new(limits)),
        );
    }
    if let Some(exclusions) = &state.exclusions {
        content = content.child(
            div()
                .debug_selector(|| "clipboard-exclusions".into())
                .w_full()
                .min_w_0()
                .child(Input::new(exclusions)),
        );
    }
    content = content
        .child(div().debug_selector(|| "clipboard-policy-apply".into()).child(ui::action_button(
            "clipboard-policy-apply-button", "보관 한도·제외 문구 적용", Size::Md, ButtonStyle::secondary(cx),
            cx.listener(|this, _, _, cx| this.apply_clipboard_policy(cx)))))
        .child(div().text_sm().text_color(muted).child(
            "고정은 일반 항목보다 우선 보관합니다. 기간 만료·한도 축소는 고정에도 적용됩니다. 텍스트 64 KiB / PNG 8 MiB·8192px·16M 픽셀 이하. 제외 문구는 최대 100개, 각 256자입니다."));
    super::scroll_pane("clipboard-page", &state.scroll, content.into_any_element())
}
