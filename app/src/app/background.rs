//! 백그라운드 스레드.
//!
//! 광고 창 스캔과 파일 동기화를 각각 별도 스레드에서 돌린다.
//! 파일 I/O는 블로킹이므로 스캔 루프와 스레드를 분리했다.
//!
//! 두 루프 모두 UI에는 [`PlatformEvent`] 채널로만 결과를 전달한다.

use std::{
    collections::HashMap,
    future::pending,
    sync::{atomic::Ordering, Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::mpsc::UnboundedSender;

use super::state::{PlatformEvent, ScannerState, SyncSharedState};
use super::AppRoot;
use crate::platform::{NativeWindowHandle, Platform};
use crate::sync::{run_sync_job_with_control, SyncControl, SyncProgress};

/// 진행 상황 이벤트 최소 간격.
///
/// 엔진은 파일마다 보고하지만 그대로 채널에 흘리면 3,000개 폴더에서 이벤트가 폭주해
/// 렌더 루프가 진행 표시만 그리게 된다. 사람이 읽을 수 있는 속도로 제한한다.
const PROGRESS_EVENT_INTERVAL: Duration = Duration::from_millis(120);

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
                    let mut last_hidden: Option<NativeWindowHandle> = None;

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
                            if last_running != Some(false) {
                                let _ = event_tx.send(PlatformEvent::TargetStatusChanged(false));
                                last_running = Some(false);
                            }
                            tokio::time::sleep(sleep_duration).await;
                            continue;
                        }

                        let mut any_running = false;
                        let mut detected_handle: Option<NativeWindowHandle> = None;

                        for target in targets.iter().filter(|t| t.enabled) {
                            if !platform.is_target_running(&target.process_name) {
                                continue;
                            }

                            any_running = true;

                            if let Ok(Some(hwnd)) = platform.find_ad_window(&target.process_name) {
                                detected_handle = Some(hwnd);
                                break;
                            }
                        }

                        if let Some(hwnd) = detected_handle {
                            let _ = platform.hide_ad(hwnd);
                            if last_hidden != Some(hwnd) {
                                let _ = event_tx.send(PlatformEvent::AdBlocked);
                            }
                            last_hidden = Some(hwnd);
                        } else if let Some(hwnd) = last_hidden {
                            let _ = platform.show_ad(hwnd);
                            last_hidden = None;
                        }

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

            loop {
                std::thread::sleep(Duration::from_secs(1));

                let (jobs, manual, cancel) = {
                    let Ok(mut state) = sync_state.lock() else {
                        continue;
                    };
                    let manual = std::mem::take(&mut state.run_now);
                    (state.jobs.clone(), manual, Arc::clone(&state.cancel))
                };

                // 삭제된 작업의 기록은 정리한다.
                last_run.retain(|id, _| jobs.iter().any(|job| &job.id == id));

                for job in jobs.iter() {
                    let manual_requested = manual.contains(&job.id);

                    let due = match last_run.get(&job.id) {
                        Some(prev) => {
                            prev.elapsed() >= Duration::from_secs(job.interval_secs.max(1) as u64)
                        }
                        None => true,
                    };

                    if !manual_requested && (!job.enabled || !due) {
                        continue;
                    }

                    let label = job.label();

                    // 이전 실행에서 남은 중지 요청이 새 작업을 곧바로 끊지 않게 한다.
                    cancel.store(false, Ordering::Relaxed);
                    let _ = event_tx.send(PlatformEvent::SyncStarted {
                        id: job.id.clone(),
                        label: label.clone(),
                    });

                    let outcome = {
                        let progress_tx = event_tx.clone();
                        let progress_id = job.id.clone();
                        let mut last_sent: Option<Instant> = None;
                        let mut reporter = |progress: SyncProgress<'_>| {
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
                        run_sync_job_with_control(job, &mut control)
                    };
                    last_run.insert(job.id.clone(), Instant::now());

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
