/// B-2: Windows 서비스 관리 패널
///
/// 시스템에 설치된 Win32 서비스 목록을 표시하고,
/// 이름 검색 필터링과 시작/중지 제어를 제공한다.
/// 서비스 제어는 관리자 권한이 필요하며, 권한 없을 때 배너를 표시한다.

use gpui::{
    div, px, size, AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{h_flex, theme::ActiveTheme, v_flex, v_virtual_list};
use std::{ops::Range, rc::Rc};

use crate::app::AppRoot;
use crate::platform::SysServiceStatus;

pub fn render(this: &mut AppRoot, window: &mut Window, cx: &mut Context<AppRoot>) -> AnyElement {
    // ensure_service_search_input은 cx를 mut으로 빌리므로 theme() 이전에 호출
    this.ensure_service_search_input(window, cx);
    let search_input = this.service_search_input.clone();
    let pending_delete = this.pending_delete_service.clone();

    let theme = cx.theme();

    // ── 관리자 권한 배너 ──
    let is_elevated = this.platform.is_elevated();

    // ── 검색 필터 ──
    let search = this.service_search_query.to_lowercase();
    let filtered_indices: Rc<Vec<usize>> = Rc::new(
        this.sys_services
            .iter()
            .enumerate()
            .filter(|(_, svc)| {
                search.is_empty()
                    || svc.name.to_lowercase().contains(&search)
                    || svc.display_name.to_lowercase().contains(&search)
            })
            .map(|(ix, _)| ix)
            .collect(),
    );
    let total_count = this.sys_services.len();
    let filtered_count = filtered_indices.len();

    // ── 서비스 목록 본문 ──
    let list_body: AnyElement = if this.sys_services.is_empty() {
        div()
            .px_3()
            .py_4()
            .text_color(theme.muted_foreground)
            .child("서비스 목록을 불러오려면 새로고침 버튼을 누르세요.")
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
                .map(|_| size(px(0.), px(44.0)))
                .collect::<Vec<_>>(),
        );
        let idx = Rc::clone(&filtered_indices);
        let scroll = this.svc_scroll_handle.clone();

        v_virtual_list(
            cx.entity(),
            "service-mgr-vlist",
            item_sizes,
            move |this, visible_range: Range<usize>, _window, cx| {
                let theme = cx.theme();
                visible_range
                    .map(|list_ix| {
                        let Some(&svc_ix) = idx.get(list_ix) else {
                            return div().h(px(44.0));
                        };
                        let Some(svc) = this.sys_services.get(svc_ix) else {
                            return div().h(px(44.0));
                        };

                        let (badge_bg, badge_fg) = match svc.status {
                            SysServiceStatus::Running => (theme.success, theme.success_foreground),
                            SysServiceStatus::Stopped => (theme.muted, theme.muted_foreground),
                            SysServiceStatus::StartPending
                            | SysServiceStatus::StopPending => {
                                (theme.warning, theme.warning_foreground)
                            }
                            _ => (theme.muted, theme.muted_foreground),
                        };
                        let is_running = svc.status == SysServiceStatus::Running;
                        let status_label = svc.status.to_string();
                        let display = svc.display_name.clone();
                        let name_label = svc.name.clone();
                        let name_start = svc.name.clone();
                        let name_stop = svc.name.clone();
                        let name_delete = svc.name.clone();

                        div()
                            .h(px(44.0))
                            .px_3()
                            .border_b_1()
                            .border_color(theme.border)
                            .hover(|s| s.bg(theme.secondary_hover))
                            .child(
                                h_flex()
                                    .h_full()
                                    .gap_2()
                                    .items_center()
                                    // 표시 이름 + 서비스명
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .child(
                                                div()
                                                    .text_color(theme.foreground)
                                                    .child(display),
                                            )
                                            .child(
                                                div()
                                                    .text_color(theme.muted_foreground)
                                                    .child(name_label),
                                            ),
                                    )
                                    // 상태 배지
                                    .child(
                                        div()
                                            .w(px(90.0))
                                            .rounded_md()
                                            .px_2()
                                            .py_1()
                                            .bg(badge_bg)
                                            .text_color(badge_fg)
                                            .child(status_label),
                                    )
                                    // 시작 버튼
                                    .child(
                                        div()
                                            .id(("svc-start", svc_ix))
                                            .w(px(52.0))
                                            .rounded_md()
                                            .px_2()
                                            .py_1()
                                            .cursor_pointer()
                                            .bg(if is_running {
                                                theme.muted
                                            } else {
                                                theme.list
                                            })
                                            .border_1()
                                            .border_color(theme.border)
                                            .text_color(if is_running {
                                                theme.muted_foreground
                                            } else {
                                                theme.foreground
                                            })
                                            .hover(|s| s.bg(theme.secondary_hover))
                                            .on_click(cx.listener(move |this, _ev, window, cx| {
                                                match this.platform.start_sys_service(&name_start) {
                                                    Ok(()) => {
                                                        this.push_service_log(
                                                            &format!("서비스 시작: {name_start}"),
                                                            window,
                                                            cx,
                                                        );
                                                        this.refresh_sys_services();
                                                    }
                                                    Err(e) => {
                                                        this.push_service_log(
                                                            &format!("시작 실패 ({name_start}): {e}"),
                                                            window,
                                                            cx,
                                                        );
                                                    }
                                                }
                                                cx.notify();
                                            }))
                                            .child("시작"),
                                    )
                                    // 중지 버튼
                                    .child(
                                        div()
                                            .id(("svc-stop", svc_ix))
                                            .w(px(52.0))
                                            .rounded_md()
                                            .px_2()
                                            .py_1()
                                            .cursor_pointer()
                                            .bg(if is_running {
                                                theme.danger
                                            } else {
                                                theme.muted
                                            })
                                            .text_color(if is_running {
                                                theme.danger_foreground
                                            } else {
                                                theme.muted_foreground
                                            })
                                            .hover(|s| {
                                                if is_running {
                                                    s.bg(theme.danger_active)
                                                } else {
                                                    s
                                                }
                                            })
                                            .on_click(cx.listener(move |this, _ev, window, cx| {
                                                match this.platform.stop_sys_service(&name_stop) {
                                                    Ok(()) => {
                                                        this.push_service_log(
                                                            &format!("서비스 중지: {name_stop}"),
                                                            window,
                                                            cx,
                                                        );
                                                        this.refresh_sys_services();
                                                    }
                                                    Err(e) => {
                                                        this.push_service_log(
                                                            &format!("중지 실패 ({name_stop}): {e}"),
                                                            window,
                                                            cx,
                                                        );
                                                    }
                                                }
                                                cx.notify();
                                            }))
                                            .child("중지"),
                                    )
                                    // 삭제 버튼 (spacer + danger 스타일)
                                    .child(div().w(px(8.0)))
                                    .child(
                                        div()
                                            .id(("svc-delete", svc_ix))
                                            .w(px(52.0))
                                            .rounded_md()
                                            .px_2()
                                            .py_1()
                                            .cursor_pointer()
                                            .bg(theme.danger)
                                            .text_color(theme.danger_foreground)
                                            .hover(|s| s.bg(theme.danger_active))
                                            .on_click(cx.listener(move |this, _ev, _window, cx| {
                                                this.pending_delete_service = Some(name_delete.clone());
                                                cx.notify();
                                            }))
                                            .child("삭제"),
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
                h_flex().gap_2().child(
                    div()
                        .text_color(theme.muted_foreground)
                        .child(if total_count > 0 {
                            format!("{filtered_count} / {total_count}")
                        } else {
                            String::new()
                        }),
                )
                .child(
                    div()
                        .id("svc-refresh-btn")
                        .rounded_md()
                        .px_3()
                        .py_1()
                        .cursor_pointer()
                        .bg(theme.list)
                        .border_1()
                        .border_color(theme.border)
                        .text_color(theme.foreground)
                        .hover(|s| s.bg(theme.secondary_hover))
                        .on_click(cx.listener(|this, _ev, _window, cx| {
                            this.refresh_sys_services();
                            cx.notify();
                        }))
                        .child("새로고침"),
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
                    "⚠ 서비스 시작/중지는 관리자 권한이 필요합니다. 관리자 권한으로 재실행하세요.",
                ),
        );
    }

    // ── 서비스 삭제 확인 배너 ──
    if let Some(ref pending_name) = pending_delete {
        let del_confirm = pending_name.clone();
        let del_cancel = pending_name.clone();
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
                        .child(
                            div()
                                .text_color(theme.danger_foreground)
                                .child(format!(
                                    "⚠ '{}' 서비스를 삭제하면 되돌릴 수 없습니다. 계속하시겠습니까?",
                                    del_cancel
                                )),
                        )
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .id("svc-confirm-delete-btn")
                                        .rounded_md()
                                        .px_3()
                                        .py_1()
                                        .cursor_pointer()
                                        .bg(theme.background)
                                        .border_1()
                                        .border_color(theme.danger_foreground)
                                        .text_color(theme.danger)
                                        .hover(|s| s.bg(theme.secondary_hover))
                                        .on_click(cx.listener(move |this, _ev, window, cx| {
                                            match this.platform.delete_sys_service(&del_confirm) {
                                                Ok(()) => {
                                                    this.push_service_log(
                                                        &format!("서비스 삭제: {del_confirm}"),
                                                        window,
                                                        cx,
                                                    );
                                                    this.refresh_sys_services();
                                                }
                                                Err(e) => {
                                                    this.push_service_log(
                                                        &format!("삭제 실패 ({del_confirm}): {e}"),
                                                        window,
                                                        cx,
                                                    );
                                                }
                                            }
                                            this.pending_delete_service = None;
                                            cx.notify();
                                        }))
                                        .child("삭제 확인"),
                                )
                                .child(
                                    div()
                                        .id("svc-cancel-delete-btn")
                                        .rounded_md()
                                        .px_3()
                                        .py_1()
                                        .cursor_pointer()
                                        .bg(theme.secondary)
                                        .border_1()
                                        .border_color(theme.border)
                                        .text_color(theme.foreground)
                                        .hover(|s| s.bg(theme.secondary_hover))
                                        .on_click(cx.listener(|this, _ev, _window, cx| {
                                            this.pending_delete_service = None;
                                            cx.notify();
                                        }))
                                        .child("취소"),
                                ),
                        ),
                ),
        );
    }

    // 검색 입력
    if let Some(inp) = search_input {
        root = root.child(
            div()
                .rounded_md()
                .border_1()
                .border_color(theme.border)
                .bg(theme.background)
                .child(inp),
        );
    }

    // 테이블 카드
    root = root.child(
        div()
            .flex_1()
            .min_h_0()
            .rounded_lg()
            .bg(theme.secondary)
            .border_1()
            .border_color(theme.border)
            .child(
                v_flex()
                    .size_full()
                    // 컬럼 헤더
                    .child(
                        h_flex()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(theme.border)
                            .gap_2()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_color(theme.muted_foreground)
                                    .child("서비스"),
                            )
                            .child(
                                div()
                                    .w(px(90.0))
                                    .text_color(theme.muted_foreground)
                                    .child("상태"),
                            )
                            .child(
                                div()
                                    .w(px(52.0))
                                    .text_color(theme.muted_foreground)
                                    .child("시작"),
                            )
                            .child(
                                div()
                                    .w(px(52.0))
                                    .text_color(theme.muted_foreground)
                                    .child("중지"),
                            )
                            .child(div().w(px(8.0)))
                            .child(
                                div()
                                    .w(px(52.0))
                                    .text_color(theme.danger)
                                    .child("삭제"),
                            ),
                    )
                    // 목록 본문
                    .child(div().flex_1().min_h_0().child(list_body)),
            ),
    );

    root.into_any_element()
}
