use super::*;

fn row(pid: u32, cpu: f32, memory: u64) -> ProcessRow {
    ProcessRow {
        identity: ProcessIdentity {
            pid,
            start_time: 1,
            path: PathBuf::from("D:/fixture/worker.exe"),
            creation_stamp: Some(123),
        },
        name: format!("fixture-{pid}"),
        cpu_percent: cpu,
        memory_bytes: memory,
        blocked_reason: None,
    }
}

#[test]
fn process_monitor_sort_filter_and_stable_ties() {
    let rows = vec![row(12, 40.0, 100), row(11, 40.0, 300), row(13, 10.0, 200)];
    let cpu = visible_rows(&rows, "", ProcessSort::Cpu, 2);
    assert_eq!(
        cpu.iter().map(|row| row.identity.pid).collect::<Vec<_>>(),
        [11, 12]
    );
    let memory = visible_rows(&rows, "FIXTURE", ProcessSort::Memory, 3);
    assert_eq!(
        memory
            .iter()
            .map(|row| row.identity.pid)
            .collect::<Vec<_>>(),
        [11, 13, 12]
    );
    assert_eq!(
        visible_rows(&rows, "13", ProcessSort::Cpu, 1)[0]
            .identity
            .pid,
        13
    );
    assert_eq!(
        visible_rows(&rows, "worker.exe", ProcessSort::Cpu, 10).len(),
        3
    );
    assert!(visible_rows(&rows, "not-present", ProcessSort::Cpu, 10).is_empty());
}

#[test]
fn process_monitor_permission_and_identity_policy_fails_closed() {
    let identity = row(400_000, 0.0, 0).identity;
    assert!(policy_reason(&identity, "csrss.exe", true, true).is_some());
    assert!(policy_reason(&identity, "fixture.exe", false, true).is_some());
    assert!(policy_reason(&identity, "fixture.exe", true, false).is_some());
    for pid in [0, 1, 4, std::process::id()] {
        let mut protected = identity.clone();
        protected.pid = pid;
        assert!(policy_reason(&protected, "fixture.exe", true, true).is_some());
    }
    let mut unknown = identity.clone();
    unknown.path.clear();
    assert!(policy_reason(&unknown, "fixture.exe", true, true).is_some());
    let mut unknown = identity.clone();
    unknown.start_time = 0;
    assert!(policy_reason(&unknown, "fixture.exe", true, true).is_some());
    #[cfg(target_os = "windows")]
    {
        for sid in ["S-1-5-18", "S-1-5-19", "S-1-5-20", "S-1-5-80-1-2-3-4-5"] {
            assert!(!regular_owner(&sid.parse().unwrap()));
        }
        assert!(policy_reason(&identity, "fixture.exe", true, true).is_none());
        let mut unknown = identity.clone();
        unknown.creation_stamp = None;
        assert!(policy_reason(&unknown, "fixture.exe", true, true).is_some());
        let mut system = identity;
        system.path =
            PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into()))
                .join("notepad.exe");
        assert!(policy_reason(&system, "notepad.exe", true, true).is_some());
    }
}

#[cfg(target_os = "windows")]
#[test]
fn process_monitor_terminates_only_its_own_fixture_and_rejects_reused_identity() {
    use std::os::windows::process::CommandExt;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};
    struct OwnedChild(Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    // A copy of this test executable is the only process ever targeted by this test.
    let mut child = OwnedChild(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "process_monitor::tests::process_fixture_child",
                "--ignored",
                "--nocapture",
            ])
            .env("GPUI_PROCESS_MONITOR_OWNED_FIXTURE", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x0800_0000)
            .spawn()
            .unwrap(),
    );
    let pid = child.0.id();
    let deadline = Instant::now() + Duration::from_secs(5);
    let identity = loop {
        let mut system = System::new();
        let pids = [Pid::from_u32(pid), Pid::from_u32(std::process::id())];
        system.refresh_processes_specifics(ProcessesToUpdate::Some(&pids), refresh_kind());
        if let (Some(process), Some(owner)) = (
            system.process(pids[0]),
            system.process(pids[1]).and_then(Process::user_id),
        ) {
            let row = row_from_process(process, Some(owner), 1);
            if row.blocked_reason.is_none() {
                break row.identity;
            }
        }
        assert!(
            Instant::now() < deadline,
            "owned fixture must become queryable"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut replaced = identity.clone();
    replaced.start_time += 1;
    assert_eq!(terminate(&replaced), Err(ProcessError::IdentityChanged));
    let mut replaced = identity.clone();
    replaced.path.push("replacement.exe");
    assert_eq!(terminate(&replaced), Err(ProcessError::IdentityChanged));
    let mut replaced = identity.clone();
    replaced.creation_stamp = Some(identity.creation_stamp.unwrap() + 1);
    assert_eq!(terminate(&replaced), Err(ProcessError::IdentityChanged));
    assert!(
        child.0.try_wait().unwrap().is_none(),
        "rejected identities must not terminate the fixture"
    );
    terminate(&identity).expect("a regular, owned, noncritical user process must be terminable");
    let deadline = Instant::now() + Duration::from_secs(5);
    while child.0.try_wait().unwrap().is_none() {
        assert!(
            Instant::now() < deadline,
            "termination must reach the owned fixture"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "Only run as an explicitly spawned, owned termination fixture"]
fn process_fixture_child() {
    if std::env::var("GPUI_PROCESS_MONITOR_OWNED_FIXTURE").as_deref() == Ok("1") {
        std::thread::sleep(std::time::Duration::from_secs(30));
    }
}
