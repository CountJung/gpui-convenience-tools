//! Windows 서비스 관리 패널.
//!
//! 좌측(기능 영역)은 설치된 Win32 서비스 목록과 시작/중지/삭제 제어를,
//! 우측(설정 영역)은 검색·상태 필터·즐겨찾기 등 보기 설정을 담당한다.
//! 서비스 제어는 관리자 권한이 필요하며, 권한이 없으면 배너로 안내한다.

use gpui::{
    div, px, size, AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{
    h_flex,
    input::Input,
    scroll::{Scrollbar, ScrollbarShow},
    theme::ActiveTheme,
    v_flex, v_virtual_list,
};
use std::{ops::Range, rc::Rc};

use crate::app::{AppRoot, ServiceFilter};
use crate::platform::SysServiceStatus;
use crate::window::ui::{self, ButtonStyle, Tone};
use crate::window::{resizable_split, scroll_pane, SETTINGS_PANE_MIN_WIDTH};

mod actions;
mod layout;
use actions::Action;
pub use layout::render;
use layout::row_min_width;

fn render_service_list(
    this: &mut AppRoot,
    window: &Window,
    cx: &mut Context<AppRoot>,
) -> AnyElement {
    let em = window.rem_size();
    let pending_delete = this.services.pending_delete.clone();
    let svc_scroll = this.services.scroll.clone();

    let theme = cx.theme();

    // ── 관리자 권한 배너 ──
    let is_elevated = this.platform.is_elevated();

    // ── 검색 + 상태 필터 ──
    let search = this.services.search_query.to_lowercase();
    let filter = this.services.filter;
    let favorites = this.services.favorites.clone();
    let filtered_indices: Rc<Vec<usize>> = Rc::new(
        this.services
            .items
            .iter()
            .enumerate()
            .filter(|(_, svc)| {
                let matches_search = search.is_empty()
                    || svc.name.to_lowercase().contains(&search)
                    || svc.display_name.to_lowercase().contains(&search);

                let matches_filter = match filter {
                    ServiceFilter::All => true,
                    ServiceFilter::Running => svc.status == SysServiceStatus::Running,
                    ServiceFilter::Stopped => svc.status == SysServiceStatus::Stopped,
                    ServiceFilter::Favorites => favorites.iter().any(|n| n == &svc.name),
                };

                matches_search && matches_filter
            })
            .map(|(ix, _)| ix)
            .collect(),
    );
    let total_count = this.services.items.len();
    let filtered_count = filtered_indices.len();
    let min_width = row_min_width(this, window, &filtered_indices);

    // ── 서비스 목록 본문 ──
    let list_body: AnyElement = if this.services.items.is_empty() {
        div()
            .px_3()
            .py_4()
            .text_color(theme.muted_foreground)
            .child(if this.services.list_failed {
                "서비스 목록 조회에 실패했습니다. 새로고침하여 다시 조회하세요."
            } else {
                "서비스 목록을 불러오려면 새로고침 버튼을 누르세요."
            })
            .into_any_element()
    } else if filtered_count == 0 {
        div()
            .px_3()
            .py_4()
            .text_color(theme.muted_foreground)
            .child("검색 결과가 없습니다.")
            .into_any_element()
    } else {
        let item_sizes = Rc::new(
            filtered_indices
                .iter()
                .map(|_| size(px(0.), px(88.0)))
                .collect::<Vec<_>>(),
        );
        let idx = Rc::clone(&filtered_indices);
        let scroll = svc_scroll.clone();

        v_virtual_list(
            cx.entity(),
            "service-mgr-vlist",
            item_sizes,
            move |this, visible_range: Range<usize>, _window, cx| {
                let theme = cx.theme();
                visible_range
                    .map(|list_ix| {
                        let Some(&svc_ix) = idx.get(list_ix) else {
                            return div().h(px(88.0));
                        };
                        let Some(svc) = this.services.items.get(svc_ix) else {
                            return div().h(px(88.0));
                        };

                        let badge_tone = match svc.status {
                            SysServiceStatus::Running => Tone::Success,
                            SysServiceStatus::Stopped => Tone::Muted,
                            SysServiceStatus::StartPending | SysServiceStatus::StopPending => {
                                Tone::Warning
                            }
                            _ => Tone::Muted,
                        };
                        let status_label = svc.status.to_string();
                        let display = svc.display_name.clone();
                        let name_label = svc.name.clone();
                        let name_fav = svc.name.clone();
                        let is_favorite = this.is_favorite_service(&svc.name);

                        div()
                            .debug_selector(move || format!("service-row-{svc_ix}"))
                            .h(px(88.0))
                            .px_3()
                            .pr(px(20.0))
                            .py_2()
                            .border_b_1()
                            .border_color(theme.border)
                            .hover(|s| s.bg(theme.secondary_hover))
                            .child(
                                v_flex()
                                    .h_full()
                                    .gap_2()
                                    .justify_center()
                                    // 서비스 식별 정보는 제어 버튼과 행을 나눠 가용 폭을 온전히 쓴다.
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div()
                                                    .id(("svc-fav", svc_ix))
                                                    .w(px(24.0))
                                                    .flex_shrink_0()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .cursor_pointer()
                                                    .text_color(if is_favorite {
                                                        theme.warning
                                                    } else {
                                                        theme.muted_foreground
                                                    })
                                                    .on_click(cx.listener(
                                                        move |this, _ev, _window, cx| {
                                                            this.toggle_favorite_service(&name_fav);
                                                            cx.notify();
                                                        },
                                                    ))
                                                    .child(if is_favorite { "★" } else { "☆" }),
                                            )
                                            .child(
                                                v_flex()
                                                    .debug_selector(move || {
                                                        format!("service-row-{svc_ix}-identity")
                                                    })
                                                    .flex_1()
                                                    .text_size(em)
                                                    .min_w_0()
                                                    .overflow_hidden()
                                                    .child(
                                                        div()
                                                            .whitespace_nowrap()
                                                            .text_color(theme.foreground)
                                                            .child(display),
                                                    )
                                                    .child(
                                                        div()
                                                            .whitespace_nowrap()
                                                            .text_color(theme.muted_foreground)
                                                            .child(name_label),
                                                    ),
                                            ),
                                    )
                                    // 상태와 작업은 별도 줄에서 고정 폭을 보존한다.
                                    .child(
                                        h_flex()
                                            .debug_selector(move || {
                                                format!("service-row-{svc_ix}-controls")
                                            })
                                            .w_full()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                ui::badge(
                                                    status_label,
                                                    badge_tone,
                                                    ui::Size::Sm,
                                                    cx,
                                                )
                                                .w(px(90.0))
                                                .flex_shrink_0()
                                                .whitespace_nowrap(),
                                            )
                                            .child(
                                                h_flex()
                                                    .debug_selector(move || {
                                                        format!("service-row-{svc_ix}-actions")
                                                    })
                                                    .w(px(172.0))
                                                    .flex_shrink_0()
                                                    .gap_2()
                                                    .child(Action::Start.button(
                                                        svc,
                                                        svc_ix,
                                                        is_elevated,
                                                        cx,
                                                    ))
                                                    .child(Action::Stop.button(
                                                        svc,
                                                        svc_ix,
                                                        is_elevated,
                                                        cx,
                                                    ))
                                                    .child(Action::Delete.button(
                                                        svc,
                                                        svc_ix,
                                                        is_elevated,
                                                        cx,
                                                    )),
                                            ),
                                    ),
                            )
                    })
                    .collect::<Vec<_>>()
            },
        )
        .track_scroll(&scroll)
        .into_any_element()
    };

    // ── 최종 레이아웃 ──
    let mut root = v_flex().size_full().gap_3();

    // 제목 + 새로고침 버튼
    root = root.child(
        h_flex()
            .justify_between()
            .items_center()
            .child(
                div()
                    .text_color(theme.foreground)
                    .child("Windows 서비스 관리"),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_color(theme.muted_foreground)
                            .child(if total_count > 0 {
                                format!("{filtered_count} / {total_count}")
                            } else {
                                String::new()
                            }),
                    )
                    .child(
                        ui::action_button(
                            "svc-refresh-btn",
                            "새로고침",
                            ui::Size::Md,
                            ButtonStyle::neutral(cx),
                            cx.listener(|this, _ev, _window, cx| {
                                this.refresh_sys_services();
                                cx.notify();
                            }),
                        )
                        .debug_selector(|| "svc-refresh".into()),
                    ),
            ),
    );

    // 관리자 권한 배너
    if !is_elevated {
        root = root.child(
            div()
                .rounded_md()
                .px_3()
                .py_2()
                .bg(theme.warning)
                .text_color(theme.warning_foreground)
                .child(
                    "⚠ 서비스 시작/중지/삭제는 관리자 권한이 필요합니다. 관리자 권한으로 재실행하세요.",
                ),
        );
    }

    // ── 서비스 삭제 확인 배너 ──
    if let Some(ref pending_name) = pending_delete {
        let del_confirm = pending_name.clone();
        let reason = Action::Delete.cached_reason(this, pending_name);
        root = root.child(
            div()
                .rounded_md()
                .px_3()
                .py_3()
                .bg(theme.danger)
                .border_1()
                .border_color(theme.danger_active)
                .child(
                    v_flex()
                        .gap_2()
                        .child(div().text_color(theme.danger_foreground).child(format!(
                            "⚠ '{}' 서비스를 삭제하면 되돌릴 수 없습니다. 계속하시겠습니까?",
                            pending_name
                        )))
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    ui::gated_action_button(
                                        "svc-confirm-delete-btn",
                                        "삭제 확인",
                                        ui::Size::Md,
                                        if reason.is_some() {
                                            ButtonStyle::muted(cx)
                                        } else {
                                            ButtonStyle::danger_outline(cx)
                                        },
                                        reason,
                                        cx.listener(move |this, _, window, cx| {
                                            actions::confirm_delete(this, &del_confirm, window, cx);
                                        }),
                                    )
                                    .debug_selector(|| "svc-confirm".into()),
                                )
                                .child(
                                    ui::action_button(
                                        "svc-cancel-delete-btn",
                                        "취소",
                                        ui::Size::Md,
                                        ButtonStyle::secondary(cx),
                                        cx.listener(|this, _ev, _window, cx| {
                                            this.services.pending_delete = None;
                                            cx.notify();
                                        }),
                                    )
                                    .debug_selector(|| "svc-cancel".into()),
                                ),
                        ),
                ),
        );
    }

    // 테이블 카드
    let mut horizontal = div().id("svc-list-x").overflow_x_scroll();
    // 세로 휠을 바깥 가로 스크롤의 대체 입력으로 해석하지 않는다.
    horizontal.style().restrict_scroll_to_axis = Some(true);
    root = root.child(
        div()
            .relative()
            .min_w_0()
            .overflow_hidden()
            .flex_1()
            .min_h_0()
            .rounded_lg()
            .bg(theme.secondary)
            .border_1()
            .border_color(theme.border)
            .child(
                horizontal
                    .debug_selector(|| "svc-list-viewport".into())
                    .size_full()
                    .track_scroll(&this.services.horizontal)
                    .child(
                        v_flex()
                            .size_full()
                            .min_w(min_width)
                            .pb(px(12.))
                            // 컬럼 헤더
                            .child(
                                h_flex()
                                    .px_3()
                                    .pr(px(20.0))
                                    .py_2()
                                    .border_b_1()
                                    .border_color(theme.border)
                                    .child(
                                        div()
                                            .text_color(theme.muted_foreground)
                                            .child("서비스 · 상태 · 작업"),
                                    ),
                            )
                            // 목록 본문
                            .child(div().relative().flex_1().min_h_0().child(list_body)),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .top(px(40.))
                    .right_0()
                    .bottom(px(12.))
                    .w(px(12.))
                    .child(Scrollbar::vertical(&svc_scroll).scrollbar_show(ScrollbarShow::Always)),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h(px(12.))
                    .child(
                        Scrollbar::horizontal(&this.services.horizontal)
                            .scrollbar_show(ScrollbarShow::Always),
                    ),
            ),
    );

    root.into_any_element()
}

