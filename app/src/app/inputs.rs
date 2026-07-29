//! 입력 위젯(`InputState`) 준비와 값 동기화.
//!
//! GPUI 입력 위젯은 윈도우가 있어야 만들 수 있어 생성자가 아니라
//! 첫 렌더 시점에 지연 생성한다.

use gpui::{AppContext, Context, Entity, Window};
use gpui_component::input::{InputEvent, InputState};

use super::AppRoot;

impl AppRoot {
    // ─────────────────────────────────────────────
    // 입력 위젯 준비
    // ─────────────────────────────────────────────

    pub(crate) fn ensure_theme_filter_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.theme_filter_input.is_some() {
            return;
        }

        let initial_query = self.theme_filter_query.clone();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("테마 이름으로 검색")
                .default_value(initial_query)
        });

        let subscription = cx.subscribe(
            &input,
            |this: &mut Self, input: Entity<InputState>, ev: &InputEvent, cx| {
                if let InputEvent::Change = ev {
                    this.theme_filter_query = input.read(cx).value().to_string();
                    cx.notify();
                }
            },
        );

        self.theme_filter_input = Some(input);
        self.subscriptions.push(subscription);
    }

    pub(crate) fn set_theme_filter_query(
        &mut self,
        query: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.theme_filter_query = query.clone();

        if let Some(input) = self.theme_filter_input.as_ref() {
            input.update(cx, |state, cx| {
                state.set_value(query, window, cx);
            });
        }
    }

    pub(crate) fn ensure_service_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.service_search_input.is_some() {
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("서비스 이름 검색"));
        let subscription = cx.subscribe(
            &input,
            |this: &mut Self, input: Entity<InputState>, ev: &InputEvent, cx| {
                if let InputEvent::Change = ev {
                    this.service_search_query = input.read(cx).value().to_string();
                    cx.notify();
                }
            },
        );
        self.service_search_input = Some(input);
        self.subscriptions.push(subscription);
    }

    /// 동기화 작업 편집용 입력 3종을 준비한다.
    pub(crate) fn ensure_sync_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sync_name_input.is_some() {
            return;
        }

        let job = self
            .selected_sync_job
            .and_then(|ix| self.sync_jobs.get(ix))
            .cloned()
            .unwrap_or_default();

        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("작업 이름 (비우면 원본 폴더명)")
                .default_value(job.name.clone())
        });
        let source = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(r"예: D:\작업\원본")
                .default_value(job.source.clone())
        });
        let target = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(r"예: E:\백업\대상")
                .default_value(job.target.clone())
        });

        // 이름은 입력 즉시 반영한다(경로는 '경로 적용'으로 명시 저장).
        let name_sub = cx.subscribe(
            &name,
            |this: &mut Self, input: Entity<InputState>, ev: &InputEvent, cx| {
                if let InputEvent::Change = ev {
                    let value = input.read(cx).value().to_string();
                    if let Some(ix) = this.selected_sync_job {
                        if let Some(job) = this.sync_jobs.get_mut(ix) {
                            job.name = value;
                        }
                        this.persist_sync_jobs();
                        cx.notify();
                    }
                }
            },
        );

        self.sync_name_input = Some(name);
        self.sync_source_input = Some(source);
        self.sync_target_input = Some(target);
        self.subscriptions.push(name_sub);
    }
}
