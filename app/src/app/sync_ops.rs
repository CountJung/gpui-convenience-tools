//! 파일 동기화 작업 조작.
//!
//! 작업은 목록 인덱스가 아니라 [`SyncJob::id`]로 식별한다.
//! 인덱스는 추가·삭제로 밀리기 때문에 백그라운드의 실행 주기 추적과
//! 결과 매칭이 어긋난다.

use gpui::{Context, PathPromptOptions, Window};
use gpui_component::notification::NotificationType;

use super::AppRoot;
use crate::config::{update_config, SyncJob};

impl AppRoot {
    // ─────────────────────────────────────────────
    // 파일 동기화
    // ─────────────────────────────────────────────

    pub(super) fn persist_sync_jobs(&mut self) {
        let jobs = self.sync_jobs.clone();

        if let Ok(mut state) = self.sync_state.lock() {
            state.jobs = jobs.clone();
        }

        if let Err(err) = update_config(move |cfg| cfg.sync_jobs = jobs) {
            log::error!("동기화 작업 저장 실패: {err}");
        }
    }

    pub(crate) fn add_sync_job(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_jobs.push(SyncJob::default());
        let index = self.sync_jobs.len() - 1;
        self.persist_sync_jobs();
        self.select_sync_job(index, window, cx);
        self.push_log("INFO", "동기화 작업을 추가했습니다.".to_string());
        cx.notify();
    }

    pub(crate) fn remove_sync_job(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.sync_jobs.len() {
            return;
        }

        let label = self.sync_jobs[index].label();
        let removed = self.sync_jobs.remove(index);
        self.sync_status.remove(&removed.id);

        self.selected_sync_job = if self.sync_jobs.is_empty() {
            None
        } else {
            Some(index.min(self.sync_jobs.len() - 1))
        };

        self.persist_sync_jobs();
        if let Some(next) = self.selected_sync_job {
            self.load_sync_inputs(next, window, cx);
        }
        self.push_log("INFO", format!("동기화 작업을 삭제했습니다: {label}"));
        cx.notify();
    }

    pub(crate) fn select_sync_job(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if index >= self.sync_jobs.len() {
            return;
        }
        self.selected_sync_job = Some(index);
        self.load_sync_inputs(index, window, cx);
        cx.notify();
    }

    /// 선택한 작업의 값을 입력 위젯에 채운다.
    pub(super) fn load_sync_inputs(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(job) = self.sync_jobs.get(index).cloned() else {
            return;
        };

        if let Some(input) = self.sync_name_input.as_ref() {
            input.update(cx, |state, cx| state.set_value(job.name.clone(), window, cx));
        }
        if let Some(input) = self.sync_source_input.as_ref() {
            input.update(cx, |state, cx| {
                state.set_value(job.source.clone(), window, cx)
            });
        }
        if let Some(input) = self.sync_target_input.as_ref() {
            input.update(cx, |state, cx| {
                state.set_value(job.target.clone(), window, cx)
            });
        }
    }

    /// 선택한 작업을 수정하고 저장한다.
    pub(crate) fn update_selected_sync_job(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut SyncJob),
    ) {
        let Some(index) = self.selected_sync_job else {
            return;
        };
        let Some(job) = self.sync_jobs.get_mut(index) else {
            return;
        };
        edit(job);
        self.persist_sync_jobs();
        cx.notify();
    }

    /// 입력창의 경로 텍스트를 선택한 작업에 반영한다.
    pub(crate) fn apply_sync_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.selected_sync_job else {
            return;
        };

        let source = self
            .sync_source_input
            .as_ref()
            .map(|i| i.read(cx).value().to_string())
            .unwrap_or_default();
        let target = self
            .sync_target_input
            .as_ref()
            .map(|i| i.read(cx).value().to_string())
            .unwrap_or_default();

        if let Some(job) = self.sync_jobs.get_mut(index) {
            job.source = source.trim().to_string();
            job.target = target.trim().to_string();
        }

        self.persist_sync_jobs();
        self.push_log("INFO", "동기화 경로를 저장했습니다.".to_string());
        self.notify_toast("경로를 저장했습니다", NotificationType::Success, window, cx);
        cx.notify();
    }

    /// 네이티브 폴더 선택 대화상자를 띄워 경로를 채운다.
    pub(crate) fn pick_sync_folder(
        &mut self,
        is_source: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_sync_job.is_none() {
            return;
        }

        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(if is_source {
                "원본 폴더 선택".into()
            } else {
                "대상 폴더 선택".into()
            }),
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
                let Some(index) = this.selected_sync_job else {
                    return;
                };

                if let Some(job) = this.sync_jobs.get_mut(index) {
                    if is_source {
                        job.source = picked.clone();
                    } else {
                        job.target = picked.clone();
                    }
                }

                let input = if is_source {
                    this.sync_source_input.clone()
                } else {
                    this.sync_target_input.clone()
                };
                if let Some(input) = input {
                    input.update(cx, |state, cx| state.set_value(picked.clone(), window, cx));
                }

                this.persist_sync_jobs();
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn request_sync_job(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.sync_jobs.get(index).map(|job| job.id.clone()) else {
            return;
        };

        if let Ok(mut state) = self.sync_state.lock() {
            if !state.run_now.contains(&id) {
                state.run_now.push(id);
            }
        }
        self.notify_toast("동기화를 시작했습니다", NotificationType::Info, window, cx);
        cx.notify();
    }

    pub(crate) fn request_sync_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.sync_jobs.len();
        if count == 0 {
            self.notify_toast(
                "등록된 동기화 작업이 없습니다",
                NotificationType::Warning,
                window,
                cx,
            );
            return;
        }

        if let Ok(mut state) = self.sync_state.lock() {
            state.run_now = self.sync_jobs.iter().map(|job| job.id.clone()).collect();
        }
        self.notify_toast(
            "전체 동기화를 시작했습니다",
            NotificationType::Info,
            window,
            cx,
        );
        cx.notify();
    }

    pub(crate) fn toggle_sync_failure_suppression(&mut self, key: &str) {
        if !self.suppressed_sync_failures.remove(key) {
            self.suppressed_sync_failures.insert(key.to_string());
        }
    }
}
