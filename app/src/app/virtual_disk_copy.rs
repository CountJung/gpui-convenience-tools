//! VirtualBox 게스트 항목을 호스트 대상으로 복사하는 UI 상태와 작업 실행기.
//!
//! VDE-015의 복사는 현재 UI 스레드를 막지 않도록 별도 스레드에서 VDI를 다시
//! read-only로 열어 수행한다. UI에는 진행 이벤트만 전달하며, 원본 탐색 세션의
//! `NtfsGuestFileSource`를 작업 스레드와 공유하지 않는다.

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use gpui::{
    actions, App, AppContext, Context, Entity, KeyBinding, PathPromptOptions, Timer, Window,
};
use gpui_component::{input::InputEvent, input::InputState, notification::NotificationType};

use crate::virtual_disk::{
    copy::{CollisionPolicy, CopyIssue, CopyIssueLog, CopyReport, GuestCopyEngine},
    ntfs::NtfsGuestFileSource,
    path_policy::HostPathPolicy,
    vdi::VdiReader,
    GuestFileEntry, VdiPartition, VirtualDiskError,
};

use super::{state::PlatformEvent, AppRoot};

actions!(
    virtual_disk,
    [
        CopySelected,
        EnterSelected,
        ParentDirectory,
        SelectAll,
        Refresh,
    ]
);

pub(crate) const VIRTUAL_DISK_KEY_CONTEXT: &str = "VirtualDisk";

pub(crate) fn init_virtual_disk_keymap(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-c", CopySelected, Some(VIRTUAL_DISK_KEY_CONTEXT)),
        KeyBinding::new("cmd-c", CopySelected, Some(VIRTUAL_DISK_KEY_CONTEXT)),
        KeyBinding::new("enter", EnterSelected, Some(VIRTUAL_DISK_KEY_CONTEXT)),
        KeyBinding::new("backspace", ParentDirectory, Some(VIRTUAL_DISK_KEY_CONTEXT)),
        KeyBinding::new("ctrl-a", SelectAll, Some(VIRTUAL_DISK_KEY_CONTEXT)),
        KeyBinding::new("cmd-a", SelectAll, Some(VIRTUAL_DISK_KEY_CONTEXT)),
        KeyBinding::new("f5", Refresh, Some(VIRTUAL_DISK_KEY_CONTEXT)),
    ]);
}

const COPY_CHUNK_BYTES: usize = 1024 * 1024;
const MAX_HOST_PATH_UNITS: usize = 260;

/// 현재 VirtualBox 복사 작업의 UI 진행 상태.
#[derive(Clone, Debug, Default)]
pub(crate) struct VirtualDiskCopyProgress {
    pub(crate) total_entries: usize,
    pub(crate) completed_entries: usize,
    pub(crate) current_path: String,
    pub(crate) copied_files: u64,
    pub(crate) copied_bytes: u64,
    pub(crate) skipped_entries: u64,
    pub(crate) failed_entries: u64,
    pub(crate) stopping: bool,
}

/// 종료된 VirtualBox 복사의 마지막 요약.
#[derive(Clone, Debug, Default)]
pub(crate) struct VirtualDiskCopySummary {
    pub(crate) cancelled: bool,
    pub(crate) copied_files: u64,
    pub(crate) copied_bytes: u64,
    pub(crate) skipped_entries: u64,
    pub(crate) failed_entries: u64,
    pub(crate) issues: Vec<CopyIssue>,
    pub(crate) error: Option<String>,
}

impl VirtualDiskCopySummary {
    pub(crate) fn line(&self) -> String {
        if let Some(error) = &self.error {
            return format!("복사하지 못했습니다: {error}");
        }

        let state = if self.cancelled {
            "중지됨"
        } else {
            "완료"
        };
        format!(
            "{state} · 파일 {}개 · {} · 건너뜀 {} · 실패 {}",
            self.copied_files,
            format_bytes(self.copied_bytes),
            self.skipped_entries,
            self.failed_entries
        )
    }
}

