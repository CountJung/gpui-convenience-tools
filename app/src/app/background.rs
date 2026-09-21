//! 백그라운드 스레드.
//!
//! 광고 창 스캔과 파일 동기화를 각각 별도 스레드에서 돌린다.
//! 파일 I/O는 블로킹이므로 스캔 루프와 스레드를 분리했다.
//!
//! 두 루프 모두 UI에는 [`PlatformEvent`] 채널로만 결과를 전달한다.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    future::pending,
    sync::{atomic::Ordering, Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::mpsc::UnboundedSender;

use super::state::{PlatformEvent, ScannerState, SyncSharedState};
use super::AppRoot;
use super::watch::WatchManager;
use crate::platform::{AdWindowSnapshot, NativeWindowHandle, Platform};
use crate::sync::{run_sync_job_with_control, SyncControl, SyncProgress};

/// 진행 상황 이벤트 최소 간격.
///
/// 엔진은 파일마다 보고하지만 그대로 채널에 흘리면 3,000개 폴더에서 이벤트가 폭주해
/// 렌더 루프가 진행 표시만 그리게 된다. 사람이 읽을 수 있는 속도로 제한한다.
const PROGRESS_EVENT_INTERVAL: Duration = Duration::from_millis(120);

/// 이어서 시작할 지점을 config에 적어 두는 최소 간격.
///
/// 앱이 트레이에서 강제 종료되거나 로그아웃으로 끊기면 완료 시점 기록만으로는 아무것도
/// 남지 않는다. 실행 중에도 위치를 남겨야 다음 실행이 처음부터 다시 돌지 않는다.
/// 대신 config.json 쓰기이므로 간격을 넉넉히 둔다.
const CURSOR_PERSIST_INTERVAL: Duration = Duration::from_secs(5);

/// 실행 위치와 마지막 실행 시각을 config의 해당 작업에만 반영한다.
///
/// UI 스냅샷 전체를 저장하는 경로와 분리해, 사용자가 설정을 만지는 것과 무관하게
/// 진행 상황만 갱신한다.
fn persist_job_progress(id: &str, cursor: Option<String>, last_run_unix: Option<u64>) {
    let id = id.to_string();
    if let Err(err) = crate::config::update_config(move |cfg| {
        let Some(job) = cfg.sync_jobs.iter_mut().find(|job| job.id == id) else {
            return;
        };
        job.resume_cursor = cursor;
        if let Some(finished_at) = last_run_unix {
            job.last_run_unix = Some(finished_at);
        }
    }) {
        log::warn!("동기화 진행 상황 저장 실패: {err}");
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// 저장된 유닉스 초를 이번 프로세스의 [`Instant`]로 되돌린다.
///
/// 앱을 껐다 켠 뒤에도 주기를 이어가려면 "마지막 실행이 얼마나 지났는지"가 필요하다.
/// 시계가 뒤로 갔거나 값이 미래면 `None`을 돌려 즉시 실행되게 둔다.
fn instant_from_unix(unix_secs: u64) -> Option<Instant> {
    let elapsed = now_unix().checked_sub(unix_secs)?;
    Instant::now().checked_sub(Duration::from_secs(elapsed))
}

/// 종료되었거나 기능이 꺼진 광고 창 스냅샷을 복원한다.
///
/// 복원이 실패해도 같은 잘못된 HWND를 반복 조작하지 않도록 스냅샷을 먼저 제거한다.
fn restore_collapsed_ad_windows(
    platform: &dyn Platform,
    collapsed_windows: &mut HashMap<NativeWindowHandle, AdWindowSnapshot>,
    force: bool,
) {
    let handles: Vec<_> = collapsed_windows
        .iter()
        .filter_map(|(handle, snapshot)| {
            (force || !platform.is_process_id_running(snapshot.process_id)).then_some(*handle)
        })
        .collect();

    for handle in handles {
        let Some(snapshot) = collapsed_windows.remove(&handle) else {
            continue;
        };

        if let Err(err) = platform.restore_ad_window_state(&snapshot) {
            log::warn!("광고 창 원래 상태 복원 실패 (HWND {:?}): {err}", handle);
        }
    }
}

/// 닫히거나 재생성되어 더 이상 유효하지 않은 HWND의 스냅샷을 제거한다.
fn discard_dead_ad_window_snapshots(
    platform: &dyn Platform,
    collapsed_windows: &mut HashMap<NativeWindowHandle, AdWindowSnapshot>,
) {
    let dead_handles: Vec<_> = collapsed_windows
        .keys()
        .copied()
        .filter(|handle| !platform.is_ad_window_alive(*handle))
        .collect();

    for handle in dead_handles {
        collapsed_windows.remove(&handle);
        log::info!("광고 창이 닫혀 기존 상태 스냅샷을 폐기했습니다 (HWND {:?})", handle);
    }
}

impl AppRoot {
    // ─────────────────────────────────────────────
    // 백그라운드 루프
    // ─────────────────────────────────────────────

    pub(super) fn spawn_platform_loop(
        platform: Arc<dyn Platform>,
        event_tx: UnboundedSender<PlatformEvent>,
        scanner_state: Arc<Mutex<ScannerState>>,
    ) {
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build();

            let Ok(runtime) = runtime else {
                return;
            };

            runtime.block_on(async move {
                tokio::spawn(async move {
                    let mut last_running: Option<bool> = None;
                    let mut collapsed_windows: HashMap<NativeWindowHandle, AdWindowSnapshot> =
                        HashMap::new();

                    loop {
                        let snapshot = scanner_state
                            .lock()
                            .ok()
                            .map(|s| (s.service_enabled, s.targets.clone(), s.scan_interval_secs));

                        let Some((service_enabled, targets, interval_secs)) = snapshot else {
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            continue;
                        };

                        let sleep_duration = Duration::from_secs(interval_secs.max(1) as u64);

                        if !service_enabled {
                            restore_collapsed_ad_windows(
                                platform.as_ref(),
                                &mut collapsed_windows,
                                true,
                            );
                            if last_running != Some(false) {
                                let _ = event_tx.send(PlatformEvent::TargetStatusChanged(false));
                                last_running = Some(false);
                            }
                            tokio::time::sleep(sleep_duration).await;
                            continue;
                        }

                        let mut any_running = false;
                        let mut candidate_windows = HashSet::new();

                        for target in targets.iter().filter(|t| t.enabled) {
                            if !platform.is_target_running(&target.process_name) {
                                continue;
                            }

                            any_running = true;

                            match platform
                                .find_ad_windows(&target.process_name, &target.ad_window_class)
                            {
                                Ok(windows) => candidate_windows.extend(windows),
                                Err(err) => {
                                    log::warn!("광고 창 후보 열거 실패: {err}");
                                }
                            }
                        }

                        discard_dead_ad_window_snapshots(
                            platform.as_ref(),
                            &mut collapsed_windows,
                        );

                        for hwnd in candidate_windows {
                            let current_process_id = match platform.ad_window_process_id(hwnd) {
                                Ok(process_id) => process_id,
                                Err(err) => {
                                    log::warn!("광고 창 프로세스 ID 조회 실패: {err}");
                                    continue;
                                }
                            };

                            if let Some(snapshot) = collapsed_windows.get(&hwnd) {
                                if snapshot.process_id != current_process_id {
                                    log::warn!(
                                        "HWND가 다른 프로세스로 재사용되어 기존 광고 창 상태를 폐기합니다 (HWND {:?})",
                                        hwnd
                                    );
                                    collapsed_windows.remove(&hwnd);
                                } else {
                                    match platform.is_ad_window_collapsed(hwnd) {
                                        Ok(true) => continue,
                                        Ok(false) => {
                                            log::info!(
                                                "광고 창의 수동 상태 변경을 감지해 다시 0×0으로 축소합니다 (HWND {:?})",
                                                hwnd
                                            );
                                        }
                                        Err(err) => {
                                            log::warn!("광고 창 축소 상태 조회 실패: {err}");
                                            continue;
                                        }
                                    }

                                    if let Err(err) = platform.collapse_ad(hwnd) {
                                        log::warn!("광고 창 재축소 실패: {err}");
                                    }
                                    continue;
                                }
                            }

                            match platform.capture_ad_window_state(hwnd) {
                                Ok(snapshot) => match platform.collapse_ad(hwnd) {
                                    Ok(()) => {
                                        collapsed_windows.insert(hwnd, snapshot);
                                        let _ = event_tx.send(PlatformEvent::AdBlocked);
                                    }
                                    Err(err) => {
                                        log::warn!("광고 창 0×0 축소 실패: {err}");
                                    }
                                },
                                Err(err) => {
                                    log::warn!("광고 창 원래 상태 캡처 실패: {err}");
                                }
                            }
                        }

                        restore_collapsed_ad_windows(
                            platform.as_ref(),
                            &mut collapsed_windows,
                            !any_running,
                        );

                        if last_running != Some(any_running) {
                            let _ = event_tx.send(PlatformEvent::TargetStatusChanged(any_running));
                            last_running = Some(any_running);
                        }

                        tokio::time::sleep(sleep_duration).await;
                    }
                });

                pending::<()>().await;
            });
        });
    }

    /// 동기화 전용 백그라운드 스레드.
    ///
    /// 1초마다 깨어나 (1) 수동 실행 요청과 (2) 주기가 도래한 자동 작업을 처리한다.
    /// 파일 I/O는 블로킹이므로 광고 스캔 루프와 스레드를 분리했다.
    pub(super) fn spawn_sync_loop(
        event_tx: UnboundedSender<PlatformEvent>,
        sync_state: Arc<Mutex<SyncSharedState>>,
    ) {
        std::thread::spawn(move || {
            // 작업 인덱스는 추가·삭제로 밀리므로 실행 주기는 반드시 ID로 추적한다.
            let mut last_run: HashMap<String, Instant> = HashMap::new();
            let mut watch_manager = WatchManager::new();

            // 앱을 껐다 켰다고 해서 주기를 처음부터 세면, 시작할 때마다 원본 전체를 다시
            // 훑어 "건너뜀"만 쌓인다. 저장해 둔 마지막 실행 시각으로 주기를 이어받는다.
            if let Ok(state) = sync_state.lock() {
                for job in &state.jobs {
                    if let Some(finished_at) = job.last_run_unix {
                        if let Some(instant) = instant_from_unix(finished_at) {
                            last_run.insert(job.id.clone(), instant);
                        }
                    }
                }
            }

            loop {
                std::thread::sleep(Duration::from_secs(1));

                let (jobs, manual, cancel, auto_enabled) = {
                    let Ok(mut state) = sync_state.lock() else {
                        continue;
                    };
                    let manual = std::mem::take(&mut state.run_now);
                    (
                        state.jobs.clone(),
                        manual,
                        Arc::clone(&state.cancel),
                        state.auto_enabled,
                    )
                };

                // 삭제된 작업의 기록은 정리한다.
                last_run.retain(|id, _| jobs.iter().any(|job| &job.id == id));

                for failure in watch_manager.poll(&jobs) {
                    log::warn!(
                        "실시간 감시를 시작하지 못해 주기 모드로 전환합니다 ({}) : {}",
                        failure.id,
                        failure.reason
                    );
                    let _ = event_tx.send(PlatformEvent::SyncWatchFallback {
                        id: failure.id,
                        reason: failure.reason,
                    });
                }

                for job in jobs.iter() {
                    let manual_requested = manual.contains(&job.id);

                    let due = if job.watch_mode == crate::config::WatchMode::Realtime {
                        watch_manager.take_ready(&job.id, Instant::now())
                    } else {
                        match last_run.get(&job.id) {
                            Some(prev) => {
                                prev.elapsed()
                                    >= Duration::from_secs(job.interval_secs.max(1) as u64)
                            }
                            None => true,
                        }
                    };

                    // 전역 스위치는 자동 실행만 막는다. 사용자가 직접 누른 요청까지 막으면
                    // 버튼이 아무 반응 없이 무시되는 것처럼 보인다.
                    if !manual_requested && (!auto_enabled || !job.enabled || !due) {
                        continue;
                    }

                    let label = job.label();
                    let started_at_unix = now_unix();
                    let resume_from = sync_state
                        .lock()
                        .ok()
                        .and_then(|state| state.cursors.get(&job.id).cloned());

                    // 이전 실행에서 남은 중지 요청이 새 작업을 곧바로 끊지 않게 한다.
                    cancel.store(false, Ordering::Relaxed);
                    let _ = event_tx.send(PlatformEvent::SyncStarted {
                        id: job.id.clone(),
                        label: label.clone(),
                    });

                    // 진행 위치는 콜백 안에서 갱신하고 실행이 끝난 뒤 읽어야 하므로
                    // 클로저가 빌려도 되는 셀에 담는다.
                    let cursor = RefCell::new(String::new());
                    let outcome = {
                        let progress_tx = event_tx.clone();
                        let progress_id = job.id.clone();
                        let mut last_sent: Option<Instant> = None;
                        let mut last_persisted: Option<Instant> = None;
                        let mut reporter = |progress: SyncProgress<'_>| {
                            *cursor.borrow_mut() = progress.current_path.to_string();

                            if last_persisted
                                .is_none_or(|at| at.elapsed() >= CURSOR_PERSIST_INTERVAL)
                            {
                                last_persisted = Some(Instant::now());
                                persist_job_progress(
                                    &progress_id,
                                    Some(progress.current_path.to_string()),
                                    None,
                                );
                            }

                            if last_sent
                                .is_some_and(|at| at.elapsed() < PROGRESS_EVENT_INTERVAL)
                            {
                                return;
                            }
                            last_sent = Some(Instant::now());
                            let _ = progress_tx.send(PlatformEvent::SyncProgress {
                                id: progress_id.clone(),
                                current_path: progress.current_path.to_string(),
                                copied: progress.copied,
                                skipped: progress.skipped,
                                failed: progress.failed,
                            });
                        };

                        let mut control = SyncControl::new()
                            .cancel_flag(&cancel)
                            .on_progress(&mut reporter);
                        if let Some(from) = resume_from.as_deref() {
                            control = control.resume_from(from);
                        }
                        run_sync_job_with_control(job, &mut control)
                    };
                    last_run.insert(job.id.clone(), Instant::now());

                    // 끊긴 실행만 위치를 남긴다. 끝까지 돈 실행은 다음에 원본 전체를
                    // 다시 확인해야 하므로 위치를 지운다.
                    let next_cursor = outcome
                        .cancelled
                        .then(|| cursor.into_inner())
                        .filter(|path| !path.is_empty());
                    if let Ok(mut state) = sync_state.lock() {
                        match next_cursor.clone() {
                            Some(path) => state.cursors.insert(job.id.clone(), path),
                            None => state.cursors.remove(&job.id),
                        };
                    }
                    persist_job_progress(&job.id, next_cursor, Some(now_unix()));

                    let finished_at_unix = now_unix();
                    let history = crate::sync_history::SyncHistoryEntry::from_outcome(
                        &job.id,
                        &label,
                        started_at_unix,
                        finished_at_unix,
                        &outcome,
                    );
                    if let Err(err) = crate::sync_history::append(&history) {
                        log::warn!("동기화 이력 저장 실패: {err:#}");
                    }

                    // 파일 단위 기록은 남기지 않는다. 개별 실패 사유는 UI의 실패 목록이
                    // 소유하고, 로그에는 실행 단위의 중요한 결과만 남긴다.
                    if outcome.cancelled {
                        log::info!("동기화 '{label}' 중지됨: {}", outcome.summary());
                    } else if outcome.has_failures() {
                        log::warn!("동기화 '{label}' 완료(실패 포함): {}", outcome.summary());
                    } else if outcome.copied > 0 || outcome.deleted > 0 {
                        log::info!("동기화 '{label}' 완료: {}", outcome.summary());
                    }

                    let cancelled = outcome.cancelled;
                    let _ = event_tx.send(PlatformEvent::SyncFinished {
                        id: job.id.clone(),
                        label,
                        outcome,
                    });

                    // 중지는 이번 틱의 남은 작업까지 멈춘다. 한 작업만 끊고 다음 작업을
                    // 이어서 돌리면 '중지'를 누른 사용자의 기대와 어긋난다.
                    if cancelled {
                        if let Ok(mut state) = sync_state.lock() {
                            state.run_now.clear();
                        }
                        cancel.store(false, Ordering::Relaxed);
                        break;
                    }
                }
            }
        });
    }
}
