use super::*;
use crate::platform::{AutoStartAction, TaskState};
use std::sync::atomic::AtomicUsize;

struct MockScheduler {
    admin: bool,
    state: Mutex<TaskState>,
    queries: AtomicUsize,
    calls: Mutex<Vec<AutoStartAction>>,
}

impl Platform for MockScheduler {
    fn is_target_running(&self, _: &str) -> bool {
        false
    }
    fn list_running_processes(&self) -> Result<Vec<String>> {
        Ok(vec![])
    }
    fn find_ad_window(&self, _: &str, _: &str) -> Result<Option<NativeWindowHandle>> {
        Ok(None)
    }
    fn hide_ad(&self, _: NativeWindowHandle) -> Result<()> {
        Ok(())
    }
    fn show_ad(&self, _: NativeWindowHandle) -> Result<()> {
        Ok(())
    }
    fn is_elevated(&self) -> bool {
        self.admin
    }

    fn auto_start_state(&self) -> TaskState {
        self.queries.fetch_add(1, AtomicOrdering::SeqCst);
        self.state.lock().unwrap().clone()
    }

    fn apply_auto_start(&self, action: AutoStartAction) -> Result<()> {
        self.calls.lock().unwrap().push(action);
        *self.state.lock().unwrap() = match action {
            AutoStartAction::Install => TaskState::Ready,
            AutoStartAction::Run => TaskState::Running,
            AutoStartAction::Delete => TaskState::NotInstalled,
        };
        Ok(())
    }
}

fn setup(
    cx: &mut TestAppContext,
    state: TaskState,
    admin: bool,
) -> (Arc<MockScheduler>, &mut gpui::VisualTestContext) {
    initialize_components(cx);
    let mock = Arc::new(MockScheduler {
        admin,
        state: Mutex::new(state),
        queries: AtomicUsize::new(0),
        calls: Mutex::new(vec![]),
    });
    let (_, cx) = cx.add_window_view(|_, _| {
        let mut root = test_app_root(ActivePanel::AutoStart);
        root.platform = mock.clone();
        root
    });
    cx.simulate_resize(size(px(1100.), px(850.)));
    refresh(cx);
    assert_eq!(mock.queries.load(AtomicOrdering::SeqCst), 1);
    (mock, cx)
}

#[gpui::test]
fn auto_start_unknown_retries_then_only_allowed_actions_run_once(cx: &mut TestAppContext) {
    use AutoStartAction::*;
    let (mock, cx) = setup(cx, TaskState::Unknown, true);
    for button in ["task-install", "task-run-now", "task-uninstall"] {
        click_debug_element(cx, button);
    }
    assert!(cx.debug_bounds("task-confirm").is_none());
    assert!(mock.calls.lock().unwrap().is_empty());
    *mock.state.lock().unwrap() = TaskState::NotInstalled;
    click_debug_element(cx, "task-refresh");
    assert_eq!(mock.queries.load(AtomicOrdering::SeqCst), 2);
    for button in ["task-run-now", "task-uninstall"] {
        click_debug_element(cx, button);
    }
    assert!(cx.debug_bounds("task-confirm").is_none());
    assert!(mock.calls.lock().unwrap().is_empty());
    for button in [
        "task-install",
        "task-install",
        "task-run-now",
        "task-run-now",
    ] {
        click_debug_element(cx, button);
    }
    assert_eq!(*mock.calls.lock().unwrap(), [Install, Run]);
}

#[gpui::test]
fn auto_start_disabled_delete_requires_confirmation_and_cancel_calls_nothing(
    cx: &mut TestAppContext,
) {
    let (mock, cx) = setup(cx, TaskState::Disabled, true);
    for button in [
        "task-install",
        "task-run-now",
        "task-uninstall",
        "task-cancel",
    ] {
        click_debug_element(cx, button);
    }
    assert!(mock.calls.lock().unwrap().is_empty());
    click_debug_element(cx, "task-uninstall");
    *mock.state.lock().unwrap() = TaskState::Ready;
    click_debug_element(cx, "task-refresh");
    assert_eq!(mock.queries.load(AtomicOrdering::SeqCst), 2);
    // 이전 프레임의 좌표가 아니라, 재조회 후 일반 버튼이 다시 동작하는지 확인한다.
    click_debug_element(cx, "task-run-now");
    assert_eq!(*mock.calls.lock().unwrap(), [AutoStartAction::Run]);
    for button in ["task-uninstall", "task-confirm", "task-uninstall"] {
        click_debug_element(cx, button);
    }
    assert_eq!(
        *mock.calls.lock().unwrap(),
        [AutoStartAction::Run, AutoStartAction::Delete]
    );
}

#[gpui::test]
fn auto_start_standard_user_cannot_register_or_delete(cx: &mut TestAppContext) {
    let (mock, cx) = setup(cx, TaskState::NotInstalled, false);
    click_debug_element(cx, "task-install");
    *mock.state.lock().unwrap() = TaskState::Ready;
    click_debug_element(cx, "task-refresh");
    click_debug_element(cx, "task-uninstall");
    assert!(cx.debug_bounds("task-confirm").is_none());
    assert!(mock.calls.lock().unwrap().is_empty());
    click_debug_element(cx, "task-run-now");
    assert_eq!(*mock.calls.lock().unwrap(), [AutoStartAction::Run]);
}