/// VDI 복사 패널이 소유하는 입력·진행·중지 상태.
#[derive(Default)]
pub(crate) struct VirtualDiskCopyState {
    pub(crate) target_path_text: String,
    pub(crate) target_path_input: Option<Entity<InputState>>,
    pub(crate) progress: Option<VirtualDiskCopyProgress>,
    pub(crate) summary: Option<VirtualDiskCopySummary>,
    pub(crate) cancel: Arc<AtomicBool>,
    pub(crate) validation_auto_copy_scheduled: bool,
    /// 릴리스 검증 하네스 전용 자동 중지 지연. 일반 사용자 설정에는 저장하지 않는다.
    pub(crate) validation_cancel_after_ms: Option<u64>,
}

/// 백그라운드 작업이 UI 채널로 전달하는 종료 결과.
#[derive(Debug)]
pub(crate) struct VirtualDiskCopyOutcome {
    pub(crate) cancelled: bool,
    pub(crate) report: CopyReport,
    pub(crate) error: Option<String>,
}

impl AppRoot {
    /// 격리 릴리스 검증에서만 시드된 선택 항목의 복사를 예약한다.
    pub(crate) fn schedule_validation_virtual_disk_copy(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.virtual_disk.validation_auto_copy_requested
            || self.virtual_disk.copy.validation_auto_copy_scheduled
            || self.virtual_disk.selected_paths.is_empty()
            || self.virtual_disk.copy.progress.is_some()
            || self.virtual_disk.copy.summary.is_some()
        {
            return;
        }

        self.virtual_disk.copy.validation_auto_copy_scheduled = true;
        cx.spawn_in(window, async move |this, cx| {
            Timer::after(std::time::Duration::from_millis(500)).await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.virtual_disk.validation_auto_copy_requested = false;
                this.start_virtual_disk_copy(window, cx);
            });
        })
        .detach();
    }

    pub(crate) fn ensure_virtual_disk_target_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.virtual_disk.copy.target_path_input.is_some() {
            return;
        }

        let path = self.virtual_disk.copy.target_path_text.clone();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(r"예: D:\복구\KakaoTalk")
                .default_value(path)
        });
        let subscription = cx.subscribe(
            &input,
            |this: &mut Self, input: Entity<InputState>, event: &InputEvent, cx| {
                if let InputEvent::Change = event {
                    this.virtual_disk.copy.target_path_text = input.read(cx).value().to_string();
                    cx.notify();
                }
            },
        );
        self.virtual_disk.copy.target_path_input = Some(input);
        self.subscriptions.push(subscription);
    }

    pub(crate) fn pick_virtual_disk_target(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("VDI 파일을 복사할 대상 폴더 선택".into()),
        });

        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let picked = path.display().to_string();

            let _ = this.update_in(cx, |this, window, cx| {
                this.virtual_disk.copy.target_path_text = picked.clone();
                if let Some(input) = this.virtual_disk.copy.target_path_input.clone() {
                    input.update(cx, |state, cx| state.set_value(picked.clone(), window, cx));
                }
                this.push_log(
                    "INFO",
                    format!("VDI 복사 대상 폴더를 선택했습니다: {picked}"),
                );
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn start_virtual_disk_copy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.virtual_disk.copy.progress.is_some() {
            self.notify_toast(
                "이미 실행 중인 VDI 복사가 있습니다",
                NotificationType::Info,
                window,
                cx,
            );
            return;
        }

        let target_text = self.virtual_disk.copy.target_path_text.trim().to_string();
        let Some(source_path) = self.virtual_disk.vdi_path.clone() else {
            self.notify_toast(
                "먼저 VDI와 NTFS 파티션을 선택하세요",
                NotificationType::Warning,
                window,
                cx,
            );
            return;
        };
        let Some(partition_index) = self.virtual_disk.selected_partition else {
            self.notify_toast(
                "복사할 NTFS 파티션을 선택하세요",
                NotificationType::Warning,
                window,
                cx,
            );
            return;
        };
        if target_text.is_empty() {
            self.notify_toast(
                "복사 대상 폴더를 입력하거나 선택하세요",
                NotificationType::Warning,
                window,
                cx,
            );
            return;
        }

        let entries: Vec<GuestFileEntry> = self
            .virtual_disk
            .entries
            .iter()
            .filter(|entry| self.virtual_disk.selected_paths.contains(&entry.path))
            .cloned()
            .collect();
        if entries.is_empty() {
            self.notify_toast(
                "복사할 파일 또는 폴더를 선택하세요",
                NotificationType::Warning,
                window,
                cx,
            );
            return;
        }
        let Some(partition) = self.virtual_disk.partitions.get(partition_index).cloned() else {
            self.notify_toast(
                "선택한 파티션 정보를 찾을 수 없습니다",
                NotificationType::Warning,
                window,
                cx,
            );
            return;
        };

        self.virtual_disk
            .copy
            .cancel
            .store(false, Ordering::Relaxed);
        self.virtual_disk.copy.progress = Some(VirtualDiskCopyProgress {
            total_entries: entries.len(),
            ..VirtualDiskCopyProgress::default()
        });
        self.virtual_disk.copy.summary = None;
        self.push_log(
            "INFO",
            format!(
                "VDI 복사를 시작했습니다: {}개 항목 → {target_text}",
                entries.len()
            ),
        );

        let cancel = Arc::clone(&self.virtual_disk.copy.cancel);
        let event_tx = self.event_tx.clone();
        std::thread::spawn(move || {
            let outcome = copy_selected_entries(
                source_path,
                partition,
                PathBuf::from(target_text),
                entries,
                cancel,
                event_tx.clone(),
            );
            let _ = event_tx.send(PlatformEvent::VirtualDiskCopyFinished { outcome });
        });
        if let Some(delay_ms) = self.virtual_disk.copy.validation_cancel_after_ms.take() {
            cx.spawn_in(window, async move |this, cx| {
                Timer::after(std::time::Duration::from_millis(delay_ms)).await;
                let _ = this.update_in(cx, |this, window, cx| {
                    if this.virtual_disk.copy.progress.is_some() {
                        this.stop_virtual_disk_copy(window, cx);
                    }
                });
            })
            .detach();
        }
        cx.notify();
    }

    pub(crate) fn stop_virtual_disk_copy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.virtual_disk.copy.progress.is_none() {
            self.notify_toast(
                "실행 중인 VDI 복사가 없습니다",
                NotificationType::Warning,
                window,
                cx,
            );
            return;
        }
        self.virtual_disk.copy.cancel.store(true, Ordering::Relaxed);
        if let Some(progress) = self.virtual_disk.copy.progress.as_mut() {
            progress.stopping = true;
        }
        self.push_log("WARN", "VDI 복사 중지를 요청했습니다.".to_string());
        self.notify_toast("VDI 복사를 중지합니다", NotificationType::Info, window, cx);
        cx.notify();
    }

    pub(crate) fn handle_virtual_disk_copy_started(&mut self, total_entries: usize) {
        self.virtual_disk.copy.progress = Some(VirtualDiskCopyProgress {
            total_entries,
            ..VirtualDiskCopyProgress::default()
        });
        self.virtual_disk.copy.summary = None;
    }

    pub(crate) fn handle_virtual_disk_copy_progress(
        &mut self,
        completed_entries: usize,
        total_entries: usize,
        current_path: String,
        report: &CopyReport,
    ) {
        if let Some(progress) = self.virtual_disk.copy.progress.as_mut() {
            progress.completed_entries = completed_entries;
            progress.total_entries = total_entries;
            progress.current_path = current_path;
            progress.copied_files = report.copied_files;
            progress.copied_bytes = report.copied_bytes;
            progress.skipped_entries = report.skipped_entries;
            progress.failed_entries = report.issues.len() as u64;
        }
    }

    pub(crate) fn handle_virtual_disk_copy_finished(
        &mut self,
        outcome: VirtualDiskCopyOutcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let summary = VirtualDiskCopySummary {
            cancelled: outcome.cancelled,
            copied_files: outcome.report.copied_files,
            copied_bytes: outcome.report.copied_bytes,
            skipped_entries: outcome.report.skipped_entries,
            failed_entries: outcome.report.issues.len() as u64,
            issues: outcome.report.issues,
            error: outcome.error,
        };
        let line = summary.line();
        let has_error = summary.error.is_some();
        let failed = summary.failed_entries > 0 || summary.error.is_some();
        let unsuppressed_issue_count = summary
            .issues
            .iter()
            .filter(|issue| {
                !self
                    .virtual_disk
                    .suppressed_issue_keys
                    .contains(&issue.key())
            })
            .count();
        self.virtual_disk.copy.progress = None;
        self.virtual_disk.copy.summary = Some(summary);
        self.push_log(
            if failed { "ERROR" } else { "INFO" },
            format!("[VDI] {line}"),
        );
        let issue_log_lines: Vec<String> = self
            .virtual_disk
            .copy
            .summary
            .as_ref()
            .map(|summary| {
                summary
                    .issues
                    .iter()
                    .map(|issue| format!("[VDI][{}] {}: {}", issue.kind, issue.path, issue.detail))
                    .collect()
            })
            .unwrap_or_default();
        for message in issue_log_lines {
            self.push_log("ERROR", message);
        }
        if failed && (has_error || unsuppressed_issue_count > 0) {
            self.notify_toast(
                "VDI 복사에 실패한 항목이 있습니다",
                NotificationType::Warning,
                window,
                cx,
            );
        } else if self
            .virtual_disk
            .copy
            .summary
            .as_ref()
            .is_some_and(|summary| summary.cancelled)
        {
            self.notify_toast(
                "VDI 복사를 중지했습니다",
                NotificationType::Info,
                window,
                cx,
            );
        } else {
            self.notify_toast(
                "VDI 복사를 완료했습니다",
                NotificationType::Success,
                window,
                cx,
            );
        }
        cx.notify();
    }
}

