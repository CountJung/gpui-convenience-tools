//! 광고 차단 · 서비스 관리 · 로그 설정 조작.
//!
//! 상태를 바꾼 뒤 백그라운드 동기화(`sync_scanner_state`)와
//! 설정 저장(`persist_config`)을 함께 수행하는 것이 이 계층의 책임이다.

use gpui::{Context, Window};
use gpui_component::notification::NotificationType;

use super::state::{PlatformEvent, TargetApp};
use super::AppRoot;
use crate::config::{update_config, LogConfig};

impl AppRoot {
    // ─────────────────────────────────────────────
    // 광고 차단 상태 조작
    // ─────────────────────────────────────────────

    pub(super) fn refresh_target_status(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let running = self
            .app_state
            .targets
            .iter()
            .filter(|t| t.enabled)
            .any(|t| self.platform.is_target_running(&t.process_name));

        let _ = self
            .event_tx
            .send(PlatformEvent::TargetStatusChanged(running));
        self.process_pending_events(window, cx);
    }

    pub(super) fn sync_scanner_state(&self) {
        if let Ok(mut state) = self.scanner_state.lock() {
            state.service_enabled = self.app_state.is_active;
            state.targets = self.app_state.targets.clone();
            state.scan_interval_secs = self.scan_interval_secs;
        }
    }

    pub(super) fn persist_config(&self) {
        let is_active = self.app_state.is_active;
        let targets = self.app_state.targets.clone();
        let interval = self.scan_interval_secs;

        if let Err(err) = update_config(move |cfg| {
            cfg.service_enabled = is_active;
            cfg.targets = targets;
            cfg.scan_interval_secs = interval;
        }) {
            log::error!("설정 저장 실패: {err}");
        }
    }

    pub fn set_scan_interval(&mut self, secs: u32, cx: &mut Context<Self>) {
        let secs = secs.max(1);
        self.scan_interval_secs = secs;
        self.sync_scanner_state();
        self.persist_config();
        self.push_log("INFO", format!("스캔 주기를 {secs}초로 변경했습니다."));
        cx.notify();
    }

    pub(crate) fn set_service_enabled(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = self.event_tx.send(PlatformEvent::ServiceToggled(enabled));
        self.process_pending_events(window, cx);
        cx.notify();
    }

    pub(crate) fn set_target_enabled(
        &mut self,
        index: usize,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = self
            .event_tx
            .send(PlatformEvent::TargetToggled { index, enabled });
        self.process_pending_events(window, cx);
        cx.notify();
    }

    pub(crate) fn remove_target(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = self.event_tx.send(PlatformEvent::TargetRemoved { index });
        self.process_pending_events(window, cx);
        cx.notify();
    }

    pub(crate) fn refresh_running_processes(&mut self) {
        match self.platform.list_running_processes() {
            Ok(processes) => self.running_processes = processes,
            Err(err) => log::error!("실행 중인 프로세스 조회 실패: {err}"),
        }
    }

    pub(crate) fn add_target_process(
        &mut self,
        process_name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .app_state
            .targets
            .iter()
            .any(|target| target.process_name.eq_ignore_ascii_case(process_name))
        {
            self.notify_toast("이미 등록된 타겟입니다", NotificationType::Info, window, cx);
            return;
        }

        let display_name = process_name
            .strip_suffix(".exe")
            .unwrap_or(process_name)
            .to_string();

        self.app_state.targets.push(TargetApp {
            process_name: process_name.to_string(),
            display_name,
            enabled: true,
            ad_window_class: "auto:webview".to_string(),
        });

        self.sync_scanner_state();
        self.persist_config();
        self.refresh_target_status(window, cx);
        self.refresh_running_processes();
        self.push_log("INFO", format!("타겟을 추가했습니다: {process_name}"));
        self.notify_toast("타겟을 추가했습니다", NotificationType::Success, window, cx);
        cx.notify();
    }
    // ─────────────────────────────────────────────
    // 서비스 관리
    // ─────────────────────────────────────────────

    pub(crate) fn refresh_sys_services(&mut self) {
        match self.platform.list_sys_services() {
            Ok(services) => self.sys_services = services,
            Err(err) => log::error!("서비스 목록 조회 실패: {err}"),
        }
    }

    pub(crate) fn toggle_favorite_service(&mut self, name: &str) {
        if let Some(pos) = self.favorite_services.iter().position(|n| n == name) {
            self.favorite_services.remove(pos);
        } else {
            self.favorite_services.push(name.to_string());
        }

        let favorites = self.favorite_services.clone();
        if let Err(err) = update_config(move |cfg| cfg.favorite_services = favorites) {
            log::error!("즐겨찾기 저장 실패: {err}");
        }
    }

    pub(crate) fn is_favorite_service(&self, name: &str) -> bool {
        self.favorite_services.iter().any(|n| n == name)
    }

    /// 서비스 뷰에서 발생하는 동작 결과를 로그 패널에 기록한다.
    pub fn push_service_log(&mut self, message: &str, _window: &mut Window, cx: &mut Context<Self>) {
        self.push_log("INFO", message.to_string());
        cx.notify();
    }
    // ─────────────────────────────────────────────
    // 로그 설정
    // ─────────────────────────────────────────────

    pub(crate) fn update_log_config(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut LogConfig),
    ) {
        edit(&mut self.log_config);
        crate::logging::update_config(self.log_config.clone());

        let log_config = self.log_config.clone();
        if let Err(err) = update_config(move |cfg| cfg.log = log_config) {
            log::error!("로그 설정 저장 실패: {err}");
        }

        self.push_log("INFO", "로그 설정을 변경했습니다.".to_string());
        cx.notify();
    }
}
