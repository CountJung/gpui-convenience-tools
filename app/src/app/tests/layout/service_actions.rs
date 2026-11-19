//! UX-007: GPUI 입력에서 모의 서비스 플랫폼까지의 호출 순서와 대상.
use super::*;
use std::sync::atomic::AtomicBool;

const NAME: &str = "synthetic-ux007";

#[derive(Debug, PartialEq)]
enum Call {
    List,
    Start(String),
    Stop(String),
    Query(String),
    Delete(String),
}

struct Services {
    rows: Mutex<Vec<SysServiceInfo>>,
    admin: AtomicBool,
    fail_list: AtomicBool,
    fail_delete: bool,
    calls: Mutex<Vec<Call>>,
}

impl Services {
    fn new(rows: Vec<SysServiceInfo>, admin: bool) -> Self {
        Self {
            rows: Mutex::new(rows),
            admin: AtomicBool::new(admin),
            fail_list: AtomicBool::new(false),
            fail_delete: false,
            calls: Mutex::default(),
        }
    }

    fn take_calls(&self) -> Vec<Call> {
        std::mem::take(&mut *self.calls.lock().unwrap())
    }
}

impl Platform for Services {
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
        self.admin.load(AtomicOrdering::Relaxed)
    }
    fn list_sys_services(&self) -> Result<Vec<SysServiceInfo>> {
        self.calls.lock().unwrap().push(Call::List);
        anyhow::ensure!(
            !self.fail_list.load(AtomicOrdering::Relaxed),
            "mock list error"
        );
        Ok(self.rows.lock().unwrap().clone())
    }
    fn query_sys_service(&self, name: &str) -> Result<SysServiceInfo> {
        self.calls.lock().unwrap().push(Call::Query(name.into()));
        self.rows
            .lock()
            .unwrap()
            .iter()
            .find(|r| r.name == name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("mock service missing"))
    }
    fn start_sys_service(&self, name: &str) -> Result<()> {
        self.calls.lock().unwrap().push(Call::Start(name.into()));
        self.rows
            .lock()
            .unwrap()
            .iter_mut()
            .find(|r| r.name == name)
            .unwrap()
            .status = SysServiceStatus::Running;
        Ok(())
    }
    fn stop_sys_service(&self, name: &str) -> Result<()> {
        self.calls.lock().unwrap().push(Call::Stop(name.into()));
        self.rows
            .lock()
            .unwrap()
            .iter_mut()
            .find(|r| r.name == name)
            .unwrap()
            .status = SysServiceStatus::Stopped;
        Ok(())
    }
    fn delete_sys_service(&self, name: &str) -> Result<()> {
        self.calls.lock().unwrap().push(Call::Delete(name.into()));
        anyhow::ensure!(!self.fail_delete, "mock delete error");
        self.rows.lock().unwrap().retain(|r| r.name != name);
        Ok(())
    }
}

pub(super) fn platform_with_delete_failure(rows: &[SysServiceInfo]) -> Arc<dyn Platform> {
    Arc::new(Services {
        fail_delete: true,
        ..Services::new(rows.to_vec(), true)
    })
}

fn panel(
    cx: &mut TestAppContext,
    status: SysServiceStatus,
    admin: bool,
) -> (
    Arc<Services>,
    gpui::Entity<AppRoot>,
    &mut gpui::VisualTestContext,
) {
    let rows = vec![SysServiceInfo {
        name: NAME.into(),
        display_name: "합성 서비스".into(),
        status,
        start_type: SysServiceStartType::Manual,
    }];
    let mock = Arc::new(Services::new(rows, admin));
    let platform = mock.clone();
    let (view, cx) = cx.add_window_view(move |_, _| {
        let mut root = test_app_root(ActivePanel::Services);
        root.platform = platform;
        root.refresh_sys_services();
        root
    });
    refresh(cx);
    assert_eq!(mock.take_calls(), [Call::List]);
    (mock, view, cx)
}

