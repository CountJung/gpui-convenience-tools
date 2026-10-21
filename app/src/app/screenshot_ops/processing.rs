//! 픽셀 크기에 비례하는 crop·BGRA 변환은 UI가 아닌 background executor에서 수행한다.
use super::*;

pub(super) struct SelectedScreenshot {
    pub image: Arc<ScreenshotImage>,
    pub rect: Option<PixelRect>,
}

impl SelectedScreenshot {
    pub fn materialize(self) -> Result<Arc<ScreenshotImage>, ScreenshotError> {
        match self.rect {
            Some(rect) => self.image.crop(rect).map(Arc::new),
            None => Ok(self.image),
        }
    }
}

pub(super) struct PreparedScreenshot {
    pub image: Arc<ScreenshotImage>,
    pub preview: Arc<RenderImage>,
}

impl PreparedScreenshot {
    pub fn new(image: Arc<ScreenshotImage>) -> Result<Self, ScreenshotError> {
        let preview = image.render_image()?;
        Ok(Self { image, preview })
    }
}

impl AppRoot {
    pub(super) fn prepare_screenshot_crop(
        &mut self,
        selected: SelectedScreenshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.screenshot.busy = true;
        self.screenshot.failed = false;
        self.screenshot.status = "선택 영역 자르기 중…".into();
        let generation = self.screenshot.generation;
        let task = cx
            .background_executor()
            .spawn(async move { PreparedScreenshot::new(selected.materialize()?) });
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.screenshot.generation != generation {
                    return;
                }
                this.screenshot.busy = false;
                match result {
                    Ok(prepared) => this.apply_screenshot(prepared, window, cx),
                    Err(error) => this.screenshot_error(error),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
