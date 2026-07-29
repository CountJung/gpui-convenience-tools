use gpui::{
    div, AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{
    input::Input,
    h_flex, v_flex,
    switch::Switch,
    IconName,
    theme::{ActiveTheme, Theme, ThemeMode, ThemeRegistry},
};

use crate::app::AppRoot;
use crate::config::save_theme_selection;

/// 로그 보관 설정 프리셋.
const MAX_FILES_PRESETS: [u32; 5] = [3, 5, 10, 20, 50];
const MAX_AGE_PRESETS: [u32; 5] = [0, 7, 14, 30, 90];
const MAX_SIZE_PRESETS: [u32; 5] = [1, 5, 10, 50, 100];

fn render_theme_option(
    id_seed: usize,
    theme_name: &str,
    is_selected: bool,
    bg_card: gpui::Hsla,
    fg: gpui::Hsla,
    border: gpui::Hsla,
    selected_bg: gpui::Hsla,
    selected_fg: gpui::Hsla,
    selected_border: gpui::Hsla,
    cx: &mut Context<AppRoot>,
) -> AnyElement {
    let theme_name = theme_name.to_string();
    let label = theme_name.clone();

    div()
        .rounded_md()
        .p_2()
        .cursor_pointer()
        .bg(if is_selected { selected_bg } else { bg_card })
        .text_color(if is_selected { selected_fg } else { fg })
        .border_1()
        .border_color(if is_selected { selected_border } else { border })
        .id(("theme-name", id_seed))
        .on_click(cx.listener(move |_this, _event, window, cx| {
            let selected = ThemeRegistry::global(cx)
                .themes()
                .values()
                .find(|theme| theme.name.as_ref() == theme_name.as_str())
                .cloned();

            if let Some(theme) = selected {
                Theme::global_mut(cx).apply_config(&theme);
                Theme::change(theme.mode, Some(window), cx);
                if let Err(err) = save_theme_selection(theme.mode, theme.name.as_ref()) {
                    log::error!("failed to save theme selection: {err}");
                }
            }
        }))
        .child(label)
        .into_any_element()
}

fn render_filter_chip(
    id_seed: usize,
    label: &str,
    is_selected: bool,
    bg_card: gpui::Hsla,
    fg: gpui::Hsla,
    border: gpui::Hsla,
    selected_bg: gpui::Hsla,
    selected_fg: gpui::Hsla,
    selected_border: gpui::Hsla,
    on_click: impl Fn(&mut AppRoot, &gpui::ClickEvent, &mut Window, &mut Context<AppRoot>) + 'static,
    cx: &mut Context<AppRoot>,
) -> AnyElement {
    let label = label.to_string();

    div()
        .rounded_md()
        .px_2()
        .py_1()
        .cursor_pointer()
        .bg(if is_selected { selected_bg } else { bg_card })
        .text_color(if is_selected { selected_fg } else { fg })
        .border_1()
        .border_color(if is_selected { selected_border } else { border })
        .id(("theme-filter-chip", id_seed))
        .on_click(cx.listener(on_click))
        .child(label)
        .into_any_element()
}

/// 로그 보관 설정 카드.
///
/// 개수·날짜·용량 세 기준이 함께 적용되며, 어느 하나라도 초과하면 오래된 파일이 지워진다.
fn render_log_settings(this: &mut AppRoot, cx: &mut Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();
    let fg = theme.foreground;
    let muted_fg = theme.muted_foreground;
    let border = theme.border;
    let bg_card = theme.secondary;
    let selected_bg = theme.primary;
    let selected_fg = theme.primary_foreground;
    let selected_border = theme.primary_hover;

    let config = this.log_config.clone();
    let (file_count, total_bytes) = crate::logging::log_dir_stats();
    let log_dir = crate::config::logs_path();

    // 프리셋 칩 한 줄을 만든다. `apply`는 선택된 값을 LogConfig에 반영한다.
    fn preset_row(
        id_prefix: &'static str,
        presets: &'static [u32],
        current: u32,
        label_of: fn(u32) -> String,
        apply: fn(&mut crate::config::LogConfig, u32),
        bg_card: gpui::Hsla,
        fg: gpui::Hsla,
        border: gpui::Hsla,
        selected_bg: gpui::Hsla,
        selected_fg: gpui::Hsla,
        selected_border: gpui::Hsla,
        hover_bg: gpui::Hsla,
        cx: &mut Context<AppRoot>,
    ) -> AnyElement {
        let mut row = h_flex().gap_2().flex_wrap();
        for &value in presets {
            let is_selected = current == value;
            row = row.child(
                div()
                    .id((id_prefix, value as usize))
                    .rounded_md()
                    .px_3()
                    .py_2()
                    .cursor_pointer()
                    .bg(if is_selected { selected_bg } else { bg_card })
                    .text_color(if is_selected { selected_fg } else { fg })
                    .border_1()
                    .border_color(if is_selected { selected_border } else { border })
                    .hover(|s| s.bg(hover_bg))
                    .on_click(cx.listener(move |this, _ev, _window, cx| {
                        this.update_log_config(cx, |cfg| apply(cfg, value));
                    }))
                    .child(label_of(value)),
            );
        }
        row.into_any_element()
    }

    let hover_bg = theme.secondary_hover;

    let files_row = preset_row(
        "log-max-files",
        &MAX_FILES_PRESETS,
        config.max_files,
        |v| format!("{v}개"),
        |cfg, v| cfg.max_files = v,
        bg_card, fg, border, selected_bg, selected_fg, selected_border, hover_bg,
        cx,
    );
    let age_row = preset_row(
        "log-max-age",
        &MAX_AGE_PRESETS,
        config.max_age_days,
        |v| if v == 0 { "제한 없음".to_string() } else { format!("{v}일") },
        |cfg, v| cfg.max_age_days = v,
        bg_card, fg, border, selected_bg, selected_fg, selected_border, hover_bg,
        cx,
    );
    let size_row = preset_row(
        "log-max-size",
        &MAX_SIZE_PRESETS,
        config.max_file_size_mb,
        |v| format!("{v} MB"),
        |cfg, v| cfg.max_file_size_mb = v,
        bg_card, fg, border, selected_bg, selected_fg, selected_border, hover_bg,
        cx,
    );

    div()
        .rounded_lg()
        .bg(bg_card)
        .border_1()
        .border_color(border)
        .p_4()
        .child(
            v_flex()
                .gap_3()
                .child(div().text_color(fg).child("로그 파일 보관"))
                .child(
                    div().text_color(muted_fg).child(format!(
                        "저장 위치: {} · 현재 {file_count}개 · {:.1} MB",
                        log_dir.display(),
                        total_bytes as f64 / (1024.0 * 1024.0),
                    )),
                )
                // ── 파일 기록 사용 ──
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_color(fg).child("파일로 기록"))
                                .child(
                                    div()
                                        .text_color(muted_fg)
                                        .child("끄면 화면 로그만 남고 파일에는 쓰지 않습니다."),
                                ),
                        )
                        .child(
                            Switch::new("log-file-enabled")
                                .checked(config.file_enabled)
                                .on_click(cx.listener(|this, checked: &bool, _window, cx| {
                                    let checked = *checked;
                                    this.update_log_config(cx, move |cfg| {
                                        cfg.file_enabled = checked
                                    });
                                })),
                        ),
                )
                // ── 파일 개수 ──
                .child(div().text_color(fg).child("보관 파일 수"))
                .child(
                    div()
                        .text_color(muted_fg)
                        .child("현재 파일을 포함한 최대 개수입니다. 초과분은 오래된 순으로 삭제됩니다."),
                )
                .child(files_row)
                // ── 날짜 범위 ──
                .child(div().text_color(fg).child("보관 기간"))
                .child(
                    div()
                        .text_color(muted_fg)
                        .child("이 기간보다 오래된 로그 파일을 삭제합니다."),
                )
                .child(age_row)
                // ── 파일 용량 ──
                .child(div().text_color(fg).child("파일당 최대 용량"))
                .child(
                    div()
                        .text_color(muted_fg)
                        .child("이 크기를 넘으면 새 파일로 롤링합니다."),
                )
                .child(size_row),
        )
        .into_any_element()
}

