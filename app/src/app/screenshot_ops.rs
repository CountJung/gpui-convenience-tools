//! 수동 캡처·메모리 미리보기·선택·명시적 내보내기. 이미지 이력은 만들지 않는다.
mod export;
#[cfg(test)]
mod integration_tests;
mod monitor;
mod processing;
#[cfg(test)]
mod tests;
mod validation;

use super::AppRoot;
use crate::config::{update_config, CaptureTarget, ScreenshotConfig};
use crate::screenshot_tool::{platform, PixelRect, PreviewRect, ScreenshotError, ScreenshotImage};
use gpui::{
    AppContext, Bounds, Context, Entity, Pixels, Point, RenderImage, ScrollHandle, Subscription,
    Window,
};
use gpui_component::input::{InputEvent, InputState};
use processing::{PreparedScreenshot, SelectedScreenshot};
use std::{cell::Cell, rc::Rc, sync::Arc};

#[derive(Default)]
pub(crate) struct ScreenshotState {
    pub config: ScreenshotConfig,
    pub save_error: bool,
    pub scroll: ScrollHandle,
    pub folder_input: Option<Entity<InputState>>,
    folder_subscription: Option<Subscription>,
    pub monitors: Vec<platform::Monitor>,
    pub image: Option<Arc<ScreenshotImage>>,
    pub preview: Option<Arc<RenderImage>>,
    pub preview_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    pub painted_image_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    pub selection: Option<PixelRect>,
    selection_invalid: bool,
    drag_start: Option<(f32, f32)>,
    pub status: String,
    pub failed: bool,
    pub busy: bool,
    generation: u64,
    validation_seed_attempted: bool,
}

impl ScreenshotState {
    pub fn from_config(config: ScreenshotConfig) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }

    pub fn image_rect(&self) -> Option<PreviewRect> {
        let image = self.image.as_ref()?;
        let bounds = self.preview_bounds.get()?;
        PreviewRect {
            left: bounds.origin.x.into(),
            top: bounds.origin.y.into(),
            width: bounds.size.width.into(),
            height: bounds.size.height.into(),
        }
        .fit(image.width(), image.height())
    }
}

