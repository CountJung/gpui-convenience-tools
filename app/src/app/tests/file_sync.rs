//! 파일 동기화 패널 조작·진행 표시줄·로그 요약 회귀 테스트.

use super::*;

#[gpui::test]
fn file_sync_unified_page_uses_full_width_and_scrolls_to_last_record(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = test_app_root(ActivePanel::FileSync);
        root.sync_jobs = (0..12)
            .map(|index| SyncJob {
                name: format!("검증 작업 {index}"),
                source: format!(r"D:\validation\source-{index}"),
                target: format!(r"D:\validation\target-{index}"),
                ..SyncJob::default()
            })
            .collect();
        root.selected_sync_job = Some(0);
        root.sync_failures = (0..12)
            .map(|index| SyncFailure {
                path: format!("failure-{index}.txt"),
                reason: "검증용 공유 위반".to_string(),
            })
            .collect();
        root
    });

    cx.simulate_resize(size(
        px(MIN_SUPPORTED_WINDOW_WIDTH),
        px(COMPACT_WINDOW_HEIGHT),
    ));
    refresh(cx);

    let job_card = cx
        .debug_bounds("file-sync-job-list-card")
        .expect("job list card should be rendered");
    let settings_card = cx
        .debug_bounds("file-sync-settings-card")
        .expect("settings card should be rendered");
    let failures_card = cx
        .debug_bounds("file-sync-failures-card")
        .expect("failures card should be rendered");
    let viewport = cx
        .debug_bounds("file-sync-page")
        .expect("File Sync viewport should be rendered");
    assert!(
        (job_card.origin.x - settings_card.origin.x).abs() <= px(1.0)
            && (job_card.size.width - settings_card.size.width).abs() <= px(1.0)
            && (job_card.origin.x - failures_card.origin.x).abs() <= px(1.0)
            && (job_card.size.width - failures_card.size.width).abs() <= px(1.0),
        "all File Sync sections should share one full-width layout: \
         jobs={job_card:?}, settings={settings_card:?}, failures={failures_card:?}"
    );
    assert!(
        job_card.size.width >= viewport.size.width - px(24.0),
        "File Sync sections should fill the viewport width within page padding: \
         viewport={viewport:?}, jobs={job_card:?}"
    );
    let max_scroll = cx.update(|_, app| view.read(app).sync_page_scroll.max_offset().height);
    assert!(
        max_scroll > px(0.0),
        "compact unified File Sync page should overflow"
    );

    wheel_to_end(cx, "file-sync-page", -10000.0);
    let offset = cx.update(|_, app| view.read(app).sync_page_scroll.offset().y);
    assert!(
        offset < px(0.0),
        "wheel input should move the unified page scroll offset"
    );
    assert_inside_viewport(cx, "file-sync-page", "sync-failure-row-11");
}

#[test]
fn running_path_keeps_the_file_name_when_the_path_is_too_long() {
    let mut running = SyncRunning {
        id: "job".to_string(),
        label: "백업".to_string(),
        current_path: String::new(),
        copied: 0,
        skipped: 0,
        failed: 0,
        stopping: false,
    };

    assert_eq!(running.display_path(), "폴더를 검사하는 중…");

    running.current_path = r"docs\report.xlsx".to_string();
    assert_eq!(
        running.display_path(),
        r"docs\report.xlsx",
        "short paths are shown as-is"
    );

    running.current_path = format!(r"{}\분기보고서.xlsx", r"deeply\nested".repeat(20));
    let shortened = running.display_path();
    assert!(
        shortened.chars().count() <= 72,
        "long paths must be bounded, got {} chars",
        shortened.chars().count()
    );
    assert!(
        shortened.starts_with('…') && shortened.ends_with("분기보고서.xlsx"),
        "the head is dropped so the file name survives: {shortened}"
    );
}

