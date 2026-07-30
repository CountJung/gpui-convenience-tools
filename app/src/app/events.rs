//! 백그라운드 → UI 이벤트 처리와 로그·토스트 유틸.
//!
//! [`PlatformEvent`] 채널을 비우는 유일한 지점이다. 렌더 진입 시점과
//! UI 핸들러에서 호출되며, 상태 변경은 모두 여기를 거친다.

use gpui::{Context, Window};
use gpui_component::{
    notification::{Notification, NotificationType},
    WindowExt,
};

use super::state::{LogEntry, PlatformEvent, SyncJobStatus, SyncRunning};
use super::AppRoot;
use crate::sync::SyncOutcome;

#[cfg(target_os = "windows")]
use crate::platform::set_tray_service_active;

impl AppRoot {
    // ─────────────────────────────────────────────
    // 공통 유틸
    // ─────────────────────────────────────────────

    pub(super) fn push_log(&mut self, level: &str, message: String) {
        self.app_state.log_entries.push(LogEntry {
            level: level.to_string(),
            message,
        });
        // UI 로그 패널이 무한정 자라지 않도록 상한을 둔다(파일 로그는 별도 보존).
        const MAX_UI_LOG_ENTRIES: usize = 2000;
        if self.app_state.log_entries.len() > MAX_UI_LOG_ENTRIES {
            let excess = self.app_state.log_entries.len() - MAX_UI_LOG_ENTRIES;
            self.app_state.log_entries.drain(0..excess);
        }
        self.log_scroll_handle.scroll_to_bottom();
    }

    pub(super) fn notify_toast(
        &self,
        message: &'static str,
        kind: NotificationType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        #[cfg(test)]
        if !self.external_side_effects_enabled {
            return;
        }

        window.push_notification(
            Notification::new()
                .message(message)
                .with_type(kind)
                .title("gpui-convenience-tools"),
            cx,
        );
    }