impl AppRoot {
    pub(crate) fn ensure_screenshot_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.screenshot.folder_input.is_some() {
            return;
        }
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(self.screenshot.config.save_folder.clone())
                .placeholder("비어 있으면 사진 폴더 · 자동 저장은 폴더를 명시해야 사용 가능")
        });
        self.screenshot.folder_subscription = Some(cx.subscribe_in(
            &input,
            window,
            |this: &mut Self, input, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.screenshot.config.save_folder = input.read(cx).value().to_string();
                    this.persist_screenshot_config(cx);
                }
            },
        ));
        self.screenshot.folder_input = Some(input);
        self.refresh_screenshot_monitors(cx);
    }

    pub(crate) fn persist_screenshot_config(&mut self, cx: &mut Context<Self>) {
        #[cfg(test)]
        if !self.sync.external_side_effects_enabled {
            cx.notify();
            return;
        }
        let config = self.screenshot.config.clone();
        self.screenshot.save_error = update_config(|cfg| cfg.screenshot = config).is_err();
        if self.screenshot.save_error {
            log::error!("스크린샷 설정 저장 실패");
        }
        cx.notify();
    }

    pub(crate) fn refresh_screenshot_monitors(&mut self, cx: &mut Context<Self>) {
        // 테스트는 사용자의 모니터 토폴로지까지 읽을 필요가 없다.
        #[cfg(test)]
        let result = Ok::<_, ScreenshotError>(vec![platform::Monitor {
            device_id: "fixture-monitor".into(),
            rect: platform::DesktopRect {
                left: 0,
                top: 0,
                width: 1920,
                height: 1080,
            },
            primary: true,
        }]);
        #[cfg(not(test))]
        let result = platform::monitors();
        match result {
            Ok(monitors) => {
                self.apply_screenshot_monitors(monitors, cx);
            }
            Err(error) => {
                self.screenshot_error(error);
                if error == ScreenshotError::MonitorChanged {
                    self.persist_screenshot_config(cx);
                }
            }
        }
        cx.notify();
    }

    pub(crate) fn set_screenshot_target(&mut self, target: CaptureTarget, cx: &mut Context<Self>) {
        self.screenshot.config.monitor_binding = None;
        if let CaptureTarget::Monitor(index) = target {
            let Some(monitor) = self.screenshot.monitors.get(index) else {
                self.screenshot_error(ScreenshotError::MonitorUnavailable);
                cx.notify();
                return;
            };
            if monitor.device_id.is_empty() {
                self.screenshot_error(ScreenshotError::MonitorChanged);
                cx.notify();
                return;
            }
            self.screenshot.config.bind_monitor(
                monitor.device_id.clone(),
                platform::topology(&self.screenshot.monitors),
            );
        }
        self.screenshot.config.target = target;
        self.screenshot.failed = false;
        self.screenshot.status = "캡처 대상 선택 완료 · 수동 캡처 버튼으로 실행".into();
        self.persist_screenshot_config(cx);
    }

    pub(crate) fn set_screenshot_auto_save(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.screenshot.config.auto_save = enabled;
        self.persist_screenshot_config(cx);
    }

    fn screenshot_error(&mut self, error: ScreenshotError) {
        if error == ScreenshotError::MonitorChanged {
            self.screenshot.config.monitor_binding = None;
        }
        self.screenshot.failed = true;
        self.screenshot.status = error.to_string();
        // 에러 타입의 고정 문구만 기록하며 이미지/경로/화면 내용은 기록하지 않는다.
        log::warn!("스크린샷 작업 실패: {error}");
    }

    pub(crate) fn capture_screenshot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.screenshot.busy {
            return;
        }
        let request = match self.screenshot_capture_request() {
            Ok(request) => request,
            Err(error) => {
                self.screenshot_error(error);
                self.persist_screenshot_config(cx);
                return;
            }
        };
        #[cfg(test)]
        {
            let _ = (window, request);
            self.screenshot_error(ScreenshotError::CaptureFailed);
            cx.notify();
        }
        #[cfg(not(test))]
        {
            self.screenshot.busy = true;
            self.screenshot.failed = false;
            self.screenshot.status = "수동 캡처 중…".into();
            self.screenshot.generation = self.screenshot.generation.wrapping_add(1);
            let generation = self.screenshot.generation;
            let task = cx.background_executor().spawn(async move {
                let image = Arc::new(platform::capture(request)?);
                PreparedScreenshot::new(image)
            });
            cx.spawn_in(window, async move |this, cx| {
                let result = task.await;
                let _ = this.update_in(cx, |this, window, cx| {
                    if this.screenshot.generation != generation {
                        return;
                    }
                    this.screenshot.busy = false;
                    match result {
                        Ok(prepared) => {
                            this.apply_screenshot(prepared, window, cx);
                            if !this.screenshot.failed
                                && this.screenshot.config.auto_save
                                && this.screenshot.image.is_some()
                            {
                                this.save_screenshot_default(window, cx);
                            }
                        }
                        Err(error) => {
                            this.screenshot_error(error);
                            if error == ScreenshotError::MonitorChanged {
                                this.persist_screenshot_config(cx);
                            }
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
            cx.notify();
        }
    }

    // 합성 fixture 전용 진입점. 제품 경로의 픽셀 준비는 processing 작업에서 수행한다.
    #[cfg(test)]
    pub(crate) fn install_screenshot(
        &mut self,
        image: ScreenshotImage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match PreparedScreenshot::new(Arc::new(image)) {
            Ok(prepared) => self.apply_screenshot(prepared, window, cx),
            Err(error) => self.screenshot_error(error),
        }
        cx.notify();
    }

    fn apply_screenshot(
        &mut self,
        prepared: PreparedScreenshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(old) = self.screenshot.preview.take() {
            cx.drop_image(old, Some(window));
        }
        self.screenshot.status = format!(
            "{} × {} px · 메모리에만 보관 · 드래그로 영역 선택",
            prepared.image.width(),
            prepared.image.height()
        );
        self.screenshot.image = Some(prepared.image);
        self.screenshot.preview = Some(prepared.preview);
        self.screenshot.selection = None;
        self.screenshot.selection_invalid = false;
        self.screenshot.drag_start = None;
        self.screenshot.failed = false;
        cx.notify();
    }

    pub(crate) fn begin_screenshot_selection(
        &mut self,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.screenshot.busy {
            return;
        }
        let point = (position.x.into(), position.y.into());
        if self
            .screenshot
            .image_rect()
            .is_some_and(|rect| rect.contains(point))
        {
            self.screenshot.drag_start = Some(point);
            self.screenshot.selection = None;
            self.screenshot.selection_invalid = false;
            cx.notify();
        }
    }

    pub(crate) fn update_screenshot_selection(
        &mut self,
        position: Point<Pixels>,
        finish: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(start) = self.screenshot.drag_start else {
            return;
        };
        if let (Some(rect), Some(image)) = (self.screenshot.image_rect(), &self.screenshot.image) {
            let end = (position.x.into(), position.y.into());
            self.screenshot.selection = rect
                .selection(start, end, image.width(), image.height())
                .ok();
            if finish {
                self.screenshot.drag_start = None;
                self.screenshot.failed = self.screenshot.selection.is_none();
                self.screenshot.selection_invalid = self.screenshot.selection.is_none();
                self.screenshot.status = self.screenshot.selection.map_or_else(
                    || ScreenshotError::EmptySelection.to_string(),
                    |r| {
                        format!(
                            "선택: {}, {} · {} × {} px · 저장/복사는 선택 영역만 사용",
                            r.x, r.y, r.width, r.height
                        )
                    },
                );
            }
            cx.notify();
        }
    }

    fn selected_screenshot(&self) -> Result<SelectedScreenshot, ScreenshotError> {
        if self.screenshot.selection_invalid || self.screenshot.drag_start.is_some() {
            return Err(ScreenshotError::EmptySelection);
        }
        let image = self
            .screenshot
            .image
            .as_ref()
            .ok_or(ScreenshotError::NoImage)?;
        // UI에서는 Arc와 좌표만 복사한다. crop은 내보내기/자르기 작업 스레드에서 실행한다.
        Ok(SelectedScreenshot {
            image: image.clone(),
            rect: self.screenshot.selection,
        })
    }

    pub(crate) fn crop_screenshot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.screenshot.busy {
            return;
        }
        if self.screenshot.selection.is_none() {
            self.screenshot_error(ScreenshotError::EmptySelection);
        } else {
            match self.selected_screenshot() {
                Ok(selected) => self.prepare_screenshot_crop(selected, window, cx),
                Err(error) => self.screenshot_error(error),
            }
        }
        cx.notify();
    }

    pub(crate) fn reset_screenshot_selection(&mut self, cx: &mut Context<Self>) {
        if self.screenshot.busy {
            return;
        }
        self.screenshot.selection = None;
        self.screenshot.selection_invalid = false;
        self.screenshot.drag_start = None;
        self.screenshot.status = "전체 이미지 선택 · 저장/복사는 전체 이미지를 사용".into();
        self.screenshot.failed = false;
        cx.notify();
    }

    pub(crate) fn clear_screenshot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.screenshot.busy {
            self.screenshot.status = "작업 완료 후 이미지를 지워 주세요. 이미 시작된 저장은 이 버튼으로 취소되지 않습니다.".into();
            cx.notify();
            return;
        }
        self.screenshot.generation = self.screenshot.generation.wrapping_add(1);
        self.screenshot.image = None;
        if let Some(old) = self.screenshot.preview.take() {
            cx.drop_image(old, Some(window));
        }
        self.screenshot.selection = None;
        self.screenshot.selection_invalid = false;
        self.screenshot.drag_start = None;
        self.screenshot.busy = false;
        self.screenshot.failed = false;
        self.screenshot.status =
            "메모리 이미지를 지웠습니다. 파일/클립보드는 변경하지 않았습니다.".into();
        cx.notify();
    }
}
