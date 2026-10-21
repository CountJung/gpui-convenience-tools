use super::*;

impl AppRoot {
    pub(crate) fn select_screenshot_monitor(
        &mut self,
        device_id: &str,
        topology: &[String],
        cx: &mut Context<Self>,
    ) {
        if topology != platform::topology(&self.screenshot.monitors) {
            self.screenshot_error(ScreenshotError::MonitorChanged);
            self.persist_screenshot_config(cx);
            return;
        }
        let index = self
            .screenshot
            .monitors
            .iter()
            .position(|monitor| monitor.device_id == device_id);
        match index {
            Some(index) => self.set_screenshot_target(CaptureTarget::Monitor(index), cx),
            None => {
                self.screenshot_error(ScreenshotError::MonitorChanged);
                self.persist_screenshot_config(cx);
            }
        }
    }

    pub(super) fn screenshot_capture_request(
        &self,
    ) -> Result<platform::CaptureRequest, ScreenshotError> {
        let binding = self.screenshot.config.monitor_binding.as_ref();
        let request = platform::CaptureRequest {
            target: self.screenshot.config.target,
            device_id: binding.map(|b| b.device_id.clone()),
            topology: match self.screenshot.config.target {
                CaptureTarget::AllDisplays => platform::topology(&self.screenshot.monitors),
                CaptureTarget::Monitor(_) => binding
                    .ok_or(ScreenshotError::MonitorChanged)?
                    .topology
                    .clone(),
            },
        };
        platform::resolve_capture_rect(&request, &self.screenshot.monitors)?;
        Ok(request)
    }

    pub(super) fn apply_screenshot_monitors(
        &mut self,
        monitors: Vec<platform::Monitor>,
        cx: &mut Context<Self>,
    ) {
        self.screenshot.monitors = monitors;
        if matches!(self.screenshot.config.target, CaptureTarget::Monitor(_))
            && self.screenshot_capture_request().is_err()
        {
            self.screenshot_error(ScreenshotError::MonitorChanged);
            self.persist_screenshot_config(cx);
        }
    }
}
