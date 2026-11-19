use super::*;
use crate::platform::{SysServiceInfo, SysServiceStartType};

#[derive(Clone, Copy)]
pub(super) enum Action {
    Start,
    Stop,
    Delete,
}

impl Action {
    fn key(self) -> &'static str {
        match self {
            Self::Start => "svc-start",
            Self::Stop => "svc-stop",
            Self::Delete => "svc-delete",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Start => "시작",
            Self::Stop => "중지",
            Self::Delete => "삭제",
        }
    }

    fn blocked(self, elevated: bool, svc: Option<&SysServiceInfo>) -> Option<&'static str> {
        use SysServiceStatus::*;
        if !elevated {
            return Some("관리자 권한으로 다시 실행하세요.");
        }
        let Some(svc) = svc else {
            return Some("서비스를 찾을 수 없습니다. 새로고침하세요.");
        };
        match (&svc.status, self) {
            (Unknown, _) => Some("상태를 확인할 수 없습니다. 새로고침하세요."),
            (StartPending | StopPending, _) => Some("상태 변경 중입니다. 잠시 후 새로고침하세요."),
            (Running, Self::Start) => Some("이미 실행 중입니다."),
            (Paused, Self::Start) => Some("일시 중지된 서비스는 시작할 수 없습니다."),
            (Running | Paused, Self::Delete) => Some("서비스를 중지한 뒤 삭제하세요."),
            (Stopped, Self::Stop) => Some("이미 중지되었습니다."),
            (Stopped, Self::Start) if svc.start_type == SysServiceStartType::Disabled => {
                Some("시작 유형이 사용 안 함입니다.")
            }
            (Stopped, Self::Start | Self::Delete) | (Running | Paused, Self::Stop) => None,
        }
    }

    pub(super) fn cached_reason(self, root: &AppRoot, name: &str) -> Option<&'static str> {
        self.blocked(
            root.platform.is_elevated(),
            root.services.items.iter().find(|item| item.name == name),
        )
    }

    pub(super) fn button(
        self,
        svc: &SysServiceInfo,
        index: usize,
        elevated: bool,
        cx: &Context<AppRoot>,
    ) -> gpui::Stateful<gpui::Div> {
        let name = svc.name.clone();
        let reason = self.blocked(elevated, Some(svc));
        let style = match (reason, self) {
            (Some(_), _) => ButtonStyle::muted(cx),
            (None, Self::Start) => ButtonStyle::neutral(cx),
            (None, Self::Stop | Self::Delete) => ButtonStyle::danger(cx),
        };
        ui::gated_action_button(
            (self.key(), index),
            self.label(),
            ui::Size::Sm,
            style,
            reason,
            cx.listener(move |root, _, window, cx| {
                if self.cached_reason(root, &name).is_some() {
                    cx.notify();
                    return;
                }
                let result = match self {
                    Self::Start => root.platform.start_sys_service(&name),
                    Self::Stop => root.platform.stop_sys_service(&name),
                    Self::Delete => {
                        root.services.pending_delete = Some(name.clone());
                        cx.notify();
                        return;
                    }
                };
                self.finish(result, root, &name, window, cx);
            }),
        )
        .debug_selector(move || format!("{}-{index}", self.key()))
        .w(px(52.0))
    }

    fn finish(
        self,
        result: anyhow::Result<()>,
        root: &mut AppRoot,
        name: &str,
        window: &mut Window,
        cx: &mut Context<AppRoot>,
    ) {
        let label = self.label();
        let message = match result {
            Ok(()) => {
                root.refresh_sys_services();
                format!("서비스 {label}: {name}")
            }
            Err(err) => format!("{label} 실패 ({name}): {err}"),
        };
        root.push_service_log(&message, window, cx);
    }
}

pub(super) fn confirm_delete(
    root: &mut AppRoot,
    name: &str,
    window: &mut Window,
    cx: &mut Context<AppRoot>,
) {
    if root.services.pending_delete.as_deref() != Some(name) {
        return;
    }
    root.services.pending_delete = None;
    if let Some(reason) = Action::Delete.cached_reason(root, name) {
        root.push_service_log(&format!("삭제 차단 ({name}): {reason}"), window, cx);
        return;
    }

    // 확인 배너가 열린 뒤 SCM 상태가 바뀔 수 있다. 행 번호 대신 대상 이름으로 다시 조회한다.
    let status = match root.platform.query_sys_service(name) {
        Ok(current) if current.name == name => current.status,
        _ => SysServiceStatus::Unknown,
    };
    let item = root
        .services
        .items
        .iter_mut()
        .find(|item| item.name == name);
    if let Some(item) = item {
        item.status = status;
    }
    if let Some(reason) = Action::Delete.cached_reason(root, name) {
        root.push_service_log(&format!("삭제 차단 ({name}): {reason}"), window, cx);
        return;
    }
    let result = root.platform.delete_sys_service(name);
    Action::Delete.finish(result, root, name, window, cx);
}
