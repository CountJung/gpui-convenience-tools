//! 대시보드 패널.
//!
//! 현재 플랫폼에서 실제로 동작하는 기능의 상태만 요약하고 최근 활동을 보여준다.
//! 이 패널은 설정이 없어 스플리터를 쓰지 않는다.

use gpui::{div, px, AnyElement, Context, IntoElement, ParentElement, Styled};
use gpui_component::{h_flex, theme::ActiveTheme, v_flex};

use crate::app::AppRoot;
use crate::window::ui;

pub fn render(root: &AppRoot, cx: &mut Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();

    let summary = render_summary(root, cx);
    let recent: Vec<_> = root.app_state.log_entries.iter().rev().take(6).collect();
    let mut activity = v_flex().gap_1();
    if recent.is_empty() {
        activity = activity.child(div().text_color(theme.muted_foreground).child("활동 없음"));
    } else {
        for entry in &recent {
            let level_color = match entry.level.as_str() {
                "SUCCESS" => theme.success,
                "WARN" => theme.warning,
                "ERROR" => theme.danger,
                _ => theme.info,
            };
            activity = activity.child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .w(px(64.0))
                            .text_color(level_color)
                            .child(entry.level.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_color(theme.muted_foreground)
                            .child(entry.message.clone()),
                    ),
            );
        }
    }

    v_flex()
        .w_full()
        .gap_3()
        .child(div().text_color(theme.foreground).child("대시보드"))
        .child(summary)
        .child(
            div()
                .rounded_lg()
                .p_4()
                .bg(theme.secondary)
                .border_1()
                .border_color(theme.border)
                .child(
                    v_flex()
                        .gap_2()
                        .child(div().text_color(theme.foreground).child("최근 활동"))
                        .child(activity),
                ),
        )
        .into_any_element()
}

/// 상태 요약 카드 — 광고 차단(Win32 전용)과 파일 동기화를 함께 보여준다.
#[cfg(target_os = "windows")]
fn render_summary(root: &AppRoot, cx: &Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();

    let (svc_label, svc_bg, svc_fg) = if root.app_state.is_active {
        ("광고 차단: 동작 중", theme.success, theme.success_foreground)
    } else {
        ("광고 차단: 중지됨", theme.warning, theme.warning_foreground)
    };

    let (tgt_label, tgt_bg, tgt_fg) = if root.app_state.is_target_running {
        ("타겟: 실행 중", theme.success, theme.success_foreground)
    } else {
        ("타겟: 미실행", theme.muted, theme.muted_foreground)
    };

    let active_targets = root.app_state.targets.iter().filter(|t| t.enabled).count();

    summary_card(
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .rounded_md()
                            .px_3()
                            .py_1()
                            .bg(svc_bg)
                            .text_color(svc_fg)
                            .child(svc_label),
                    )
                    .child(
                        div()
                            .rounded_md()
                            .px_3()
                            .py_1()
                            .bg(tgt_bg)
                            .text_color(tgt_fg)
                            .child(tgt_label),
                    ),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(stat_tile(
                        "누적 차단",
                        &root.app_state.blocked_count.to_string(),
                        cx,
                    ))
                    .child(stat_tile(
                        "활성 타겟",
                        &format!("{active_targets} / {}", root.app_state.targets.len()),
                        cx,
                    ))
                    .children(sync_tiles(root, cx)),
            )
            .into_any_element(),
        cx,
    )
}

/// 비Windows 요약 카드 — 이 플랫폼에서 동작하는 파일 동기화만 보여준다.
#[cfg(not(target_os = "windows"))]
fn render_summary(root: &AppRoot, cx: &Context<AppRoot>) -> AnyElement {
    summary_card(
        v_flex()
            .gap_3()
            .child(h_flex().gap_3().children(sync_tiles(root, cx)))
            .into_any_element(),
        cx,
    )
}

/// 두 플랫폼이 공유하는 파일 동기화 통계 타일.
fn sync_tiles(root: &AppRoot, cx: &Context<AppRoot>) -> Vec<AnyElement> {
    let auto_sync_jobs = root.sync_jobs.iter().filter(|j| j.enabled).count();
    let failed_syncs = root.sync_status.values().filter(|s| s.failed).count();

    vec![
        stat_tile(
            "자동 동기화",
            &format!("{auto_sync_jobs} / {}", root.sync_jobs.len()),
            cx,
        ),
        stat_tile("동기화 실패", &failed_syncs.to_string(), cx),
    ]
}

fn summary_card(content: AnyElement, cx: &Context<AppRoot>) -> AnyElement {
    let theme = cx.theme();
    div()
        .rounded_lg()
        .p_4()
        .bg(theme.secondary)
        .border_1()
        .border_color(theme.border)
        .child(content)
        .into_any_element()
}

fn stat_tile(label: &'static str, value: &str, cx: &Context<AppRoot>) -> AnyElement {
    ui::stat_tile(label, value.to_string(), cx).into_any_element()
}
