//! 본문 실측 폭에 따른 설정 전환과 서비스 식별 열의 글리프 폭 측정.
use super::*;

const FEATURE_PANE_MIN_WIDTH: gpui::Pixels = px(400.0);

pub fn render(this: &mut AppRoot, window: &mut Window, cx: &mut Context<AppRoot>) -> AnyElement {
    // ensure_service_search_input은 cx를 mut으로 빌리므로 theme() 이전에 호출
    this.ensure_service_search_input(window, cx);

    let right_scroll = this.services.right_scroll.clone();
    let available = this.services.available_width;
    let split_min = FEATURE_PANE_MIN_WIDTH + SETTINGS_PANE_MIN_WIDTH + px(1.);
    let owner = cx.entity();
    let mut body = v_flex()
        .relative()
        .size_full()
        .min_w_0()
        .min_h_0()
        .overflow_hidden();
    if available < split_min {
        let open = this.services.settings_open;
        body = body.child(
            h_flex().flex_shrink_0().justify_end().child(
                ui::action_button(
                    "svc-settings-toggle",
                    if open {
                        "목록으로 돌아가기"
                    } else {
                        "보기 설정"
                    },
                    ui::Size::Sm,
                    ButtonStyle::neutral(cx),
                    cx.listener(|this, _, _, cx| {
                        this.services.settings_open = !this.services.settings_open;
                        cx.notify();
                    }),
                )
                .debug_selector(|| "svc-settings-toggle".into()),
            ),
        );
        let panel = if open {
            scroll_pane(
                "service-mgr-right",
                &right_scroll,
                render_view_settings(this, cx),
            )
        } else {
            render_service_list(this, window, cx)
        };
        body = body.child(div().flex_1().min_h_0().child(panel));
    } else {
        let layout_owner = owner.clone();
        let list = render_service_list(this, window, cx);
        let settings = render_view_settings(this, cx);
        let left = this
            .services
            .wide_list_width
            .map(px)
            .unwrap_or(available / 2.)
            .clamp(FEATURE_PANE_MIN_WIDTH, available - SETTINGS_PANE_MIN_WIDTH);
        body = body.child(resizable_split(
            "service-mgr-split",
            left,
            FEATURE_PANE_MIN_WIDTH..gpui::Pixels::MAX,
            SETTINGS_PANE_MIN_WIDTH,
            list,
            scroll_pane("service-mgr-right", &right_scroll, settings),
            move |state, _, app| {
                if let Some(width) = state.read(app).sizes().first().copied() {
                    layout_owner.update(app, |this, _| {
                        let width = Some(f32::from(width));
                        this.services.wide_list_width = width;
                        #[cfg(test)]
                        if !this.sync.external_side_effects_enabled {
                            return;
                        }
                        if let Err(err) =
                            crate::config::update_config(|cfg| cfg.service_list_width = width)
                        {
                            log::warn!("서비스 목록 폭 저장 실패: {err}");
                        }
                    });
                }
            },
        ));
    }
    body.child(
        gpui::canvas(
            move |bounds, _, cx| {
                let width = bounds.size.width;
                if available != width {
                    let owner = owner.clone();
                    cx.defer(move |cx| {
                        owner.update(cx, |this, cx| {
                            if this.services.available_width != width {
                                if width >= split_min {
                                    this.services.settings_open = false;
                                }
                                this.services.available_width = width;
                                cx.notify();
                            }
                        })
                    });
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full(),
    )
    .into_any_element()
}

pub(super) fn row_min_width(
    this: &mut AppRoot,
    window: &Window,
    visible: &[usize],
) -> gpui::Pixels {
    let font = window.text_style().font();
    let em = window.rem_size();
    if let Some((cached_font, cached_em, indices, width)) = &this.services.measured_names {
        if cached_font == &font && *cached_em == em && indices == visible {
            return *width;
        }
    }
    let longest = visible
        .iter()
        .map(|&i| &this.services.items[i])
        .flat_map(|svc| [&svc.display_name, &svc.name])
        .map(|s| {
            window
                .text_system()
                .shape_line(
                    s.clone().into(),
                    em,
                    &[gpui::TextRun {
                        len: s.len(),
                        font: font.clone(),
                        color: window.text_style().color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    }],
                    None,
                )
                .width
        })
        .max()
        .unwrap_or_default();
    // 행 여백, 즐겨찾기, 간격 및 오른쪽 세로 스크롤바 자리를 확보한다.
    let width = (longest + em * 1.25 + px(48.)).max(px(320.));
    if !this.services.items.is_empty() {
        this.services.measured_names = Some((font, em, visible.to_vec(), width));
    }
    width
}