#[gpui::test]
fn file_sync_status_bar_stays_visible_at_compact_height_while_running(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = test_app_root(ActivePanel::FileSync);
        root.sync_jobs = (0..12)
            .map(|index| SyncJob {
                name: format!("검증 작업 {index}"),
                ..SyncJob::default()
            })
            .collect();
        root.selected_sync_job = Some(0);
        root.sync_failures = (0..12)
            .map(|index| SyncFailure {
                path: format!("failure-{index}.txt"),
                reason: "검증용 공유 위반".to_string(),
            })
            .collect();
        root
    });

    cx.simulate_resize(size(
        px(MIN_SUPPORTED_WINDOW_WIDTH),
        px(COMPACT_WINDOW_HEIGHT),
    ));
    refresh(cx);

    // 유휴 상태에서도 표시줄은 자리를 지킨다.
    let idle_bar = cx
        .debug_bounds("file-sync-status-bar")
        .expect("idle status bar should be rendered");

    cx.update(|_, app| {
        view.update(app, |root, _| {
            root.sync_running = Some(SyncRunning {
                id: root.sync_jobs[0].id.clone(),
                label: "검증 작업 0".to_string(),
                current_path: r"docs\report\2026\분기보고서.xlsx".to_string(),
                copied: 12,
                skipped: 340,
                failed: 2,
                stopping: false,
            });
        });
    });
    refresh(cx);

    let viewport = cx
        .debug_bounds("file-sync-page")
        .expect("File Sync viewport should be rendered");
    let status_bar = cx
        .debug_bounds("file-sync-status-bar")
        .expect("running status bar should be rendered");
    let current_file = cx
        .debug_bounds("file-sync-current-file")
        .expect("current file line should be rendered");

    assert!(
        (status_bar.origin.x - idle_bar.origin.x).abs() <= px(1.0),
        "status bar should not shift between idle and running: idle={idle_bar:?}, running={status_bar:?}"
    );
    assert!(
        status_bar.origin.y >= viewport.origin.y + viewport.size.height - px(1.0),
        "status bar belongs below the scroll viewport, not inside it: \
         viewport={viewport:?}, bar={status_bar:?}"
    );
    assert!(
        status_bar.origin.y + status_bar.size.height <= px(COMPACT_WINDOW_HEIGHT),
        "status bar must stay on screen at the minimum supported height: {status_bar:?}"
    );
    assert!(
        current_file.size.width > px(0.0) && current_file.size.height > px(0.0),
        "current file line should occupy space: {current_file:?}"
    );

    // 스크롤을 끝까지 내려도 표시줄은 같은 자리에 남는다.
    wheel_to_end(cx, "file-sync-page", -10000.0);
    let after_scroll = cx
        .debug_bounds("file-sync-status-bar")
        .expect("status bar should survive scrolling");
    assert!(
        (after_scroll.origin.y - status_bar.origin.y).abs() <= px(1.0),
        "status bar is fixed, so scrolling must not move it: \
         before={status_bar:?}, after={after_scroll:?}"
    );
}

#[gpui::test]
fn file_sync_stop_button_requests_cancellation_and_clears_pending_queue(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = test_app_root(ActivePanel::FileSync);
        root.sync_jobs = vec![SyncJob::default(), SyncJob::default()];
        root.selected_sync_job = Some(0);
        root
    });

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    refresh(cx);

    // 유휴 상태에서 누르면 중지 요청이 생기지 않는다.
    click_debug_element(cx, "sync-stop");
    cx.update(|_, app| {
        let root = view.read(app);
        let shared = root.sync_state.lock().expect("sync shared state");
        assert!(
            !shared.cancel.load(AtomicOrdering::Relaxed),
            "an idle stop press must not arm cancellation for the next run"
        );
    });

    // 실행 중 + 대기 큐가 있는 상태로 만든 뒤 중지한다.
    let second_id = cx.update(|_, app| {
        view.update(app, |root, _| {
            let running_id = root.sync_jobs[0].id.clone();
            let queued_id = root.sync_jobs[1].id.clone();
            root.sync_running = Some(SyncRunning {
                id: running_id,
                label: "검증 작업".to_string(),
                current_path: "a.txt".to_string(),
                copied: 3,
                skipped: 0,
                failed: 0,
                stopping: false,
            });
            root.sync_state
                .lock()
                .expect("sync shared state")
                .run_now = vec![queued_id.clone()];
            queued_id
        })
    });
    refresh(cx);

    click_debug_element(cx, "sync-stop");

    cx.update(|_, app| {
        let root = view.read(app);
        assert!(
            root.sync_running
                .as_ref()
                .expect("still running until the engine reports back")
                .stopping,
            "the stop press should mark the run as stopping"
        );
        let shared = root.sync_state.lock().expect("sync shared state");
        assert!(
            shared.cancel.load(AtomicOrdering::Relaxed),
            "the engine cancel flag should be armed"
        );
        assert!(
            !shared.run_now.contains(&second_id),
            "stopping should also drop jobs still waiting in the manual queue"
        );
    });
}

