//! 신뢰 등록 → 검색·선택 → 명시 실행의 단일 스크롤 작업 흐름.
use super::ui::{self, ButtonStyle, Size};
use crate::{app::AppRoot, config::launcher::LauncherKind, quick_launcher};
use gpui::{
    div, px, AnyElement, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    SharedString, Styled, Window,
};
use gpui_component::{
    h_flex,
    input::{Input, InputState},
    theme::ActiveTheme,
    v_flex,
};

fn draft_input(
    label: &'static str,
    selector: &'static str,
    input: &Entity<InputState>,
) -> AnyElement {
    v_flex()
        .w_full()
        .min_w_0()
        .gap_1()
        .child(div().text_sm().child(label))
        .child(
            div()
                .debug_selector(move || selector.into())
                .w_full()
                .min_w_0()
                .child(Input::new(input)),
        )
        .into_any_element()
}

pub fn render(this: &mut AppRoot, window: &mut Window, cx: &mut Context<AppRoot>) -> AnyElement {
    this.ensure_launcher_inputs(window, cx);
    let state = &this.launcher;
    let (Some(search), Some(name), Some(path), Some(args)) =
        (&state.search, &state.name, &state.path, &state.args)
    else {
        return div().into_any_element();
    };
    let theme = cx.theme();
    let muted = theme.muted_foreground;
    let foreground = theme.foreground;
    let danger = theme.danger;
    let indices = quick_launcher::matching_indices(&state.config.entries, &state.query);
    let mut entries = v_flex().w_full().min_w_0().gap_2();
    if indices.is_empty() {
        entries = entries.child(
            div()
                .debug_selector(|| "launcher-empty".into())
                .text_sm()
                .text_color(muted)
                .child("등록 항목이 없거나 검색 결과가 없습니다."),
        );
    }
    for index in indices {
        let entry = &state.config.entries[index];
        let selected = state.selected == Some(index);
        let button_id = SharedString::from(format!("launcher-select-{index}"));
        let edit_id = SharedString::from(format!("launcher-edit-{index}"));
        let delete_id = SharedString::from(format!("launcher-delete-{index}"));
        entries = entries.child(
            v_flex()
                .debug_selector(move || format!("launcher-entry-{index}"))
                .w_full()
                .min_w_0()
                .flex_shrink_0()
                .gap_1()
                .p_2()
                .border_1()
                .rounded_md()
                .border_color(if selected {
                    theme.primary
                } else {
                    theme.border
                })
                .child(
                    h_flex()
                        .w_full()
                        .min_w_0()
                        .flex_shrink_0()
                        .gap_2()
                        .flex_wrap()
                        .child(
                            div()
                                .debug_selector(move || format!("launcher-select-{index}"))
                                .flex_1()
                                .min_w(px(180.0))
                                .child(
                                    ui::action_button(
                                        button_id,
                                        entry.name.clone(),
                                        Size::Sm,
                                        if selected {
                                            ButtonStyle::primary(cx)
                                        } else {
                                            ButtonStyle::neutral(cx)
                                        },
                                        cx.listener(move |this, _, _, cx| {
                                            this.launcher.selected = Some(index);
                                            this.launcher.pending_script = None;
                                            cx.notify();
                                        }),
                                    )
                                    .w_full(),
                                ),
                        )
                        .child(
                            ui::badge(entry.kind.label(), ui::Tone::Muted, Size::Sm, cx)
                                .flex_shrink_0(),
                        )
                        .child(
                            div()
                                .debug_selector(move || format!("launcher-edit-{index}"))
                                .flex_shrink_0()
                                .child(ui::action_button(
                                    edit_id,
                                    "편집",
                                    Size::Sm,
                                    ButtonStyle::secondary(cx),
                                    cx.listener(move |this, _, window, cx| {
                                        this.edit_launcher_entry(index, window, cx)
                                    }),
                                )),
                        )
                        .child(
                            div()
                                .debug_selector(move || format!("launcher-delete-{index}"))
                                .flex_shrink_0()
                                .child(ui::action_button(
                                    delete_id,
                                    "등록 삭제",
                                    Size::Sm,
                                    ButtonStyle::danger_outline(cx),
                                    cx.listener(move |this, _, window, cx| {
                                        this.delete_launcher_entry(index, window, cx)
                                    }),
                                )),
                        ),
                )
                .child(
                    div()
                        .debug_selector(move || format!("launcher-entry-path-{index}"))
                        .w_full()
                        .min_w_0()
                        .flex_shrink_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_sm()
                        .text_color(muted)
                        .child(entry.path.clone()),
                ),
        );
    }
    let mut kinds = h_flex().w_full().gap_2().flex_wrap();
    for (kind, id) in [
        (LauncherKind::Application, "launcher-kind-app"),
        (LauncherKind::Folder, "launcher-kind-folder"),
        (LauncherKind::Script, "launcher-kind-script"),
    ] {
        kinds = kinds.child(
            div()
                .debug_selector(move || id.into())
                .child(ui::action_button(
                    id,
                    kind.label(),
                    Size::Sm,
                    if state.draft_kind == kind {
                        ButtonStyle::primary(cx)
                    } else {
                        ButtonStyle::secondary(cx)
                    },
                    cx.listener(move |this, _, _, cx| this.set_launcher_kind(kind, cx)),
                )),
        );
    }
    let mut content = v_flex().w_full().min_w_0().pr_4().gap_3().text_color(foreground)
        .child(div().text_lg().child("빠른 실행기"))
        .child(div().text_sm().text_color(muted).child("직접 확인한 로컬 절대 경로만 등록하세요. 설정에는 경로와 인수가 평문 저장됩니다. 등록 파일이 나중에 변경될 수 있으므로 내용을 직접 관리하세요."))
        .child(h_flex().w_full().min_w_0().gap_2().flex_wrap()
            .child(div().debug_selector(|| "launcher-hotkey-toggle".into()).child(ui::toggle_switch("launcher-hotkey-switch", state.config.enabled, cx)
                .on_click(cx.listener(|this, enabled: &bool, _, cx| this.set_launcher_hotkey_enabled(*enabled, cx)))))
            .child(div().text_sm().child("전역 단축키 (기본 꺼짐)"))
            .child(div().debug_selector(|| "launcher-hotkey-combo".into()).child(ui::action_button("launcher-hotkey-combo-button", state.config.hotkey.label(), Size::Sm, ButtonStyle::secondary(cx), cx.listener(|this, _, _, cx| this.cycle_launcher_hotkey(cx))))))
        .child(div().debug_selector(|| "launcher-hotkey-status".into()).text_sm().text_color(if state.hotkey_failed { danger } else { muted }).child(if state.hotkey_status.is_empty() { "단축키는 이 패널만 열며 항목을 자동 실행하지 않습니다.".into() } else { state.hotkey_status.clone() }))
        .child(div().debug_selector(|| "launcher-search".into()).w_full().min_w_0().child(Input::new(search)))
        .child(entries)
        .child(div().debug_selector(|| "launcher-run".into()).child(ui::action_button("launcher-run-button", if state.is_busy() { "실행 요청 처리 중…" } else { "선택한 항목 실행" }, Size::Md, ButtonStyle::primary(cx), cx.listener(|this, _, _, cx| this.request_launcher_run(cx)))))
        .child(div().debug_selector(|| "launcher-status".into()).text_sm().text_color(if state.failed { danger } else { muted }).child(state.status.clone()));
    if let Some(entry) = &state.pending_script {
        content = content.child(
            v_flex()
                .debug_selector(|| "launcher-confirmation".into())
                .w_full()
                .min_w_0()
                .p_3()
                .gap_2()
                .border_1()
                .border_color(danger)
                .rounded_md()
                .child(div().child("스크립트 실행 확인 · 신뢰한 내용만 실행하세요"))
                .child(div().text_sm().w_full().min_w_0().child(entry.path.clone()))
                .child(
                    div()
                        .text_sm()
                        .w_full()
                        .min_w_0()
                        .child(serde_json::to_string(&entry.args).unwrap_or_default()),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .flex_wrap()
                        .child(div().debug_selector(|| "launcher-confirm".into()).child(
                            ui::action_button(
                                "launcher-confirm-button",
                                "확인 후 실행",
                                Size::Md,
                                ButtonStyle::danger(cx),
                                cx.listener(|this, _, _, cx| {
                                    this.confirm_launcher_script(true, cx)
                                }),
                            ),
                        ))
                        .child(div().debug_selector(|| "launcher-cancel".into()).child(
                            ui::action_button(
                                "launcher-cancel-button",
                                "취소",
                                Size::Md,
                                ButtonStyle::secondary(cx),
                                cx.listener(|this, _, _, cx| {
                                    this.confirm_launcher_script(false, cx)
                                }),
                            ),
                        )),
                ),
        );
    }
    content = content.child(h_flex().w_full().gap_2().flex_wrap()
            .child(div().text_lg().child(if state.editing.is_some() { "등록 항목 편집" } else { "새 항목 등록" }))
            .child(div().debug_selector(|| "launcher-new".into()).child(ui::action_button("launcher-new-button", "새 항목", Size::Sm, ButtonStyle::secondary(cx), cx.listener(|this, _, window, cx| this.new_launcher_entry(window, cx))))))
        .child(kinds)
        .child(draft_input("표시 이름", "launcher-name", name))
        .child(draft_input("실행 대상 절대 경로", "launcher-path", path))
        .child(h_flex().gap_2().flex_wrap()
            .child(div().debug_selector(|| "launcher-browse-file".into()).child(ui::action_button("launcher-browse-file-button", "파일 찾기", Size::Sm, ButtonStyle::secondary(cx), cx.listener(|this, _, window, cx| this.browse_launcher_path(false, window, cx)))))
            .child(div().debug_selector(|| "launcher-browse-folder".into()).child(ui::action_button("launcher-browse-folder-button", "폴더 선택", Size::Sm, ButtonStyle::secondary(cx), cx.listener(|this, _, window, cx| this.browse_launcher_path(true, window, cx))))))
        .child(draft_input("인수 · 문자열 JSON 배열 (폴더는 [])", "launcher-args", args))
        .child(div().text_sm().text_color(muted).child(r#"예: ["--name", "value with spaces"] · 셸 명령 문자열과 .bat/.cmd/.lnk는 지원하지 않습니다. .ps1은 시스템 PowerShell -File로만 실행하며 실행 정책을 우회하지 않습니다."#))
        .child(h_flex().w_full().min_w_0().gap_2()
            .child(div().debug_selector(|| "launcher-trust".into()).child(ui::toggle_switch("launcher-trust-switch", state.draft_trusted, cx).on_click(cx.listener(|this, enabled: &bool, _, cx| { this.launcher.draft_trusted = *enabled; cx.notify(); }))))
            .child(div().min_w_0().flex_1().text_sm().child("이 경로와 인수, 파일 내용을 직접 확인했으며 신뢰합니다.")))
        .child(div().debug_selector(|| "launcher-save".into()).child(ui::action_button("launcher-save-button", "신뢰 대상 저장", Size::Md, ButtonStyle::primary(cx), cx.listener(|this, _, _, cx| this.register_launcher_entry(cx)))))
        .child(div().debug_selector(|| "launcher-edit-status".into()).text_sm().text_color(if state.failed { danger } else { muted }).child(state.status.clone()))
        .child(div().debug_selector(|| "launcher-save-status".into()).text_sm().text_color(if state.save_error { danger } else { muted }).child(if state.save_error { "설정 저장 실패 · 쓰기 권한과 설정 파일을 확인하고 전체 저장으로 재시도하세요." } else { "등록 항목과 단축키 설정만 저장합니다. 검색어·초안·실행 결과는 저장하지 않습니다." }));
    super::scroll_pane("launcher-page", &state.scroll, content.into_any_element())
}
