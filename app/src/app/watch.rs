//! 파일 동기화 실시간 감시와 변경 이벤트 디바운스.
//!
//! 감시자는 동기화 백그라운드 스레드에서만 소유한다. 이벤트 콜백은 파일 I/O를 하지 않고
//! 작업 ID만 채널에 전달하며, 실제 동기화는 기존 엔진을 재사용해 순차적으로 실행한다.

use crate::config::{SyncJob, WatchMode};
use notify::{recommended_watcher, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

/// 연속된 파일 변경을 하나의 실행으로 묶는 대기 시간.
pub(crate) const REALTIME_DEBOUNCE: Duration = Duration::from_secs(2);

#[derive(Debug)]
enum WatchNotification {
    Changed(String),
    Failed { id: String, reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WatchFailure {
    pub(crate) id: String,
    pub(crate) reason: String,
}

#[derive(Default)]
struct DebounceState {
    pending_since: HashMap<String, Instant>,
}

impl DebounceState {
    fn mark(&mut self, id: String, at: Instant) {
        self.pending_since.insert(id, at);
    }

    fn clear(&mut self, id: &str) {
        self.pending_since.remove(id);
    }

    fn ready(&mut self, id: &str, now: Instant) -> bool {
        let Some(since) = self.pending_since.get(id).copied() else {
            return false;
        };
        if now.duration_since(since) < REALTIME_DEBOUNCE {
            return false;
        }
        self.pending_since.remove(id);
        true
    }
}

/// 실시간 작업의 watcher와 디바운스 상태를 관리한다.
pub(crate) struct WatchManager {
    notifications_tx: Sender<WatchNotification>,
    notifications_rx: Receiver<WatchNotification>,
    watchers: HashMap<String, (PathBuf, RecommendedWatcher)>,
    modes: HashMap<String, WatchMode>,
    failed: HashSet<String>,
    debounce: DebounceState,
}

impl WatchManager {
    pub(crate) fn new() -> Self {
        let (notifications_tx, notifications_rx) = mpsc::channel();
        Self {
            notifications_tx,
            notifications_rx,
            watchers: HashMap::new(),
            modes: HashMap::new(),
            failed: HashSet::new(),
            debounce: DebounceState::default(),
        }
    }

    /// watcher를 작업 목록에 맞추고, 이번 틱의 오류를 반환한다.
    pub(crate) fn poll(&mut self, jobs: &[SyncJob]) -> Vec<WatchFailure> {
        let mut failures = self.drain_notifications();
        self.reconcile(jobs, &mut failures);
        failures
    }

    /// 디바운스가 끝난 실시간 작업인지 확인하고 해당 대기 상태를 소비한다.
    pub(crate) fn take_ready(&mut self, id: &str, now: Instant) -> bool {
        self.debounce.ready(id, now)
    }

    fn drain_notifications(&mut self) -> Vec<WatchFailure> {
        let mut failures = Vec::new();
        while let Ok(notification) = self.notifications_rx.try_recv() {
            match notification {
                WatchNotification::Changed(id) => {
                    if self.watchers.contains_key(&id) {
                        self.debounce.mark(id, Instant::now());
                    }
                }
                WatchNotification::Failed { id, reason } => {
                    self.remove_watcher(&id);
                    self.debounce.clear(&id);
                    if self.failed.insert(id.clone()) {
                        failures.push(WatchFailure { id, reason });
                    }
                }
            }
        }
        failures
    }

    fn reconcile(&mut self, jobs: &[SyncJob], failures: &mut Vec<WatchFailure>) {
        let desired: HashMap<String, (PathBuf, WatchMode)> = jobs
            .iter()
            .map(|job| (job.id.clone(), (PathBuf::from(&job.source), job.watch_mode)))
            .collect();

        let stale_ids: Vec<String> = self
            .modes
            .keys()
            .filter(|id| !desired.contains_key(*id))
            .cloned()
            .collect();
        for id in stale_ids {
            self.reset_job(&id);
        }

        for (id, (path, mode)) in &desired {
            let mode_changed = self.modes.get(id).copied() != Some(*mode);
            let path_changed = self
                .watchers
                .get(id)
                .is_some_and(|(watched_path, _)| watched_path != path);
            if mode_changed || path_changed {
                self.reset_job(id);
                self.modes.insert(id.clone(), *mode);
            }

            if *mode != WatchMode::Realtime
                || self.watchers.contains_key(id)
                || self.failed.contains(id)
            {
                continue;
            }

            let tx = self.notifications_tx.clone();
            let callback_id = id.clone();
            let watcher = recommended_watcher(move |result: notify::Result<notify::Event>| {
                let notification = match result {
                    Ok(_) => WatchNotification::Changed(callback_id.clone()),
                    Err(err) => WatchNotification::Failed {
                        id: callback_id.clone(),
                        reason: err.to_string(),
                    },
                };
                let _ = tx.send(notification);
            });

            let mut watcher = match watcher {
                Ok(watcher) => watcher,
                Err(err) => {
                    self.failed.insert(id.clone());
                    failures.push(WatchFailure {
                        id: id.clone(),
                        reason: err.to_string(),
                    });
                    continue;
                }
            };

            if let Err(err) = watcher.watch(path, RecursiveMode::Recursive) {
                self.failed.insert(id.clone());
                failures.push(WatchFailure {
                    id: id.clone(),
                    reason: err.to_string(),
                });
                continue;
            }
            self.watchers.insert(id.clone(), (path.clone(), watcher));
        }
    }

    fn reset_job(&mut self, id: &str) {
        self.remove_watcher(id);
        self.modes.remove(id);
        self.failed.remove(id);
        self.debounce.clear(id);
    }

    fn remove_watcher(&mut self, id: &str) {
        if let Some((path, mut watcher)) = self.watchers.remove(id) {
            if let Err(err) = watcher.unwatch(&path) {
                log::debug!("실시간 감시 해제 실패 ({id}): {err}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct TemporaryWatchDirectory(PathBuf);

    impl TemporaryWatchDirectory {
        fn new() -> Self {
            let nonce = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be after the Unix epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "gpui-convenience-tools-watch-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("temporary watch directory should be created");
            Self(path)
        }
    }

    impl Drop for TemporaryWatchDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn debounce_coalesces_changes_until_two_seconds_are_quiet() {
        let started = Instant::now();
        let mut state = DebounceState::default();

        state.mark("job-1".to_string(), started);
        assert!(!state.ready("job-1", started + Duration::from_secs(1)));

        // 두 번째 변경은 대기 시간을 새로 시작한다.
        state.mark("job-1".to_string(), started + Duration::from_secs(1));
        assert!(!state.ready("job-1", started + Duration::from_millis(2_999)));
        assert!(state.ready("job-1", started + Duration::from_secs(3)));
        assert!(!state.ready("job-1", started + Duration::from_secs(4)));
    }

    #[test]
    fn watch_failure_is_reported_once_until_the_job_mode_changes() {
        let mut manager = WatchManager::new();
        manager
            .notifications_tx
            .send(WatchNotification::Failed {
                id: "job-1".to_string(),
                reason: "test watcher failure".to_string(),
            })
            .expect("test notification should be queued");

        let first = manager.drain_notifications();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].id, "job-1");

        manager
            .notifications_tx
            .send(WatchNotification::Failed {
                id: "job-1".to_string(),
                reason: "same test watcher failure".to_string(),
            })
            .expect("duplicate test notification should be queued");
        assert!(manager.drain_notifications().is_empty());

        manager.reset_job("job-1");
        manager
            .notifications_tx
            .send(WatchNotification::Failed {
                id: "job-1".to_string(),
                reason: "test watcher failure after reset".to_string(),
            })
            .expect("reset test notification should be queued");
        assert_eq!(manager.drain_notifications().len(), 1);
    }

    #[test]
    fn realtime_watcher_reports_a_changed_file_after_polling() {
        let directory = TemporaryWatchDirectory::new();
        let job = SyncJob {
            id: "watch-job".to_string(),
            source: directory.0.display().to_string(),
            watch_mode: WatchMode::Realtime,
            ..SyncJob::default()
        };
        let mut manager = WatchManager::new();

        assert!(manager.poll(std::slice::from_ref(&job)).is_empty());
        fs::write(directory.0.join("changed.txt"), b"changed")
            .expect("watched file should be written");

        let deadline = Instant::now() + Duration::from_secs(6);
        let mut ready = false;
        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
            manager.poll(std::slice::from_ref(&job));
            if manager.take_ready(&job.id, Instant::now()) {
                ready = true;
                break;
            }
        }

        assert!(ready, "a changed file should become ready after the debounce window");
    }
}