    pub(super) fn process_pending_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                PlatformEvent::AdBlocked => {
                    self.app_state.blocked_count += 1;
                    self.push_log("SUCCESS", "광고 창을 감지하여 숨겼습니다.".to_string());
                    self.notify_toast("광고를 차단했습니다", NotificationType::Success, window, cx);
                }
                PlatformEvent::TargetStatusChanged(is_running) => {
                    self.app_state.is_target_running = is_running;
                    self.push_log(
                        "INFO",
                        if is_running {
                            "타겟 상태: 실행 중".to_string()
                        } else {
                            "타겟 상태: 미실행".to_string()
                        },
                    );
                }
                PlatformEvent::ServiceToggled(enabled) => {
                    self.app_state.is_active = enabled;
                    self.sync_scanner_state();
                    self.persist_config();
                    #[cfg(target_os = "windows")]
                    set_tray_service_active(enabled);
                    self.push_log(
                        "INFO",
                        if enabled {
                            "광고 차단을 켰습니다.".to_string()
                        } else {
                            "광고 차단을 껐습니다.".to_string()
                        },
                    );
                    self.notify_toast(
                        if enabled {
                            "광고 차단을 켰습니다"
                        } else {
                            "광고 차단을 껐습니다"
                        },
                        NotificationType::Info,
                        window,
                        cx,
                    );
                }
                PlatformEvent::SyncAutoToggled(enabled) => {
                    self.sync_enabled = enabled;
                    self.apply_sync_enabled();
                    self.push_log(
                        "INFO",
                        if enabled {
                            "파일 동기화 자동 실행을 켰습니다.".to_string()
                        } else {
                            "파일 동기화 자동 실행을 껐습니다.".to_string()
                        },
                    );
                    self.notify_toast(
                        if enabled {
                            "파일 동기화를 켰습니다"
                        } else {
                            "파일 동기화를 껐습니다"
                        },
                        NotificationType::Info,
                        window,
                        cx,
                    );
                }
                PlatformEvent::TargetToggled { index, enabled } => {
                    let message = if let Some(target) = self.app_state.targets.get_mut(index) {
                        target.enabled = enabled;
                        format!("{} 활성 상태 변경: {}", target.process_name, enabled)
                    } else {
                        continue;
                    };

                    self.sync_scanner_state();
                    self.persist_config();
                    self.push_log("INFO", message);
                }
                PlatformEvent::TargetRemoved { index } => {
                    if index < self.app_state.targets.len() {
                        let name = self.app_state.targets.remove(index).process_name;
                        self.sync_scanner_state();
                        self.persist_config();
                        self.push_log("INFO", format!("타겟을 삭제했습니다: {name}"));
                    }
                }
                PlatformEvent::SyncStarted { id, label } => {
                    self.push_log("INFO", format!("[{label}] 동기화를 시작했습니다."));
                    self.sync_running = Some(SyncRunning {
                        id,
                        label,
                        current_path: String::new(),
                        copied: 0,
                        skipped: 0,
                        failed: 0,
                        stopping: false,
                    });
                }
                PlatformEvent::SyncProgress {
                    id,
                    current_path,
                    copied,
                    skipped,
                    failed,
                } => {
                    // 앞선 작업이 끝난 뒤 늦게 도착한 보고는 무시한다.
                    if let Some(running) = self
                        .sync_running
                        .as_mut()
                        .filter(|running| running.id == id)
                    {
                        running.current_path = current_path;
                        running.copied = copied;
                        running.skipped = skipped;
                        running.failed = failed;
                    }
                }
                PlatformEvent::SyncFinished { id, label, outcome } => {
                    if self
                        .sync_running
                        .as_ref()
                        .is_some_and(|running| running.id == id)
                    {
                        self.sync_running = None;
                    }
                    self.handle_sync_finished(id, label, outcome, window, cx);
                }
            }
        }
    }

    pub(super) fn handle_sync_finished(
        &mut self,
        id: String,
        label: String,
        outcome: SyncOutcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // 실행 중 작업이 삭제됐을 수 있으므로 아직 존재할 때만 상태를 반영한다.
        if !self.sync_jobs.iter().any(|job| job.id == id) {
            return;
        }

        self.sync_status.insert(
            id,
            SyncJobStatus {
                last_run: Some(crate::logging::now_hms()),
                summary: outcome.summary(),
                failed: outcome.has_failures(),
            },
        );

        if outcome.cancelled {
            self.push_log("WARN", format!("[{label}] 중지했습니다 — {}", outcome.summary()));
        } else if outcome.copied > 0 || outcome.deleted > 0 {
            self.push_log("INFO", format!("[{label}] {}", outcome.summary()));
        }

        // ── 실패 처리 ──
        //
        // 로그에는 파일마다 남기지 않는다. 실패가 많은 폴더에서 로그가 실패 목록의
        // 복사본이 되어 다른 활동 기록을 밀어내기 때문이다. 개별 사유는 이 패널의
        // 실패 목록이 소유하고, 로그에는 실행 단위 요약 한 줄만 남긴다.
        let mut unsuppressed_new = 0usize;
        let mut newly_recorded = 0usize;
        for failure in &outcome.failures {
            let key = failure.key();
            if self.sync_failures.iter().any(|f| f.key() == key) {
                continue;
            }

            self.sync_failures.push(failure.clone());
            newly_recorded += 1;

            if !self.suppressed_sync_failures.contains(&key) {
                unsuppressed_new += 1;
            }
        }

        if newly_recorded > 0 || outcome.truncated_failures > 0 {
            let omitted = outcome.truncated_failures;
            let mut message =
                format!("[{label}] {newly_recorded}건을 복사하지 못했습니다. 사유는 아래 실패 목록에서 확인하세요.");
            if omitted > 0 {
                message.push_str(&format!(" (많아서 {omitted}건은 목록에서 생략)"));
            }
            self.push_log("ERROR", message);
        }

        // 기록 상한
        const MAX_TRACKED_FAILURES: usize = 300;
        if self.sync_failures.len() > MAX_TRACKED_FAILURES {
            let excess = self.sync_failures.len() - MAX_TRACKED_FAILURES;
            self.sync_failures.drain(0..excess);
        }
        self.log_scroll_handle.scroll_to_bottom();

        if unsuppressed_new > 0 && self.sync_notify_enabled {
            window.push_notification(
                Notification::new()
                    .message(format!(
                        "'{label}' 동기화 중 {unsuppressed_new}건을 복사하지 못했습니다. 파일 동기화 패널에서 사유를 확인하세요."
                    ))
                    .with_type(NotificationType::Warning)
                    .title("동기화 실패"),
                cx,
            );
        }

        cx.notify();
    }
}
