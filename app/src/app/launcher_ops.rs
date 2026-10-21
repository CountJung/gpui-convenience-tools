//! 빠른 실행기 상태·지연 입력·등록과 명시 실행. 검색어와 초안은 메모리에만 유지한다.
use super::{ActivePanel, AppRoot};
use crate::config::{
    launcher::{LauncherConfig, LauncherEntry, LauncherHotkey, LauncherKind},
    update_config,
};
use crate::quick_launcher::{
    self,
    hotkey::{HotkeyEvent, HotkeyRegistration},
    LauncherError,
};
use gpui::{AppContext, Context, Entity, PathPromptOptions, ScrollHandle, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use std::{
    path::PathBuf,
    process::Child,
    sync::mpsc::{self, Receiver},
};

#[derive(Default)]
pub(crate) struct LauncherState {
    pub config: LauncherConfig,
    pub save_error: bool,
    pub scroll: ScrollHandle,
    pub search: Option<Entity<InputState>>,
    pub name: Option<Entity<InputState>>,
    pub path: Option<Entity<InputState>>,
    pub args: Option<Entity<InputState>>,
    pub query: String,
    pub selected: Option<usize>,
    pub editing: Option<usize>,
    pub draft_kind: LauncherKind,
    pub draft_trusted: bool,
    pub pending_script: Option<LauncherEntry>,
    pub status: String,
    pub failed: bool,
    pub hotkey_status: String,
    pub hotkey_failed: bool,
    subscriptions: Vec<Subscription>,
    hotkey: Option<HotkeyRegistration>,
    applied_hotkey: Option<(bool, LauncherHotkey)>,
    /// Reap only children started here; never enumerate or terminate other processes.
    children: Vec<Child>,
    launch_result: Option<Receiver<Result<Child, LauncherError>>>,
}

impl LauncherState {
    pub fn from_config(config: LauncherConfig) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }
    pub fn is_busy(&self) -> bool {
        self.launch_result.is_some()
    }
}

