pub mod ad_block;
pub mod dashboard;
pub mod file_sync;
pub mod interval;
pub mod log_view;
pub mod service_mgr;
pub mod service_view;
pub mod settings;
pub mod ui;

use gpui::{
    div, AnyElement, InteractiveElement, IntoElement, ParentElement, Pixels, ScrollHandle,
    StatefulInteractiveElement, Styled,
};
use gpui_component::{
    resizable::{h_resizable, resizable_panel},
    scroll::{Scrollbar, ScrollbarShow},
};

pub const SETTINGS_PANE_MIN_WIDTH: Pixels = gpui::px(300.0);

/// 좌·우 pane이 가용 너비를 함께 채우는 수평 스플리터를 만든다.
///
/// 어느 한쪽에도 고정 초기 너비를 주지 않아 기본 배치는 균형 있게 시작하며, 각 pane은
/// 지정한 최소 너비 아래로 줄어들지 않는다.
pub fn balanced_split(
    id: &'static str,
    left_min_width: Pixels,
    right_min_width: Pixels,
    left: AnyElement,
    right: AnyElement,
) -> AnyElement {
    h_resizable(id)
        .child(
            resizable_panel()
                .size_range(left_min_width..Pixels::MAX)
                .child(
                    div()
                        .debug_selector(move || format!("{id}-left-pane"))
                        .size_full()
                        .min_w_0()
                        .min_h_0()
                        .child(left),
                ),
        )
        .child(
            resizable_panel()
                .size_range(right_min_width..Pixels::MAX)
                .child(
                    div()
                        .debug_selector(move || format!("{id}-right-pane"))
                        .size_full()
                        .min_w_0()
                        .min_h_0()
                        .child(right),
                ),
        )
        .into_any_element()
}

/// 컨텐츠가 표시 영역을 넘으면 세로 스크롤할 수 있고 스크롤바가 보이는 영역으로 감싼다.
///
/// 스플리터로 나뉜 각 패널은 높이가 고정되므로 내부에서 따로 스크롤해야 한다.
/// `content` 루트는 `size_full`/`h_full`로 뷰포트 높이에 고정하지 않고 자연 높이를 유지해야 한다.
///
/// 가로는 `overflow_x_hidden`으로 잠근다. 세로만 스크롤로 열어 두면 가로가 `visible`로
/// 남아, 컨텐츠가 뷰포트 폭 대신 자기 내용 폭으로 잡히는 상황이 생긴다. 그러면 카드마다
/// 폭이 제각각이 되고(섹션마다 다른 오른쪽 끝) 페이지가 가로로 삐져나간다.
/// 이 페이지들은 어차피 가로 스크롤을 쓰지 않으므로 폭은 뷰포트에 고정하는 것이 맞다.
pub fn scroll_pane(id: &'static str, handle: &ScrollHandle, content: AnyElement) -> AnyElement {
    div()
        .relative()
        .size_full()
        .min_h_0()
        .child(
            div()
                .id(id)
                .debug_selector(move || id.to_string())
                .size_full()
                .min_h_0()
                .overflow_x_hidden()
                .overflow_y_scroll()
                .track_scroll(handle)
                .child(content),
        )
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0()
                .child(Scrollbar::vertical(handle).scrollbar_show(ScrollbarShow::Always)),
        )
        .into_any_element()
}
