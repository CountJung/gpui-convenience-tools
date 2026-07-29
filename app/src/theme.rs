//! 앱 테마 적용과 컴포넌트 대비 보정.
//!
//! gpui-component의 `Switch`는 꺼짐 트랙에 `switch.background`, 썸에
//! `switch.thumb.background`을 사용한다. 테마가 두 토큰을 생략하면 각각
//! `secondary`와 `background`으로 폴백하므로, 카드 표면과 겹쳐 보이지 않을 수 있다.

use gpui::{App, Hsla, Window};
use gpui_component::{
    theme::{Theme, ThemeMode},
    Colorize,
};

const MIN_CONTROL_CONTRAST: f32 = 3.0;

/// 테마 모드를 바꾸고 앱 컴포넌트의 최소 대비 정책을 다시 적용한다.
pub fn change_theme(mode: ThemeMode, mut window: Option<&mut Window>, cx: &mut App) {
    Theme::change(mode, window.as_deref_mut(), cx);
    normalize_component_palette(cx);

    if let Some(window) = window {
        window.refresh();
    }
}

/// 현재 테마에서 누락되거나 대비가 낮은 컴포넌트 색을 의미 토큰 기반으로 보정한다.
pub fn normalize_component_palette(cx: &mut App) {
    normalize_switch_palette(Theme::global_mut(cx));
}

/// 스위치 외곽선 색을 현재 상태의 트랙과 대비되도록 고른다.
pub(crate) fn switch_outline(theme: &Theme, checked: bool) -> Hsla {
    let track = if checked { theme.primary } else { theme.switch };

    accessible_color(
        track,
        if checked {
            [theme.primary_foreground, theme.foreground, theme.background]
        } else {
            [theme.switch_thumb, theme.foreground, theme.background]
        },
    )
}

fn normalize_switch_palette(theme: &mut Theme) {
    theme.switch = accessible_color(
        theme.background,
        [theme.muted_foreground, theme.foreground, theme.border],
    );
    theme.switch_thumb = accessible_color(
        theme.switch,
        [theme.background, theme.foreground, theme.primary_foreground],
    );
}

fn accessible_color(base: Hsla, candidates: [Hsla; 3]) -> Hsla {
    if let Some(color) = candidates
        .into_iter()
        .find(|candidate| contrast_ratio(base, *candidate) >= MIN_CONTROL_CONTRAST)
    {
        return color;
    }

    let darker = base.darken(1.0);
    let lighter = base.lighten(1.0);
    if contrast_ratio(base, darker) >= contrast_ratio(base, lighter) {
        darker
    } else {
        lighter
    }
}

fn contrast_ratio(a: Hsla, b: Hsla) -> f32 {
    let a = relative_luminance(a);
    let b = relative_luminance(b);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn relative_luminance(color: Hsla) -> f32 {
    let rgb = color.to_rgb();
    0.2126 * linear_channel(rgb.r) + 0.7152 * linear_channel(rgb.g) + 0.0722 * linear_channel(rgb.b)
}

fn linear_channel(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use gpui_component::theme::{ThemeColor, ThemeSet};

    use super::*;
    use crate::config::BUNDLED_THEMES;

    #[test]
    fn bundled_theme_switches_keep_non_text_contrast() {
        let mut theme_count = 0;

        for (file_name, contents) in BUNDLED_THEMES {
            let set: ThemeSet = serde_json::from_str(contents)
                .unwrap_or_else(|err| panic!("{file_name} 테마 파싱 실패: {err}"));

            for config in set.themes {
                theme_count += 1;
                let defaults = if config.mode.is_dark() {
                    ThemeColor::dark()
                } else {
                    ThemeColor::light()
                };
                let mut theme = Theme::from(defaults.as_ref());
                theme.apply_config(&Rc::new(config));
                normalize_switch_palette(&mut theme);

                assert!(
                    contrast_ratio(theme.background, theme.switch) >= MIN_CONTROL_CONTRAST,
                    "{} 꺼짐 트랙 대비 부족",
                    theme.theme_name()
                );
                assert!(
                    contrast_ratio(theme.switch, theme.switch_thumb) >= MIN_CONTROL_CONTRAST,
                    "{} 꺼짐 썸 대비 부족",
                    theme.theme_name()
                );
                assert!(
                    contrast_ratio(theme.primary, switch_outline(&theme, true))
                        >= MIN_CONTROL_CONTRAST,
                    "{} 켜짐 외곽선 대비 부족",
                    theme.theme_name()
                );
            }
        }

        assert_eq!(
            theme_count, 36,
            "번들 테마 변형 수가 바뀌면 감사 범위를 갱신하세요"
        );
    }
}
