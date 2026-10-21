//! 캡처 → 앱 내부 미리보기 영역 선택 → 저장/복사의 연속 작업형 화면.
use super::ui::{self, ButtonStyle, Size};
use crate::{app::AppRoot, config::CaptureTarget};
use gpui::{
    canvas, div, point, px, AnyElement, Bounds, Context, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement,
    StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{h_flex, input::Input, theme::ActiveTheme, v_flex};

pub fn render(this: &mut AppRoot, window: &mut Window, cx: &mut Context<AppRoot>) -> AnyElement {
    this.ensure_screenshot_inputs(window, cx);
    let state = &this.screenshot;
    let muted = cx.theme().muted_foreground;
    let mut targets = h_flex()
        .id("screenshot-targets")
        .w_full()
        .min_w_0()
        .max_h(px(68.0))
        .overflow_y_scroll()
        .flex_shrink_0()
        .flex_wrap()
        .gap_2()
        .child(
            div()
                .debug_selector(|| "screenshot-target-all".into())
                .child(ui::action_button(
                    "screenshot-all",
                    if state.config.target == CaptureTarget::AllDisplays {
                        "✓ 전체 모니터"
                    } else {
                        "전체 모니터"
                    },
                    Size::Sm,
                    ButtonStyle::secondary(cx),
                    cx.listener(|this, _, _, cx| {
                        this.set_screenshot_target(CaptureTarget::AllDisplays, cx)
                    }),
                )),
        );
    for (index, monitor) in state.monitors.iter().enumerate() {
        let device_id = monitor.device_id.clone();
        let topology = crate::screenshot_tool::platform::topology(&state.monitors);
        let selected = matches!(state.config.target, CaptureTarget::Monitor(_))
            && state
                .config
                .monitor_binding
                .as_ref()
                .is_some_and(|binding| binding.device_id == monitor.device_id);
        let label = format!(
            "{}모니터 {}{} · {} × {}",
            if selected { "✓ " } else { "" },
            index + 1,
            if monitor.primary { " (주)" } else { "" },
            monitor.rect.width,
            monitor.rect.height
        );
        targets = targets.child(ui::action_button(
            ("screenshot-monitor", index),
            label,
            Size::Sm,
            ButtonStyle::secondary(cx),
            cx.listener(move |this, _, _, cx| {
                this.select_screenshot_monitor(&device_id, &topology, cx)
            }),
        ));
    }
    let mut content = v_flex()
        .w_full()
        .min_w_0()
        .overflow_x_hidden()
        .pr_4()
        .gap_2()
        .text_color(cx.theme().foreground)
        .child(div().text_lg().child("스크린샷 · 수동 캡처"))
        .child(
            div()
                .text_sm()
                .text_color(muted)
                .child("Windows 수동 캡처 · 현재 이미지 1장만 메모리 보관 · 이미지 이력 없음"),
        )
        .child(targets)
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .flex_wrap()
                .gap_2()
                .child(div().debug_selector(|| "screenshot-capture".into()).child(
                    ui::action_button(
                        "screenshot-capture-button",
                        if state.busy {
                            "작업 중…"
                        } else {
                            "지금 캡처"
                        },
                        Size::Md,
                        ButtonStyle::primary(cx),
                        cx.listener(|this, _, window, cx| this.capture_screenshot(window, cx)),
                    ),
                ))
                .child(ui::action_button(
                    "screenshot-refresh",
                    "모니터 새로고침",
                    Size::Sm,
                    ButtonStyle::secondary(cx),
                    cx.listener(|this, _, _, cx| this.refresh_screenshot_monitors(cx)),
                ))
                .child(div().debug_selector(|| "screenshot-clear".into()).child(
                    ui::action_button(
                        "screenshot-clear-button",
                        "메모리 이미지 지우기",
                        Size::Sm,
                        ButtonStyle::secondary(cx),
                        cx.listener(|this, _, window, cx| this.clear_screenshot(window, cx)),
                    ),
                )),
        );
    let mut settings = v_flex()
        .w_full()
        .min_w_0()
        .gap_2()
        .child(div().text_sm().text_color(muted).child(
            "저장 설정 · 캡처 버튼은 선택 화면을 즉시 읽으며 잠금/DRM 픽셀은 보장하지 않습니다.",
        ))
        .child(
            h_flex()
                .w_full()
                .min_w_0()
                .gap_2()
                .child(
                    div()
                        .debug_selector(|| "screenshot-autosave".into())
                        .flex_shrink_0()
                        .child(
                            ui::toggle_switch("screenshot-auto-save", state.config.auto_save, cx)
                                .on_click(cx.listener(|this, enabled: &bool, _, cx| {
                                    this.set_screenshot_auto_save(*enabled, cx)
                                })),
                        ),
                )
                .child(
                    div()
                        .min_w_0()
                        .text_sm()
                        .child("수동 캡처 직후 전체 원본 자동 저장 (기본 OFF · 명시적 폴더 필요)"),
                ),
        );
    if let Some(input) = &state.folder_input {
        settings = settings.child(
            h_flex()
                .w_full()
                .min_w_0()
                .gap_2()
                .child(
                    div()
                        .debug_selector(|| "screenshot-folder-input".into())
                        .flex_1()
                        .min_w_0()
                        .child(Input::new(input)),
                )
                .child(ui::action_button(
                    "screenshot-folder",
                    "폴더 선택",
                    Size::Sm,
                    ButtonStyle::secondary(cx),
                    cx.listener(|this, _, window, cx| this.pick_screenshot_folder(window, cx)),
                )),
        );
    }
    if state.save_error {
        content = content.child(
            div()
                .text_sm()
                .text_color(cx.theme().danger)
                .child("설정 저장 실패 · 경로/권한을 확인하고 설정 저장 버튼으로 재시도하세요."),
        );
    }
    let status = if state.status.is_empty() {
        "이미지 이력·선택 영역은 저장하지 않습니다. 캡처 후 미리보기 이미지 안쪽을 드래그하세요."
            .into()
    } else {
        state.status.clone()
    };
    content = content
        .child(div().debug_selector(|| "screenshot-status".into()).w_full().min_w_0().text_sm()
            .text_color(if state.failed { cx.theme().danger } else { cx.theme().foreground }).child(status))
        .child(preview(this, window, cx))
        .child(h_flex().w_full().min_w_0().flex_wrap().gap_2()
            .child(div().debug_selector(|| "screenshot-reset-selection".into()).child(ui::action_button(
                "screenshot-reset-selection-button", "전체 이미지 선택", Size::Sm, ButtonStyle::secondary(cx),
                cx.listener(|this, _, _, cx| this.reset_screenshot_selection(cx)))))
            .child(div().debug_selector(|| "screenshot-crop".into()).child(ui::action_button(
                "screenshot-crop-button", "선택 영역으로 자르기", Size::Sm, ButtonStyle::secondary(cx),
                cx.listener(|this, _, window, cx| this.crop_screenshot(window, cx)))))
            .child(div().debug_selector(|| "screenshot-copy".into()).child(ui::action_button(
                "screenshot-copy-button", "이미지 복사", Size::Md, ButtonStyle::primary(cx),
                cx.listener(|this, _, window, cx| this.copy_screenshot(window, cx)))))
            .child(div().debug_selector(|| "screenshot-save".into()).child(ui::action_button(
                "screenshot-save-button", "PNG 저장…", Size::Md, ButtonStyle::primary(cx),
                cx.listener(|this, _, window, cx| this.save_screenshot_as(window, cx)))))
            .child(div().debug_selector(|| "screenshot-save-default".into()).child(ui::action_button(
                "screenshot-save-default-button", "기본 폴더에 PNG 저장", Size::Sm, ButtonStyle::secondary(cx),
                cx.listener(|this, _, window, cx| this.save_screenshot_default(window, cx))))))
        .child(div().debug_selector(|| "screenshot-footer".into()).text_sm().text_color(muted)
            .child("저장/복사는 선택 영역 우선 · PNG만 저장 · 기존 파일이 있으면 새 이름 사용 · 지우기는 저장된 파일과 클립보드를 변경하지 않음"))
        .child(settings);
    super::scroll_pane(
        "screenshot-page",
        &this.screenshot.scroll,
        content.into_any_element(),
    )
}

