//! 패널 공통 UI 프리미티브.
//!
//! 상태 배지와 액션 버튼은 원래 패널마다 따로 정의돼 있었다
//! (`ad_block::badge` · `service_view::state_badge` · `service_mgr` 인라인,
//! `file_sync::action_button` · `service_view::action_button` · 인라인 다수).
//! 이름만 다를 뿐 같은 일을 하는 헬퍼가 흩어지면 **색 의미가 호출부마다 달라지므로**
//! 이곳으로 모았다. 새 패널은 여기 있는 것을 쓰고, 없으면 여기에 추가한다.
//!
//! - 테마 토큰 선택은 이 모듈이 `cx.theme()`에서 직접 읽는다. 호출부는 **의미**만 넘긴다.
//! - 반환 타입이 `Div` / `Stateful<Div>`이므로 호출부에서 폭·여백을 이어 붙일 수 있다.

use gpui::{
    div, px, AnyElement, App, ClickEvent, Div, ElementId, Hsla, InteractiveElement, IntoElement,
    ParentElement, SharedString, Stateful, StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{h_flex, switch::Switch, theme::ActiveTheme, v_flex};

/// 배지·버튼의 여백 크기.
///
/// 목록 행처럼 높이가 빡빡한 곳은 `Sm`, 일반 버튼은 `Md`, 독립 카드의 주요 액션은 `Lg`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Size {
    /// `px_2 py_1` — 가상 리스트 행 내부
    Sm,
    /// `px_3 py_1` — 기본
    Md,
    /// `px_4 py_2` — 카드의 주요 액션
    Lg,
}

impl Size {
    fn apply<T: Styled>(self, el: T) -> T {
        match self {
            Size::Sm => el.px_2().py_1(),
            Size::Md => el.px_3().py_1(),
            Size::Lg => el.px_4().py_2(),
        }
    }
}

/// 상태 배지의 의미 색상.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// 정상 동작 중
    Success,
    /// 주의가 필요한 중간 상태
    Warning,
    /// 진행 중 등 정보성 상태
    Info,
    /// 비활성·미등록
    Muted,
}

impl Tone {
    fn colors(self, cx: &App) -> (Hsla, Hsla) {
        let t = cx.theme();
        match self {
            Tone::Success => (t.success, t.success_foreground),
            Tone::Warning => (t.warning, t.warning_foreground),
            Tone::Info => (t.info, t.info_foreground),
            Tone::Muted => (t.muted, t.muted_foreground),
        }
    }
}

/// 상태 배지. 폭을 고정하려면 반환값에 `.w(px(..))`를 이어 붙인다.
pub fn badge(label: impl Into<SharedString>, tone: Tone, size: Size, cx: &App) -> Div {
    let (bg, fg) = tone.colors(cx);
    size.apply(div().rounded_md())
        .bg(bg)
        .text_color(fg)
        .child(label.into())
}

/// 로그 한 줄의 레벨 표시 칸.
///
/// 대시보드「최근 활동」과 로그 패널이 같은 칸을 쓰는데 폭이 64px과 72px로 갈려 있었고,
/// 좁은 쪽에서는 `SUCCESS`가 두 줄로 접혀 잘려 보였다. 레벨 → 색 매핑도 두 곳에
/// 복제돼 있어 함께 모은다.
pub fn log_level_label(level: &str, cx: &App) -> Div {
    let t = cx.theme();
    let color = match level {
        "SUCCESS" => t.success,
        "WARN" => t.warning,
        "ERROR" => t.danger,
        _ => t.info,
    };

    div()
        .w(px(76.0))
        .flex_shrink_0()
        .whitespace_nowrap()
        .text_color(color)
        .child(level.to_string())
}

/// 액션 버튼의 색 구성.
///
/// 생성자로 의미를 고르고, 개별 화면이 다른 경우에만 `border`/`hover`로 덮어쓴다.
/// 덮어쓰기가 늘어나면 그 자체가 통일 대상이라는 신호다.
#[derive(Clone, Copy)]
pub struct ButtonStyle {
    bg: Hsla,
    fg: Hsla,
    border: Option<Hsla>,
    hover: Option<Hsla>,
}

impl ButtonStyle {
    /// 주요 액션 — 실행·적용
    pub fn primary(cx: &App) -> Self {
        let t = cx.theme();
        Self {
            bg: t.primary,
            fg: t.primary_foreground,
            border: None,
            hover: Some(t.primary_hover),
        }
    }

    /// 보조 액션 — 카드 표면 위의 기본 버튼
    pub fn neutral(cx: &App) -> Self {
        let t = cx.theme();
        Self {
            bg: t.list,
            fg: t.foreground,
            border: Some(t.border),
            hover: Some(t.secondary_hover),
        }
    }

    /// 중립 액션 — 취소·닫기
    pub fn secondary(cx: &App) -> Self {
        let t = cx.theme();
        Self {
            bg: t.secondary,
            fg: t.foreground,
            border: Some(t.border),
            hover: Some(t.secondary_hover),
        }
    }

    /// 위험 액션 — 중지·삭제
    pub fn danger(cx: &App) -> Self {
        let t = cx.theme();
        Self {
            bg: t.danger,
            fg: t.danger_foreground,
            border: None,
            hover: Some(t.danger_active),
        }
    }

