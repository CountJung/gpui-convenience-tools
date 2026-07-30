//! 대시보드 패널.
//!
//! 현재 플랫폼에서 실제로 동작하는 기능의 상태만 요약하고 최근 활동을 보여준다.
//! 이 패널은 설정이 없어 스플리터를 쓰지 않는다.

use gpui::{div, AnyElement, Context, Div, IntoElement, ParentElement, Styled};
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
            activity = activity.child(
                h_flex()
                    .gap_2()
                    .child(ui::log_level_label(&entry.level, cx))
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
    let (svc_label, svc_tone) = if root.app_state.is_active {
        ("광고 차단: 동작 중", ui::Tone::Success)
    } else {
        ("광고 차단: 중지됨", ui::Tone::Warning)
    };

    let (tgt_label, tgt_tone) = if root.app_state.is_target_running {
        ("타겟: 실행 중", ui::Tone::Success)
    } else {
        ("타겟: 미실행", ui::Tone::Muted)
    };

    let active_targets = root.app_state.targets.iter().filter(|t| t.enabled).count();

    summary_card(
        v_flex()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .child(ui::badge(svc_label, svc_tone, ui::Size::Md, cx))
                    .child(ui::badge(tgt_label, tgt_tone, ui::Size::Md, cx))
                    .child(sync_badge(root, cx)),
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
            .child(h_flex().gap_2().child(sync_badge(root, cx)))
            .child(h_flex().gap_3().children(sync_tiles(root, cx)))
            .into_any_element(),
        cx,
    )
}

/// 파일 동기화가 지금 무엇을 하고 있는지 한 줄로 알려주는 배지.
///
/// 진행 상황 표시줄은 파일 동기화 패널에만 있어서, 다른 화면에 있으면 동기화가 도는지조차
/// 알 수 없었다. 대시보드에서 바로 보이게 한다.
fn sync_badge(root: &AppRoot, cx: &Context<AppRoot>) -> Div {
    let (label, tone) = match (&root.sync_running, root.sync_enabled) {
        (Some(running), _) => (format!("동기화 중: {}", running.label), ui::Tone::Info),
        (None, true) => ("파일 동기화: 대기 중".to_string(), ui::Tone::Success),
        (None, false) => ("파일 동기화: 꺼짐".to_string(), ui::Tone::Muted),
    };
    ui::badge(label, tone, ui::Size::Md, cx)
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