// ─────────────────────────────────────────────
// 우측: 보기 설정
// ─────────────────────────────────────────────

const FILTERS: [(ServiceFilter, &str, &str); 4] = [
    (ServiceFilter::All, "전체", "설치된 모든 서비스"),
    (ServiceFilter::Running, "실행 중", "Running 상태만"),
    (ServiceFilter::Stopped, "중지됨", "Stopped 상태만"),
    (ServiceFilter::Favorites, "즐겨찾기", "★ 표시한 서비스만"),
];

fn render_view_settings(this: &mut AppRoot, cx: &mut Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();
    let fg = theme.foreground;
    let muted_fg = theme.muted_foreground;
    let border = theme.border;
    let card = theme.secondary;

    let search_input = this.services.search_input.clone();
    let current_filter = this.services.filter;
    let favorite_count = this.services.favorites.len();
    let is_elevated = this.platform.is_elevated();

    let mut filter_rows = v_flex().gap_2();
    for (filter, label, description) in FILTERS {
        let is_selected = current_filter == filter;
        filter_rows = filter_rows.child(
            div()
                .id(("svc-filter", label.as_ptr() as usize))
                .debug_selector(move || format!("svc-filter-{filter:?}"))
                .rounded_md()
                .px_3()
                .py_2()
                .cursor_pointer()
                .bg(if is_selected {
                    theme.primary
                } else {
                    theme.list
                })
                .border_1()
                .border_color(if is_selected {
                    theme.primary_hover
                } else {
                    border
                })
                .hover(|s| s.bg(theme.secondary_hover))
                .on_click(cx.listener(move |this, _ev, _window, cx| {
                    this.services.filter = filter;
                    cx.notify();
                }))
                .child(
                    v_flex()
                        .child(
                            div()
                                .text_color(if is_selected {
                                    theme.primary_foreground
                                } else {
                                    fg
                                })
                                .child(label),
                        )
                        .child(
                            div()
                                .text_color(if is_selected {
                                    theme.primary_foreground
                                } else {
                                    muted_fg
                                })
                                .child(description),
                        ),
                ),
        );
    }

    v_flex()
        .w_full()
        .gap_3()
        .p_1()
        .child(div().text_color(fg).child("보기 설정"))
        // ── 검색 ──
        .child(
            div()
                .rounded_lg()
                .bg(card)
                .border_1()
                .border_color(border)
                .p_4()
                .child(
                    v_flex()
                        .gap_2()
                        .child(div().text_color(fg).child("이름 검색"))
                        .child(
                            div()
                                .text_color(muted_fg)
                                .child("서비스 이름과 표시 이름을 함께 검색합니다."),
                        )
                        .children(search_input.as_ref().map(Input::new)),
                ),
        )
        // ── 상태 필터 ──
        .child(
            div()
                .rounded_lg()
                .bg(card)
                .border_1()
                .border_color(border)
                .p_4()
                .child(
                    v_flex()
                        .gap_2()
                        .child(div().text_color(fg).child("상태 필터"))
                        .child(
                            div()
                                .text_color(muted_fg)
                                .child(format!("즐겨찾기 {favorite_count}개 등록됨")),
                        )
                        .child(filter_rows),
                ),
        )
        // ── 권한 상태 ──
        .child(
            div()
                .rounded_lg()
                .bg(card)
                .border_1()
                .border_color(border)
                .p_4()
                .child(
                    v_flex()
                        .gap_2()
                        .child(div().text_color(fg).child("실행 권한"))
                        .child(
                            div()
                                .rounded_md()
                                .px_3()
                                .py_2()
                                .bg(if is_elevated { theme.success } else { theme.warning })
                                .text_color(if is_elevated {
                                    theme.success_foreground
                                } else {
                                    theme.warning_foreground
                                })
                                .child(if is_elevated {
                                    "관리자 권한으로 실행 중"
                                } else {
                                    "일반 권한 — 시작/중지/삭제 불가"
                                }),
                        )
                        .child(
                            div()
                                .text_color(muted_fg)
                                .child("서비스 제어에는 관리자 권한이 필요합니다. 앱을 관리자 권한으로 다시 실행하세요."),
                        ),
                ),
        )
        .into_any_element()
}