pub fn render(this: &mut AppRoot, window: &mut Window, cx: &mut Context<AppRoot>) -> AnyElement {
    this.ensure_theme_filter_input(window, cx);

    let theme = cx.theme();
    let is_dark = theme.mode == ThemeMode::Dark;
    let active_light_theme = theme.light_theme.name.to_string();
    let active_dark_theme = theme.dark_theme.name.to_string();
    let all_themes = ThemeRegistry::global(cx).sorted_themes();

    let filter_query = this.theme_filter_query.trim().to_lowercase();
    let active_only = this.theme_filter_active_only;

    let mut family_keywords = Vec::<String>::new();
    for theme in &all_themes {
        let token = theme
            .name
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string();
        if token.is_empty() || family_keywords.iter().any(|t| t == &token) {
            continue;
        }
        family_keywords.push(token);
        if family_keywords.len() >= 8 {
            break;
        }
    }

    let light_theme_names = all_themes
        .iter()
        .filter(|theme| {
            let name = theme.name.to_lowercase();
            let pass_query = filter_query.is_empty() || name.contains(&filter_query);
            let pass_active = !active_only
                || theme.name.as_ref() == active_light_theme.as_str()
                || theme.name.as_ref() == active_dark_theme.as_str();
            pass_query && pass_active
        })
        .filter(|theme| theme.mode == ThemeMode::Light)
        .map(|theme| theme.name.to_string())
        .collect::<Vec<_>>();

    let dark_theme_names = all_themes
        .iter()
        .filter(|theme| {
            let name = theme.name.to_lowercase();
            let pass_query = filter_query.is_empty() || name.contains(&filter_query);
            let pass_active = !active_only
                || theme.name.as_ref() == active_light_theme.as_str()
                || theme.name.as_ref() == active_dark_theme.as_str();
            pass_query && pass_active
        })
        .filter(|theme| theme.mode == ThemeMode::Dark)
        .map(|theme| theme.name.to_string())
        .collect::<Vec<_>>();

    let bg_card = theme.secondary;
    let fg = theme.foreground;
    let muted_fg = theme.muted_foreground;
    let border = theme.border;

    let selected_bg = theme.primary;
    let selected_fg = theme.primary_foreground;
    let selected_border = theme.primary_hover;

    let mut filter_controls = h_flex().gap_2().flex_wrap();
    filter_controls = filter_controls.child(render_filter_chip(
        0,
        "초기화",
        this.theme_filter_query.is_empty() && !this.theme_filter_active_only,
        bg_card,
        fg,
        border,
        selected_bg,
        selected_fg,
        selected_border,
        |this, _, window, cx| {
            this.set_theme_filter_query(String::new(), window, cx);
            this.theme_filter_active_only = false;
            cx.notify();
        },
        cx,
    ));
    filter_controls = filter_controls.child(render_filter_chip(
        1,
        "현재 활성만",
        this.theme_filter_active_only,
        bg_card,
        fg,
        border,
        selected_bg,
        selected_fg,
        selected_border,
        |this, _, _window, cx| {
            this.theme_filter_active_only = !this.theme_filter_active_only;
            cx.notify();
        },
        cx,
    ));
    filter_controls = filter_controls.child(render_filter_chip(
        2,
        "현재 Light",
        this.theme_filter_query == active_light_theme,
        bg_card,
        fg,
        border,
        selected_bg,
        selected_fg,
        selected_border,
        {
            let active_light_theme = active_light_theme.clone();
            move |this, _, window, cx| {
                this.set_theme_filter_query(active_light_theme.clone(), window, cx);
                cx.notify();
            }
        },
        cx,
    ));
    filter_controls = filter_controls.child(render_filter_chip(
        3,
        "현재 Dark",
        this.theme_filter_query == active_dark_theme,
        bg_card,
        fg,
        border,
        selected_bg,
        selected_fg,
        selected_border,
        {
            let active_dark_theme = active_dark_theme.clone();
            move |this, _, window, cx| {
                this.set_theme_filter_query(active_dark_theme.clone(), window, cx);
                cx.notify();
            }
        },
        cx,
    ));

    for (idx, keyword) in family_keywords.iter().enumerate() {
        filter_controls = filter_controls.child(render_filter_chip(
            10 + idx,
            keyword,
            this.theme_filter_query == *keyword,
            bg_card,
            fg,
            border,
            selected_bg,
            selected_fg,
            selected_border,
            {
                let keyword = keyword.clone();
                move |this, _, window, cx| {
                    this.set_theme_filter_query(keyword.clone(), window, cx);
                    cx.notify();
                }
            },
            cx,
        ));
    }

    let mut light_theme_list = v_flex().gap_2();
    for (idx, theme_name) in light_theme_names.into_iter().enumerate() {
        light_theme_list = light_theme_list.child(render_theme_option(
            idx,
            &theme_name,
            active_light_theme == theme_name,
            bg_card,
            fg,
            border,
            selected_bg,
            selected_fg,
            selected_border,
            cx,
        ));
    }

    let mut dark_theme_list = v_flex().gap_2();
    for (idx, theme_name) in dark_theme_names.into_iter().enumerate() {
        dark_theme_list = dark_theme_list.child(render_theme_option(
            10_000 + idx,
            &theme_name,
            active_dark_theme == theme_name,
            bg_card,
            fg,
            border,
            selected_bg,
            selected_fg,
            selected_border,
            cx,
        ));
    }

    v_flex()
        .w_full()
        .gap_3()
        .child(div().text_color(fg).child("설정"))
        .child(
            div()
                .text_color(muted_fg)
                .child("각 편의 기능의 동작 설정은 해당 기능 페이지 오른쪽 영역에 있습니다."),
        )
        .child(render_log_settings(this, cx))
        .child(
            div()
                .rounded_lg()
                .bg(bg_card)
                .border_1()
                .border_color(border)
                .p_4()
                .child(
                    v_flex()
                        .gap_3()
                        .child(div().text_color(fg).child("테마 모드"))
                        .child(div().text_color(muted_fg).child("앱의 색상 모드를 선택합니다."))
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_1()
                                        .rounded_md()
                                        .p_3()
                                        .cursor_pointer()
                                        .bg(if !is_dark { selected_bg } else { bg_card })
                                        .text_color(if !is_dark { selected_fg } else { fg })
                                        .border_1()
                                        .border_color(if !is_dark { selected_border } else { border })
                                        .id("theme-light")
                                        .on_click(cx.listener(|_this, _event, window, cx| {
                                            Theme::change(ThemeMode::Light, Some(window), cx);
                                        }))
                                        .child("Light"),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .rounded_md()
                                        .p_3()
                                        .cursor_pointer()
                                        .bg(if is_dark { selected_bg } else { bg_card })
                                        .text_color(if is_dark { selected_fg } else { fg })
                                        .border_1()
                                        .border_color(if is_dark { selected_border } else { border })
                                        .id("theme-dark")
                                        .on_click(cx.listener(|_this, _event, window, cx| {
                                            Theme::change(ThemeMode::Dark, Some(window), cx);
                                        }))
                                        .child("Dark"),
                                ),
                        ),
                ),
        )
        .child(
            div()
                .rounded_lg()
                .bg(bg_card)
                .border_1()
                .border_color(border)
                .p_4()
                .child(
                    v_flex()
                        .gap_3()
                        .child(div().text_color(fg).child("테마 선택"))
                        .child(
                            div()
                                .text_color(muted_fg)
                                .child("gpui-component themes JSON 기반 테마를 선택해 즉시 적용합니다."),
                        )
                        .child(div().text_color(fg).child("검색/필터"))
                        .children(this.theme_filter_input.as_ref().map(|input| {
                            Input::new(input)
                                .prefix(IconName::Search)
                                .cleanable(true)
                        }))
                        .child(filter_controls)
                        .child(
                            div().text_color(muted_fg).child(format!(
                                "현재 필터: query='{}', active_only={}",
                                this.theme_filter_query, this.theme_filter_active_only
                            )),
                        )
                        .child(div().text_color(fg).child("Light Themes"))
                        .child(light_theme_list)
                        .child(div().text_color(fg).child("Dark Themes"))
                        .child(dark_theme_list),
                ),
        )
        .into_any_element()
}
