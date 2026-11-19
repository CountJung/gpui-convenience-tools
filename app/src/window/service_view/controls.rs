use super::*;
use crate::platform::{AutoStartAction, Platform};
use gpui::{Entity, InteractiveElement};
use std::sync::Arc;

#[derive(Default)]
pub(super) struct Panel {
    pub state: Option<TaskState>,
    confirm_delete: bool,
}

impl Panel {
    pub fn load(platform: Arc<dyn Platform>, cx: &mut Context<Self>) -> Self {
        Self::query(platform, cx);
        Self::default()
    }

    fn query(platform: Arc<dyn Platform>, cx: &mut Context<Self>) {
        let result = cx
            .background_executor()
            .spawn(async move { platform.auto_start_state() });
        cx.spawn(async move |panel, cx| {
            let state = result.await;
            let _ = panel.update(cx, |panel, cx| {
                panel.state = Some(state);
                cx.notify();
            });
        })
        .detach();
    }

    fn reload(&mut self, platform: Arc<dyn Platform>, cx: &mut Context<Self>) {
        if self.state.take().is_some() {
            self.confirm_delete = false;
            Self::query(platform, cx);
            cx.notify();
        }
    }

    fn blocked(&self, action: AutoStartAction, admin: bool) -> Option<&'static str> {
        use AutoStartAction::*;
        match (&self.state, action) {
            (None, _) => Some("상태를 조회하고 있습니다."),
            (Some(TaskState::Unknown), _) => Some("상태를 확인할 수 없습니다. 다시 조회하세요."),
            (Some(TaskState::NotInstalled), Run | Delete) => {
                Some("등록된 자동 시작 작업이 없습니다.")
            }
            (Some(TaskState::Ready | TaskState::Running | TaskState::Disabled), Install) => {
                Some("이미 등록된 작업입니다.")
            }
            (Some(TaskState::Running), Run) => Some("이미 실행 중인 작업입니다."),
            (Some(TaskState::Disabled), Run) => Some("비활성화된 작업은 실행할 수 없습니다."),
            (_, Install | Delete) if !admin => Some("관리자 권한으로 앱을 다시 실행하세요."),
            _ => None,
        }
    }
}

pub(super) fn refresh_button(panel: Entity<Panel>, cx: &mut Context<AppRoot>) -> AnyElement {
    ui::gated_action_button(
        "task-refresh",
        "↺ 재조회",
        ui::Size::Md,
        ButtonStyle::secondary(cx),
        panel
            .read(cx)
            .state
            .is_none()
            .then_some("상태를 조회하고 있습니다."),
        cx.listener(move |this, _, _, cx| {
            panel.update(cx, |panel, cx| panel.reload(this.platform.clone(), cx));
        }),
    )
    .debug_selector(|| "task-refresh".into())
    .into_any_element()
}

pub(super) fn buttons(panel: Entity<Panel>, admin: bool, cx: &mut Context<AppRoot>) -> AnyElement {
    use AutoStartAction::*;
    let mut row = h_flex().gap_3().flex_wrap();
    let confirming = panel.read(cx).confirm_delete;
    let actions: &[(_, _, _)] = if confirming {
        &[("task-confirm", "삭제 확인", Delete)]
    } else {
        &[
            ("task-install", "등록", Install),
            ("task-run-now", "지금 실행", Run),
            ("task-uninstall", "삭제", Delete),
        ]
    };
    for &(id, label, action) in actions {
        let panel = panel.clone();
        let blocked = panel.read(cx).blocked(action, admin);
        let style = if blocked.is_some() {
            ButtonStyle::muted(cx)
        } else if action == Delete {
            ButtonStyle::danger(cx)
        } else {
            ButtonStyle::primary(cx)
        };
        row = row.child(
            ui::gated_action_button(
                id,
                label,
                ui::Size::Lg,
                style,
                blocked,
                cx.listener(move |this, _, window, cx| {
                    let message = panel.update(cx, |panel, pcx| {
                        if panel.confirm_delete != confirming
                            || panel.blocked(action, admin).is_some()
                        {
                            return None;
                        }
                        if action == Delete && !confirming {
                            panel.confirm_delete = true;
                            pcx.notify();
                            return None;
                        }
                        let message = match this.platform.apply_auto_start(action) {
                            Ok(()) => match action {
                                Install => "자동 시작 작업을 등록했습니다.",
                                Run => "자동 시작 작업의 즉시 실행을 요청했습니다.",
                                Delete => "자동 시작 작업을 삭제했습니다.",
                            }
                            .to_owned(),
                            Err(e) => format!("자동 시작 {label} 실패: {e}"),
                        };
                        panel.reload(this.platform.clone(), pcx);
                        Some(message)
                    });
                    if let Some(message) = message {
                        this.push_service_log(&message, window, cx);
                    }
                }),
            )
            .debug_selector(move || id.into()),
        );
    }
    if confirming {
        row = row.child(
            ui::action_button(
                "task-cancel",
                "취소",
                ui::Size::Lg,
                ButtonStyle::secondary(cx),
                cx.listener(move |_, _, _, cx| {
                    panel.update(cx, |panel, cx| {
                        panel.confirm_delete = false;
                        cx.notify();
                    });
                }),
            )
            .debug_selector(|| "task-cancel".into()),
        );
        return v_flex()
            .gap_2()
            .child("로그온 자동 시작 작업을 삭제할까요?")
            .child(row)
            .into_any_element();
    }
    row.into_any_element()
}