#[gpui::test]
fn sync_failures_log_one_summary_line_instead_of_one_entry_per_file(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = test_app_root(ActivePanel::FileSync);
        root.sync_jobs = vec![SyncJob::default()];
        root.selected_sync_job = Some(0);
        root.app_state.log_entries.clear();
        // 이 테스트는 로그 기록만 검증한다. 실패 토스트는 gpui-component `Root`가 필요해
        // 테스트 창에서 띄울 수 없으므로 끈다.
        root.sync_notify_enabled = false;
        root
    });

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    refresh(cx);

    let id = cx.update(|_, app| view.read(app).sync_jobs[0].id.clone());
    let failures: Vec<SyncFailure> = (0..25)
        .map(|index| SyncFailure {
            path: format!("locked-{index}.xlsx"),
            reason: "다른 프로세스가 파일을 사용 중입니다(공유 위반). (code 32)".to_string(),
        })
        .collect();

    cx.update(|window, app| {
        view.update(app, |root, cx| {
            root.handle_sync_finished(
                id.clone(),
                "야간 백업".to_string(),
                SyncOutcome {
                    copied: 4,
                    failures: failures.clone(),
                    truncated_failures: 7,
                    ..SyncOutcome::default()
                },
                window,
                cx,
            );
        });
    });
    refresh(cx);

    cx.update(|_, app| {
        let root = view.read(app);
        let error_lines: Vec<&str> = root
            .app_state
            .log_entries
            .iter()
            .filter(|entry| entry.level == "ERROR")
            .map(|entry| entry.message.as_str())
            .collect();

        assert_eq!(
            error_lines.len(),
            1,
            "32 failures must collapse into a single log line, got: {error_lines:?}"
        );
        assert!(
            error_lines[0].contains("25건") && error_lines[0].contains("7건"),
            "the summary should carry both recorded and omitted counts: {}",
            error_lines[0]
        );
        assert!(
            !error_lines[0].contains("locked-0.xlsx"),
            "per-file paths belong to the failure list, not the log: {}",
            error_lines[0]
        );
        assert_eq!(
            root.sync_failures.len(),
            25,
            "the failure list itself still keeps every recorded entry"
        );
    });
}

#[gpui::test]
fn background_sync_event_wakes_render_without_additional_user_input(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, cx| {
        let mut root = test_app_root(ActivePanel::FileSync);
        root.sync_jobs = vec![SyncJob::default()];
        root.selected_sync_job = Some(0);
        AppRoot::start_event_refresh_loop(cx);
        root
    });

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    refresh(cx);

    let wake_observed = Rc::new(Cell::new(false));
    let wake_observed_for_subscription = Rc::clone(&wake_observed);
    let _wake_subscription = cx.cx.update(|app| {
        app.observe(&view, move |_, _| {
            wake_observed_for_subscription.set(true);
        })
    });

    let (id, tx) = cx.update(|_, app| {
        let root = view.read(app);
        (root.sync_jobs[0].id.clone(), root.event_tx.clone())
    });
    tx.send(PlatformEvent::SyncFinished {
        id: id.clone(),
        label: "백그라운드 검증".to_string(),
        outcome: SyncOutcome {
            copied: 1,
            ..SyncOutcome::default()
        },
    })
    .expect("background event should enter the channel");

    std::thread::sleep(Duration::from_millis(300));
    cx.run_until_parked();
    assert!(
        wake_observed.get(),
        "pending background events should notify the AppRoot without user input"
    );
    refresh(cx);

    cx.update(|_, app| {
        let status = view
            .read(app)
            .sync_status
            .get(&id)
            .cloned()
            .expect("timer notification should trigger render and consume the event");
        assert_eq!(status.summary, "복사 1건, 건너뜀 0건");
        assert!(!status.failed);
    });
}

#[gpui::test]
fn file_sync_run_button_saves_current_inputs_and_queues_selected_job(cx: &mut TestAppContext) {
    initialize_components(cx);
    let (view, cx) = cx.add_window_view(|_, _| {
        let mut root = test_app_root(ActivePanel::FileSync);
        root.sync_jobs = vec![SyncJob::default()];
        root.selected_sync_job = Some(0);
        root
    });

    cx.simulate_resize(size(px(DEFAULT_WINDOW_WIDTH), px(DEFAULT_WINDOW_HEIGHT)));
    refresh(cx);

    cx.update(|window, app| {
        let (name, source, target) = {
            let root = view.read(app);
            (
                root.sync_name_input.clone().expect("name input"),
                root.sync_source_input.clone().expect("source input"),
                root.sync_target_input.clone().expect("target input"),
            )
        };
        name.update(app, |state, cx| state.set_value("즉시 백업", window, cx));
        source.update(app, |state, cx| {
            state.set_value(r"D:\validation\source", window, cx)
        });
        target.update(app, |state, cx| {
            state.set_value(r"E:\validation\target", window, cx)
        });
    });
    refresh(cx);
    click_debug_element(cx, "sync-run-one");

    cx.update(|_, app| {
        let root = view.read(app);
        let job = &root.sync_jobs[0];
        assert_eq!(job.name, "즉시 백업");
        assert_eq!(job.source, r"D:\validation\source");
        assert_eq!(job.target, r"E:\validation\target");
        assert_eq!(
            root.sync_status
                .get(&job.id)
                .expect("queued status")
                .summary,
            "실행 요청됨 — 결과를 기다리는 중입니다."
        );
        let shared = root.sync_state.lock().expect("sync shared state");
        assert_eq!(shared.jobs[0].source, job.source);
        assert_eq!(shared.jobs[0].target, job.target);
        assert_eq!(shared.run_now, vec![job.id.clone()]);
    });
}