#[gpui::test]
fn service_actions_block_by_permission_and_status(cx: &mut TestAppContext) {
    use SysServiceStatus::*;
    initialize_components(cx);
    for (admin, status, blocked) in [
        (
            false,
            Stopped,
            &["svc-start-0", "svc-stop-0", "svc-delete-0"][..],
        ),
        (false, Running, &["svc-stop-0"][..]),
        (true, Running, &["svc-start-0", "svc-delete-0"][..]),
        (true, Stopped, &["svc-stop-0"][..]),
        (true, Paused, &["svc-start-0", "svc-delete-0"][..]),
        (
            true,
            StartPending,
            &["svc-start-0", "svc-stop-0", "svc-delete-0"][..],
        ),
        (
            true,
            StopPending,
            &["svc-start-0", "svc-stop-0", "svc-delete-0"][..],
        ),
        (
            true,
            Unknown,
            &["svc-start-0", "svc-stop-0", "svc-delete-0"][..],
        ),
    ] {
        let (mock, view, cx) = panel(cx, status, admin);
        for id in blocked {
            click_debug_element(cx, id);
        }
        assert!(view.read_with(cx, |root, _| root.services.pending_delete.is_none()));
        assert_eq!(mock.take_calls(), []);
    }
}

#[gpui::test]
fn service_actions_allow_commands_confirm_once_and_cancel(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (mock, view, cx) = panel(cx, SysServiceStatus::Stopped, true);
    click_debug_element(cx, "svc-start-0");
    click_debug_element(cx, "svc-stop-0");
    assert_eq!(
        mock.take_calls(),
        [
            Call::Start(NAME.into()),
            Call::List,
            Call::Stop(NAME.into()),
            Call::List
        ]
    );
    click_debug_element(cx, "svc-delete-0");
    click_debug_element(cx, "svc-cancel");
    assert_eq!(mock.take_calls(), []);
    assert!(view.read_with(cx, |root, _| root.services.pending_delete.is_none()));
    click_debug_element(cx, "svc-delete-0");
    click_debug_element(cx, "svc-confirm");
    assert_eq!(
        mock.take_calls(),
        [
            Call::Query(NAME.into()),
            Call::Delete(NAME.into()),
            Call::List
        ]
    );
    assert!(view.read_with(cx, |root, _| root.services.pending_delete.is_none()));
    assert!(view.read_with(cx, |root, _| root.services.items.is_empty()));
}

#[gpui::test]
fn service_delete_rechecks_live_state_and_permission(cx: &mut TestAppContext) {
    use SysServiceStatus::*;
    initialize_components(cx);
    for current in [Some(Running), Some(StopPending), Some(Unknown), None] {
        let (mock, view, cx) = panel(cx, Stopped, true);
        click_debug_element(cx, "svc-delete-0");
        if let Some(status) = current.clone() {
            mock.rows.lock().unwrap()[0].status = status;
        } else {
            mock.rows.lock().unwrap().clear();
        }
        click_debug_element(cx, "svc-confirm");
        assert_eq!(mock.take_calls(), [Call::Query(NAME.into())]);
        view.read_with(cx, |root, _| {
            assert_eq!(root.services.items[0].status, current.unwrap_or(Unknown));
            assert!(root.services.pending_delete.is_none());
        });
        click_debug_element(cx, "svc-delete-0");
        assert!(view.read_with(cx, |root, _| root.services.pending_delete.is_none()));
    }
    let (mock, _, cx) = panel(cx, Stopped, true);
    click_debug_element(cx, "svc-delete-0");
    mock.admin.store(false, AtomicOrdering::Relaxed);
    click_debug_element(cx, "svc-confirm");
    assert_eq!(mock.take_calls(), []);
}

#[gpui::test]
fn service_refresh_recovers_without_render_time_queries(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (mock, view, cx) = panel(cx, SysServiceStatus::Unknown, true);
    mock.fail_list.store(true, AtomicOrdering::Relaxed);
    click_debug_element(cx, "svc-refresh");
    refresh(cx);
    assert!(view.read_with(cx, |root, _| root.services.list_failed
        && root.services.items.is_empty()));
    assert_eq!(mock.take_calls(), [Call::List]);
    mock.fail_list.store(false, AtomicOrdering::Relaxed);
    mock.rows.lock().unwrap()[0].status = SysServiceStatus::Stopped;
    click_debug_element(cx, "svc-refresh");
    refresh(cx);
    assert_eq!(mock.take_calls(), [Call::List]);
    click_debug_element(cx, "svc-start-0");
    assert_eq!(mock.take_calls(), [Call::Start(NAME.into()), Call::List]);
}
