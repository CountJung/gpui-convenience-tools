pub mod ad_block;
pub mod dashboard;
pub mod file_sync;
pub mod log_view;
pub mod service_mgr;
pub mod service_view;
pub mod settings;
pub mod ui;

use gpui::{
    div, AnyElement, ElementId, InteractiveElement, IntoElement, ParentElement, ScrollHandle,
    StatefulInteractiveElement, Styled,
};
use gpui_component::scroll::{Scrollbar, ScrollbarShow};

/// 컨텐츠가 표시 영역을 넘으면 세로 스크롤할 수 있고 스크롤바가 보이는 영역으로 감싼다.
///
/// 스플리터로 나뉜 각 패널은 높이가 고정되므로 내부에서 따로 스크롤해야 한다.
/// `content` 루트는 `size_full`/`h_full`로 뷰포트 높이에 고정하지 않고 자연 높이를 유지해야 한다.
pub fn scroll_pane(
    id: impl Into<ElementId>,
    handle: &ScrollHandle,
    content: AnyElement,
) -> AnyElement {
    div()
        .relative()
        .size_full()
        .min_h_0()
        .child(
            div()
                .id(id)
                .size_full()
                .min_h_0()
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
