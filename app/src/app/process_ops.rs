//! Process monitor preferences, transient input and bounded background lifecycle.
use super::{ActivePanel, AppRoot};
use crate::config::{update_config, ProcessMonitorConfig, ProcessSort};
use crate::process_monitor::{MonitorWorker, ProcessError, ProcessRow, WorkerReply};
use gpui::{AppContext, Context, Entity, ScrollHandle, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use std::time::{Duration, Instant};

#[derive(Default)]
pub(crate) struct ProcessMonitorState {
    pub config: ProcessMonitorConfig,
    pub save_error: bool,
    pub scroll: ScrollHandle,
    pub search_input: Option<Entity<InputState>>,
    pub interval_input: Option<Entity<InputState>>,
    /// Search and process snapshots must never be persisted or logged.
    pub query: String,
    pub rows: Vec<ProcessRow>,
    pub selected: Option<ProcessRow>,
    pub status: String,
    pub failed: bool,
    pub busy: bool,
    pub terminating: bool,
    subscriptions: Vec<Subscription>,
    worker: Option<MonitorWorker>,
    last_scan: Option<Instant>,
}

impl ProcessMonitorState {
    pub fn from_config(mut config: ProcessMonitorConfig) -> Self {
        config.normalize();
        Self {
            config,
            ..Default::default()
        }
    }
}

impl AppRoot {
    pub(crate) fn ensure_process_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.process_monitor.search_input.is_some() {
            return;
        }
        let search = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("프로세스 이름 · PID · 경로 검색 (저장하지 않음)")
                .validate(|text, _| text.len() <= 1024)
        });
        let subscription = cx.subscribe_in(
            &search,
            window,
            |this: &mut Self, input, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.process_monitor.query = input.read(cx).value().to_string();
                    this.process_monitor
                        .scroll
                        .set_offset(gpui::point(gpui::px(0.0), gpui::px(0.0)));
                    cx.notify();
                }
            },
        );
        let interval = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(
                    self.process_monitor
                        .config
                        .refresh_interval_secs
                        .to_string(),
                )
                .placeholder("1–60")
                .validate(|text, _| text.len() <= 2 && text.chars().all(|ch| ch.is_ascii_digit()))
        });
        let interval_subscription = cx.subscribe_in(&interval, window, |this: &mut Self, input, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                match input.read(cx).value().parse::<u32>() {
                    Ok(seconds) if (1..=60).contains(&seconds) => {
                        this.process_monitor.config.refresh_interval_secs = seconds;
                        this.process_monitor.failed = false;
                        this.process_monitor.status.clear();
                        this.persist_process_config(cx);
                    }
                    _ => {
                        this.process_monitor.failed = true;
                        this.process_monitor.status = "갱신 주기는 1–60초 정수로 입력하세요. 마지막 유효 설정을 유지합니다.".into();
                        cx.notify();
                    }
                }
            }
        });
        self.process_monitor.search_input = Some(search);
        self.process_monitor.interval_input = Some(interval);
        self.process_monitor
            .subscriptions
            .extend([subscription, interval_subscription]);
        self.tick_process_monitor(cx);
    }

    pub(crate) fn persist_process_config(&mut self, cx: &mut Context<Self>) {
        #[cfg(test)]
        if !self.sync.external_side_effects_enabled {
            cx.notify();
            return;
        }
        let config = self.process_monitor.config.clone();
        self.process_monitor.save_error =
            update_config(|cfg| cfg.process_monitor = config).is_err();
        if self.process_monitor.save_error {
            log::error!("프로세스 모니터 설정 저장 실패");
        }
        cx.notify();
    }

    pub(crate) fn set_process_sort(&mut self, sort: ProcessSort, cx: &mut Context<Self>) {
        self.process_monitor.config.sort = sort;
        self.persist_process_config(cx);
    }

    pub(crate) fn cycle_process_count(&mut self, cx: &mut Context<Self>) {
        self.process_monitor.config.top_count = match self.process_monitor.config.top_count {
            0..=49 => 50,
            50..=99 => 100,
            100..=199 => 200,
            _ => 25,
        };
        self.persist_process_config(cx);
    }

    /// The main UI timer calls this method. Only the active panel schedules scans.
    pub(crate) fn tick_process_monitor(&mut self, cx: &mut Context<Self>) {
        if let Some(worker) = &self.process_monitor.worker {
            match worker.poll() {
                Ok(Some(reply)) => self.apply_process_reply(reply, cx),
                Err(error) => {
                    self.process_monitor.worker = None;
                    self.process_monitor.busy = false;
                    self.process_monitor.terminating = false;
                    self.process_monitor.last_scan = Some(Instant::now());
                    self.process_failure(error, cx);
                }
                Ok(None) => {}
            }
        }
        if self.active_panel != ActivePanel::ProcessMonitor || self.process_monitor.busy {
            return;
        }
        let due = self.process_monitor.last_scan.is_none_or(|last| {
            last.elapsed()
                >= Duration::from_secs(u64::from(self.process_monitor.config.refresh_interval_secs))
        });
        if due {
            self.refresh_process_monitor(cx);
        }
    }

    pub(crate) fn refresh_process_monitor(&mut self, cx: &mut Context<Self>) {
        if self.process_monitor.busy {
            return;
        }
        #[cfg(test)]
        if !self.sync.external_side_effects_enabled {
            return;
        }
        // Failed starts must also obey the refresh interval, avoiding timer/log storms.
        self.process_monitor.last_scan = Some(Instant::now());
        if self.process_monitor.worker.is_none() {
            match MonitorWorker::start() {
                Ok(worker) => self.process_monitor.worker = Some(worker),
                Err(error) => {
                    self.process_failure(error, cx);
                    return;
                }
            }
        }
        let result = self
            .process_monitor
            .worker
            .as_ref()
            .ok_or(ProcessError::WorkerUnavailable)
            .and_then(MonitorWorker::scan);
        match result {
            Ok(()) => {
                self.process_monitor.busy = true;
                self.process_monitor.last_scan = Some(Instant::now());
                self.process_monitor.failed = false;
                self.process_monitor.status = "백그라운드에서 프로세스 정보를 갱신하는 중…".into();
                cx.notify();
            }
            Err(error) => self.process_failure(error, cx),
        }
    }

    fn apply_process_reply(&mut self, reply: WorkerReply, cx: &mut Context<Self>) {
        self.process_monitor.busy = false;
        self.process_monitor.terminating = false;
        match reply {
            WorkerReply::ScanFailed(error) => {
                self.process_monitor.selected = None;
                self.process_failure(error, cx);
            }
            WorkerReply::Snapshot(rows) => {
                if let Some(selected) = &self.process_monitor.selected {
                    if !rows.iter().any(|row| row.identity == selected.identity) {
                        self.process_monitor.selected = None;
                        self.process_monitor.status =
                            "선택한 프로세스가 사라지거나 신원이 바뀌어 종료 확인을 취소했습니다."
                                .into();
                        self.process_monitor.failed = true;
                        self.process_monitor.rows = rows;
                        cx.notify();
                        return;
                    }
                }
                self.process_monitor.status = format!(
                    "{}개 프로세스 갱신 완료 · CPU는 전체 논리 CPU 기준 0–100%",
                    rows.len()
                );
                self.process_monitor.failed = false;
                self.process_monitor.rows = rows;
            }
            WorkerReply::Terminated(result) => {
                self.process_monitor.selected = None;
                match result {
                    Ok(()) => {
                        self.process_monitor.status = "선택한 프로세스에 종료 요청을 전달했습니다. 새로고침으로 결과를 확인하세요.".into();
                        self.process_monitor.failed = false;
                    }
                    Err(error) => self.process_failure(error, cx),
                }
            }
        }
        cx.notify();
    }

    pub(crate) fn select_process(&mut self, row: ProcessRow, cx: &mut Context<Self>) {
        if self.process_monitor.terminating {
            return;
        }
        self.process_monitor.selected = Some(row);
        // Confirmation lives above the list; selecting a deeply scrolled row must reveal it.
        self.process_monitor
            .scroll
            .set_offset(gpui::point(gpui::px(0.0), gpui::px(0.0)));
        cx.notify();
    }

    pub(crate) fn cancel_process_termination(&mut self, cx: &mut Context<Self>) {
        if self.process_monitor.terminating {
            return;
        }
        self.process_monitor.selected = None;
        cx.notify();
    }

    pub(crate) fn confirm_process_termination(&mut self, cx: &mut Context<Self>) {
        if self.process_monitor.busy {
            return;
        }
        let Some(row) = self.process_monitor.selected.clone() else {
            return;
        };
        if row.blocked_reason.is_some() {
            self.process_failure(ProcessError::Protected, cx);
            return;
        }
        #[cfg(test)]
        if !self.sync.external_side_effects_enabled {
            self.process_failure(ProcessError::AccessDenied, cx);
            return;
        }
        if self.process_monitor.worker.is_none() {
            match MonitorWorker::start() {
                Ok(worker) => self.process_monitor.worker = Some(worker),
                Err(error) => {
                    self.process_failure(error, cx);
                    return;
                }
            }
        }
        let result = self
            .process_monitor
            .worker
            .as_ref()
            .ok_or(ProcessError::WorkerUnavailable)
            .and_then(|worker| worker.terminate(row.identity));
        match result {
            Ok(()) => {
                self.process_monitor.busy = true;
                self.process_monitor.terminating = true;
                self.process_monitor.status =
                    "대상 신원을 다시 확인하고 종료 요청을 보내는 중…".into();
                cx.notify();
            }
            Err(error) => self.process_failure(error, cx),
        }
    }

    fn process_failure(&mut self, error: ProcessError, cx: &mut Context<Self>) {
        self.process_monitor.failed = true;
        self.process_monitor.status = error.to_string();
        // Error kind only: no names, executable paths, PIDs or search input in logs.
        log::warn!("프로세스 모니터 작업 실패: {error}");
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