    /// 확인이 필요한 위험 액션 — 배경 없이 테두리로만 강조
    pub fn danger_outline(cx: &App) -> Self {
        let t = cx.theme();
        Self {
            bg: t.background,
            fg: t.danger,
            border: Some(t.danger_foreground),
            hover: Some(t.secondary_hover),
        }
    }

    /// 지금 누를 수 없거나 의미가 없는 상태 — 눌림 자체는 막지 않는다
    pub fn muted(cx: &App) -> Self {
        let t = cx.theme();
        Self {
            bg: t.muted,
            fg: t.muted_foreground,
            border: None,
            hover: None,
        }
    }

    pub fn border(mut self, color: Hsla) -> Self {
        self.border = Some(color);
        self
    }

    pub fn hover(mut self, color: Hsla) -> Self {
        self.hover = Some(color);
        self
    }

    pub fn no_hover(mut self) -> Self {
        self.hover = None;
        self
    }
}

/// 클릭 가능한 액션 버튼. 폭을 고정하려면 반환값에 `.w(px(..))`를 이어 붙인다.
///
/// `on_click`에는 보통 `cx.listener(..)`의 결과를 넘긴다.
pub fn action_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    size: Size,
    style: ButtonStyle,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let mut el = size
        .apply(div().rounded_md())
        .cursor_pointer()
        .bg(style.bg)
        .text_color(style.fg);

    if let Some(border) = style.border {
        el = el.border_1().border_color(border);
    }
    if let Some(hover) = style.hover {
        el = el.hover(move |s| s.bg(hover));
    }

    el.id(id).on_click(on_click).child(label.into())
}

/// 숫자 하나를 강조해 보여주는 통계 타일.
///
/// 가로로 나열되는 것을 전제로 `flex_1`을 포함한다. 폭을 고정하려면 반환값에 `.w(px(..))`를
/// 이어 붙인다. (원래 `ad_block::stat_card`와 `dashboard::stat_tile`로 나뉘어 있었고,
/// 한쪽만 색을 인자로 받는 차이 외에는 완전히 같은 구현이었다.)
pub fn stat_tile(label: impl Into<SharedString>, value: impl Into<SharedString>, cx: &App) -> Div {
    let t = cx.theme();

    div()
        .flex_1()
        .rounded_md()
        .px_3()
        .py_3()
        .bg(t.list)
        .border_1()
        .border_color(t.border)
        .child(
            v_flex()
                .gap_1()
                .child(div().text_color(t.muted_foreground).child(label.into()))
                .child(div().text_color(t.foreground).child(value.into())),
        )
}

/// 제목·설명과 토글 스위치를 좌우로 배치한 설정 행.
///
/// `id`로 행(`{id}-row`)과 스위치(`{id}`) 양쪽에 `debug_selector`를 부여해 GPUI 테스트가
/// 두 지점을 각각 집을 수 있게 한다.
pub fn option_row(
    id: &'static str,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    checked: bool,
    on_click: impl Fn(&bool, &mut Window, &mut App) + 'static,
    cx: &App,
) -> AnyElement {
    let t = cx.theme();

    h_flex()
        .debug_selector(move || format!("{id}-row"))
        .gap_3()
        .items_center()
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(div().text_color(t.foreground).child(title.into()))
                .child(
                    div()
                        .text_color(t.muted_foreground)
                        .child(description.into()),
                ),
        )
        .child(
            div()
                .debug_selector(move || id.to_string())
                .child(toggle_switch(id, checked, cx).on_click(on_click)),
        )
        .into_any_element()
}

/// 프리셋 목록에서 하나를 고르는 칩. 선택 상태에 따라 배경·테두리가 바뀐다.
///
/// `on_click`은 호출부에서 `cx.listener(..)`로 이어 붙인다.
/// 감시 주기·스캔 주기·로그 보관 프리셋이 각자 같은 스타일 체인을 복제하고 있었다.
pub fn choice_chip(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    cx: &App,
) -> Stateful<Div> {
    let t = cx.theme();
    let (bg, fg, border) = if selected {
        (t.primary, t.primary_foreground, t.primary_hover)
    } else {
        (t.list, t.foreground, t.border)
    };
    let hover = t.secondary_hover;

    div()
        .id(id)
        .rounded_md()
        .px_3()
        .py_2()
        .cursor_pointer()
        .bg(bg)
        .text_color(fg)
        .border_1()
        .border_color(border)
        .hover(move |s| s.bg(hover))
        .child(label.into())
}

/// 테마에 상관없이 트랙 경계가 보이는 토글 스위치.
///
/// 스위치 내부 색은 앱 테마 적용 시 보정하고, 여기서는 현재 상태의 트랙과 대비되는
/// 외곽선을 더한다. 호출부는 gpui-component `Switch`처럼 `on_click`을 이어 붙인다.
pub fn toggle_switch(id: impl Into<ElementId>, checked: bool, cx: &App) -> Switch {
    let outline = crate::theme::switch_outline(cx.theme(), checked);
    Switch::new(id)
        .checked(checked)
        .rounded_full()
        .border_1()
        .border_color(outline)
}