impl AppRoot {
    pub(crate) fn ensure_launcher_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.launcher.search.is_some() {
            return;
        }
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("이름 또는 경로 검색"));
        let subscription =
            cx.subscribe_in(&search, window, |this, input, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.launcher.query = input.read(cx).value().to_string();
                    this.launcher.selected = None;
                    cx.notify();
                }
            });
        self.launcher.subscriptions.push(subscription);
        self.launcher.search = Some(search);
        self.launcher.name =
            Some(cx.new(|cx| InputState::new(window, cx).placeholder("표시 이름")));
        self.launcher.path = Some(
            cx.new(|cx| InputState::new(window, cx).placeholder("직접 확인한 로컬 절대 경로")),
        );
        self.launcher.args = Some(cx.new(|cx| {
            InputState::new(window, cx)
                .default_value("[]")
                .placeholder(r#"["--option", "value with spaces"]"#)
        }));
        for input in [
            &self.launcher.name,
            &self.launcher.path,
            &self.launcher.args,
        ]
        .into_iter()
        .flatten()
        {
            let subscription =
                cx.subscribe_in(input, window, |this, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.launcher.draft_trusted = false;
                        this.launcher.pending_script = None;
                        cx.notify();
                    }
                });
            self.launcher.subscriptions.push(subscription);
        }
    }

    pub(crate) fn persist_launcher_config(&mut self, cx: &mut Context<Self>) {
        #[cfg(test)]
        if !self.sync.external_side_effects_enabled {
            cx.notify();
            return;
        }
        let config = self.launcher.config.clone();
        self.launcher.save_error = update_config(|cfg| cfg.launcher = config).is_err();
        if self.launcher.save_error {
            log::error!("빠른 실행기 설정 저장 실패");
        }
        cx.notify();
    }

    pub(crate) fn set_launcher_hotkey_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.launcher.config.enabled = enabled;
        self.persist_launcher_config(cx);
        self.tick_launcher(cx);
    }

    pub(crate) fn cycle_launcher_hotkey(&mut self, cx: &mut Context<Self>) {
        self.launcher.config.hotkey = self.launcher.config.hotkey.next();
        self.persist_launcher_config(cx);
        self.tick_launcher(cx);
    }

    pub(crate) fn set_launcher_kind(&mut self, kind: LauncherKind, cx: &mut Context<Self>) {
        self.launcher.draft_kind = kind;
        self.launcher.draft_trusted = false;
        self.launcher.pending_script = None;
        cx.notify();
    }

    pub(crate) fn new_launcher_entry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.launcher.editing = None;
        self.launcher.draft_trusted = false;
        self.launcher.pending_script = None;
        self.set_launcher_draft_values("", "", "[]", window, cx);
    }

    fn set_launcher_draft_values(
        &mut self,
        name: &str,
        path: &str,
        args: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for (input, text) in [
            (&self.launcher.name, name),
            (&self.launcher.path, path),
            (&self.launcher.args, args),
        ] {
            if let Some(input) = input {
                input.update(cx, |input, cx| input.set_value(text.to_owned(), window, cx));
            }
        }
        cx.notify();
    }

    pub(crate) fn edit_launcher_entry(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self.launcher.config.entries.get(index).cloned() else {
            return;
        };
        self.launcher.editing = Some(index);
        self.launcher.draft_kind = entry.kind;
        // Editing requires a fresh explicit trust acknowledgement before saving.
        self.launcher.draft_trusted = false;
        self.launcher.pending_script = None;
        let args = serde_json::to_string(&entry.args).unwrap_or_else(|_| "[]".into());
        self.set_launcher_draft_values(&entry.name, &entry.path, &args, window, cx);
    }

    pub(crate) fn register_launcher_entry(&mut self, cx: &mut Context<Self>) {
        let text = |input: &Option<Entity<InputState>>| {
            input
                .as_ref()
                .map(|v| v.read(cx).value().to_string())
                .unwrap_or_default()
        };
        let name = text(&self.launcher.name).trim().to_owned();
        let path = text(&self.launcher.path);
        let result: Result<LauncherEntry, Option<LauncherError>> = (|| {
            if name.is_empty() || name.len() > 160 {
                self.launcher.status = "표시 이름을 1~160바이트로 입력하세요.".into();
                return Err(None);
            }
            let args = quick_launcher::parse_args(&text(&self.launcher.args)).map_err(Some)?;
            let mut entry = LauncherEntry {
                name,
                path,
                kind: self.launcher.draft_kind,
                args,
                trusted: self.launcher.draft_trusted,
            };
            entry.path = quick_launcher::validate_entry(&entry)
                .map_err(Some)?
                .to_string_lossy()
                .into_owned();
            Ok(entry)
        })();
        match result {
            Ok(entry) => {
                let index = if let Some(index) = self
                    .launcher
                    .editing
                    .filter(|&i| i < self.launcher.config.entries.len())
                {
                    self.launcher.config.entries[index] = entry;
                    index
                } else {
                    if self.launcher.config.entries.len() >= 200 {
                        self.launcher.failed = true;
                        self.launcher.status = "최대 200개까지 등록할 수 있습니다.".into();
                        cx.notify();
                        return;
                    }
                    self.launcher.config.entries.push(entry);
                    self.launcher.config.entries.len() - 1
                };
                self.launcher.editing = Some(index);
                self.launcher.selected = Some(index);
                self.launcher.draft_trusted = false;
                self.launcher.pending_script = None;
                self.launcher.failed = false;
                self.launcher.status =
                    "신뢰 대상 등록 완료 · 실행하려면 목록에서 선택 후 실행하세요.".into();
                self.persist_launcher_config(cx);
            }
            Err(error) => {
                self.launcher.failed = true;
                if let Some(error) = error {
                    self.launcher.status = error.to_string();
                }
            }
        }
        cx.notify();
    }

    pub(crate) fn delete_launcher_entry(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.launcher.config.entries.len() {
            return;
        }
        self.launcher.config.entries.remove(index);
        self.launcher.selected = None;
        self.new_launcher_entry(window, cx);
        self.launcher.status = "등록 항목 삭제 완료 · 대상 파일은 삭제하지 않았습니다.".into();
        self.launcher.failed = false;
        self.persist_launcher_config(cx);
    }

    pub(crate) fn browse_launcher_path(
        &mut self,
        folder: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: !folder,
            directories: folder,
            multiple: false,
            prompt: Some("신뢰할 실행 대상 선택".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = match receiver.await {
                Ok(Ok(paths)) => Ok(paths),
                _ => Err(()),
            };
            let _ = this.update_in(cx, |this, window, cx| {
                this.apply_launcher_path_result(folder, result, window, cx);
            });
        })
        .detach();
    }

    // Keep native dialog transport separate from the testable GPUI result handler.
    fn apply_launcher_path_result(
        &mut self,
        folder: bool,
        result: Result<Option<Vec<PathBuf>>, ()>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let path = match result {
            Ok(Some(paths)) => match paths.into_iter().next() {
                Some(path) => path,
                None => return,
            },
            Ok(None) => return,
            Err(()) => {
                self.launcher.failed = true;
                self.launcher.status =
                    "파일 선택기를 열지 못했습니다. 절대 경로를 직접 입력할 수 있습니다.".into();
                cx.notify();
                return;
            }
        };
        self.launcher.draft_trusted = false;
        self.launcher.pending_script = None;
        self.launcher.draft_kind = if folder {
            LauncherKind::Folder
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ps1"))
        {
            LauncherKind::Script
        } else {
            LauncherKind::Application
        };
        if let Some(input) = &self.launcher.path {
            input.update(cx, |state, cx| {
                state.set_value(path.to_string_lossy().into_owned(), window, cx)
            });
        }
        cx.notify();
    }

    pub(crate) fn request_launcher_run(&mut self, cx: &mut Context<Self>) {
        if self.launcher.is_busy() {
            return;
        }
        let Some(entry) = self
            .launcher
            .selected
            .and_then(|i| self.launcher.config.entries.get(i))
            .cloned()
        else {
            self.launcher.failed = true;
            self.launcher.status = "실행할 등록 항목을 먼저 선택하세요.".into();
            cx.notify();
            return;
        };
        if let Err(error) = quick_launcher::validate_entry(&entry) {
            self.launcher.failed = true;
            self.launcher.status = error.to_string();
        } else if entry.kind == LauncherKind::Script {
            self.launcher.pending_script = Some(entry);
            self.launcher.failed = false;
            self.launcher.status = "스크립트의 내용과 인수를 확인한 뒤 실행을 승인하세요.".into();
        } else {
            self.start_launcher_entry(entry, false, cx);
        }
        cx.notify();
    }

    pub(crate) fn confirm_launcher_script(&mut self, confirm: bool, cx: &mut Context<Self>) {
        let Some(entry) = self.launcher.pending_script.take() else {
            return;
        };
        if confirm && self.launcher.config.entries.contains(&entry) {
            self.start_launcher_entry(entry, true, cx);
        } else {
            self.launcher.status =
                "스크립트 실행을 취소했습니다. 아무 대상도 시작하지 않았습니다.".into();
            self.launcher.failed = false;
        }
        cx.notify();
    }

    fn start_launcher_entry(
        &mut self,
        entry: LauncherEntry,
        confirmed: bool,
        cx: &mut Context<Self>,
    ) {
        #[cfg(test)]
        if !self.sync.external_side_effects_enabled {
            self.launcher.status = "테스트 모드 · 외부 실행 차단".into();
            cx.notify();
            return;
        }
        let (tx, rx) = mpsc::channel();
        // Validate files and start processes outside the UI thread.
        match std::thread::Builder::new()
            .name("launcher-start".into())
            .spawn(move || {
                let result = quick_launcher::plan(&entry, confirmed).and_then(|plan| plan.spawn());
                let _ = tx.send(result);
            }) {
            Ok(_) => {
                self.launcher.launch_result = Some(rx);
                self.launcher.failed = false;
                self.launcher.status = "실행 요청 처리 중…".into();
            }
            Err(_) => {
                self.launcher.failed = true;
                self.launcher.status = LauncherError::StartFailed.to_string();
            }
        }
        cx.notify();
    }

    /// 메인 200ms 타이머에서 호출한다. 패널 진입 여부와 무관하게 opt-in을 복원한다.
    pub(crate) fn tick_launcher(&mut self, cx: &mut Context<Self>) {
        let desired = (self.launcher.config.enabled, self.launcher.config.hotkey);
        #[cfg(test)]
        let side_effects = self.sync.external_side_effects_enabled;
        #[cfg(not(test))]
        let side_effects = true;
        if self.launcher.applied_hotkey != Some(desired) {
            self.launcher.hotkey = None;
            self.launcher.applied_hotkey = Some(desired);
            self.launcher.hotkey_failed = false;
            if desired.0 && side_effects {
                match HotkeyRegistration::start(desired.1) {
                    Ok(registration) => {
                        self.launcher.hotkey = Some(registration);
                        self.launcher.hotkey_status = "단축키 등록 중…".into();
                    }
                    Err(_) => {
                        self.launcher.hotkey_failed = true;
                        self.launcher.hotkey_status =
                            "단축키 메시지 스레드를 시작하지 못했습니다.".into();
                    }
                }
            } else {
                self.launcher.hotkey_status = if desired.0 {
                    "테스트 모드 · OS 등록 차단"
                } else {
                    "전역 단축키 꺼짐"
                }
                .into();
            }
            cx.notify();
        }
        let events: Vec<_> = self
            .launcher
            .hotkey
            .as_ref()
            .map(|r| r.events.try_iter().collect())
            .unwrap_or_default();
        for event in events {
            self.handle_launcher_hotkey(event, cx);
        }
        if let Some(receiver) = &self.launcher.launch_result {
            let result = match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(LauncherError::StartFailed)),
                Err(mpsc::TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                self.launcher.launch_result = None;
                match result {
                    Ok(child) => {
                        self.launcher.children.push(child);
                        self.launcher.failed = false;
                        self.launcher.status =
                            "실행 시작 완료 · 종료 결과는 실행한 앱에서 확인하세요.".into();
                    }
                    Err(error) => {
                        self.launcher.failed = true;
                        self.launcher.status = error.to_string();
                    }
                }
                cx.notify();
            }
        }
        self.launcher
            .children
            .retain_mut(|child| match child.try_wait() {
                Ok(None) => true,
                Ok(Some(_)) => false,
                Err(_) => {
                    log::warn!("빠른 실행기 자체 시작 프로세스 상태 확인 실패");
                    false
                }
            });
    }

    fn handle_launcher_hotkey(&mut self, event: HotkeyEvent, cx: &mut Context<Self>) {
        match event {
            HotkeyEvent::Ready => {
                self.launcher.hotkey_status = "단축키 등록됨 · 빠른 실행기만 엽니다.".into()
            }
            HotkeyEvent::Failed(code) => {
                self.launcher.hotkey_failed = true;
                self.launcher.hotkey_status = format!("단축키 등록 실패 (OS {code}) · 충돌 시 조합 변경 또는 껐다 켜기로 재시도하세요.");
            }
            HotkeyEvent::Released => self.launcher.hotkey_status = "단축키 해제 완료".into(),
            HotkeyEvent::Activated => {
                self.active_panel = ActivePanel::QuickLauncher;
                // 단축키는 탐색만 수행하며 등록 대상은 절대 자동 실행하지 않는다.
                #[cfg(not(test))]
                {
                    #[cfg(windows)]
                    quick_launcher::platform::restore_own_main_window();
                    // This application has one main GPUI window; Root wraps AppRoot.
                    if let Some(window) = cx.windows().into_iter().next() {
                        let _ = cx.update_window(window, |_, window, cx| {
                            window.activate_window();
                            cx.activate(true);
                        });
                    }
                }
            }
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
