use gpui::{
    div, AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{
    input::Input,
    h_flex, v_flex,
    IconName,
    theme::{ActiveTheme, Theme, ThemeMode, ThemeRegistry},
};

use crate::app::AppRoot;
use crate::config::save_theme_selection;

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

pub fn render(this: &mut AppRoot, window: &mut Window, cx: &mut Context<AppRoot>) -> AnyElement {
    this.ensure_theme_filter_input(window, cx);

    let theme = cx.theme();
    let is_dark = theme.mode == ThemeMode::Dark;
    let active_light_theme = theme.light_theme.name.to_string();
    let active_dark_theme = theme.dark_theme.name.to_string();
    let all_themes = ThemeRegistry::global(cx).sorted_themes();
    let current_interval = this.scan_interval_secs;

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
                .rounded_lg()
                .bg(bg_card)
                .border_1()
                .border_color(border)
                .p_4()
                .child(
                    v_flex()
                        .gap_3()
                        .child(div().text_color(fg).child("스캔 주기"))
                        .child(
                            div().text_color(muted_fg).child(format!(
                                "광고 창 감지 주기를 설정합니다. 현재: {}초",
                                current_interval
                            )),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .children([5u32, 10, 30, 60, 120].iter().map(|&secs| {
                                    let is_selected = current_interval == secs;
                                    div()
                                        .rounded_md()
                                        .px_3()
                                        .py_2()
                                        .cursor_pointer()
                                        .bg(if is_selected { selected_bg } else { bg_card })
                                        .text_color(if is_selected { selected_fg } else { fg })
                                        .border_1()
                                        .border_color(if is_selected { selected_border } else { border })
                                        .id(("interval-preset", secs as usize))
                                        .on_click(cx.listener(move |this, _ev, _window, cx| {
                                            this.set_scan_interval(secs, cx);
                                        }))
                                        .child(format!("{}s", secs))
                                        .into_any_element()
                                })),
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