fn preview(this: &AppRoot, window: &Window, cx: &mut Context<AppRoot>) -> AnyElement {
    let state = &this.screenshot;
    // 창 높이에 따라 큰 미리보기를 확보하되 작은 창은 자연 높이 컨텐츠 전체를 스크롤한다.
    let available: f32 = window.viewport_size().height.into();
    let height = (available - 390.0).clamp(220.0, 1400.0);
    let recorded_bounds = state.preview_bounds.clone();
    let painted_bounds = state.painted_image_bounds.clone();
    let mut area = div()
        .id("screenshot-preview-area")
        .debug_selector(|| "screenshot-preview".into())
        .relative()
        .w_full()
        .min_w_0()
        .h(px(height))
        .flex_shrink_0()
        .overflow_hidden()
        .bg(cx.theme().secondary)
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, event: &MouseDownEvent, _, cx| {
                this.begin_screenshot_selection(event.position, cx)
            }),
        )
        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
            if event.pressed_button == Some(MouseButton::Left) {
                this.update_screenshot_selection(event.position, false, cx);
            }
        }))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, event: &MouseUpEvent, _, cx| {
                this.update_screenshot_selection(event.position, true, cx)
            }),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            cx.listener(|this, event: &MouseUpEvent, _, cx| {
                this.update_screenshot_selection(event.position, true, cx)
            }),
        );
    if state.preview.is_none() {
        area = area.child(
            div()
                .p_4()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("수동 캡처를 실행하면 여기에 이미지가 표시됩니다."),
        );
    }
    let selection = state.selection;
    let image = state.preview.clone();
    let dimensions = state
        .image
        .as_ref()
        .map(|image| (image.width(), image.height()));
    let accent = cx.theme().primary;
    let fill = cx.theme().primary.opacity(0.12);
    area.child(
        canvas(
            move |bounds, _, _| {
                recorded_bounds.set(Some(bounds));
                bounds
            },
            move |bounds, _, window, _| {
                let outer = crate::screenshot_tool::PreviewRect {
                    left: bounds.origin.x.into(),
                    top: bounds.origin.y.into(),
                    width: bounds.size.width.into(),
                    height: bounds.size.height.into(),
                };
                painted_bounds.set(None);
                if let (Some(image), Some((width, height))) = (&image, dimensions) {
                    if let Some(fit) = outer.fit(width, height) {
                        // 이미지 고유 비율의 자동 레이아웃 대신 선택 좌표와 동일한 fit에 그린다.
                        let image_bounds = Bounds::new(
                            point(px(fit.left), px(fit.top)),
                            gpui::size(px(fit.width), px(fit.height)),
                        );
                        if let Err(error) = window.paint_image(
                            image_bounds,
                            Default::default(),
                            image.clone(),
                            0,
                            false,
                        ) {
                            log::error!("스크린샷 미리보기 렌더 실패: {error:#}");
                        } else {
                            painted_bounds.set(Some(image_bounds));
                        }
                    }
                }
                if let (Some(selection), Some((width, height))) = (selection, dimensions) {
                    if let Some(fit) = outer.fit(width, height) {
                        let x = fit.left + selection.x as f32 / width as f32 * fit.width;
                        let y = fit.top + selection.y as f32 / height as f32 * fit.height;
                        let w = selection.width as f32 / width as f32 * fit.width;
                        let h = selection.height as f32 / height as f32 * fit.height;
                        window.paint_quad(gpui::quad(
                            Bounds::new(point(px(x), px(y)), gpui::size(px(w), px(h))),
                            px(0.0),
                            fill,
                            px(2.0),
                            accent,
                            gpui::BorderStyle::Solid,
                        ));
                    }
                }
            },
        )
        .absolute()
        .inset_0()
        .size_full(),
    )
    .into_any_element()
}