fn copy_selected_entries(
    source_path: PathBuf,
    partition: VdiPartition,
    target_path: PathBuf,
    entries: Vec<GuestFileEntry>,
    cancel: Arc<AtomicBool>,
    event_tx: tokio::sync::mpsc::UnboundedSender<PlatformEvent>,
) -> VirtualDiskCopyOutcome {
    let total_entries = entries.len();
    let _ = event_tx.send(PlatformEvent::VirtualDiskCopyStarted { total_entries });

    let result = (|| {
        let reader = VdiReader::open(source_path)?;
        let mut source = NtfsGuestFileSource::open(reader, partition)?;
        let policy = HostPathPolicy::new(target_path, MAX_HOST_PATH_UNITS)?;
        let engine = GuestCopyEngine::new(policy, CollisionPolicy::Skip, COPY_CHUNK_BYTES)?;
        let mut issue_log = CopyIssueLog::default();
        let mut report = CopyReport::default();

        for (index, entry) in entries.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return Ok((report, true));
            }
            let item_report = engine.copy_entry_collecting_with_cancel(
                &mut source,
                entry,
                &mut issue_log,
                &cancel,
            );
            let item_cancelled = item_report.cancelled;
            report.merge(item_report);
            let _ = event_tx.send(PlatformEvent::VirtualDiskCopyProgress {
                completed_entries: if item_cancelled { index } else { index + 1 },
                total_entries,
                current_path: entry.path.to_string(),
                report: report.clone(),
            });
            if item_cancelled {
                return Ok((report, true));
            }
        }
        Ok((report, false))
    })();

    match result {
        Ok((report, cancelled)) => VirtualDiskCopyOutcome {
            cancelled,
            report,
            error: None,
        },
        Err(error) => VirtualDiskCopyOutcome {
            cancelled: cancel.load(Ordering::Relaxed),
            report: CopyReport::default(),
            error: Some(format_virtual_disk_error(&error)),
        },
    }
}

fn format_virtual_disk_error(error: &VirtualDiskError) -> String {
    format!("읽기 실패: {error}")
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    if bytes < 1024 * 1024 {
        return format!("{:.1} KB", bytes as f64 / 1024.0);
    }
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}
