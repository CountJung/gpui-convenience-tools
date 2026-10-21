use super::AppRoot;
use crate::config::{update_config, TimestampConfig};
use gpui::{AppContext, ClipboardItem, Context, Entity, ScrollHandle, Window};
use gpui_component::input::InputState;

#[derive(Default)]
pub(crate) struct TimestampState {
    pub config: TimestampConfig,
    pub epoch: Option<Entity<InputState>>,
    pub date: Option<Entity<InputState>>,
    pub status: String,
    pub failed: bool,
    pub save_error: bool,
    pub scroll: ScrollHandle,
}
impl TimestampState {
    pub fn from_config(config: TimestampConfig) -> Self {
        Self {
            config,
            ..Default::default()
        }
    }
}
impl AppRoot {
    pub(crate) fn ensure_timestamp_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.timestamp.epoch.is_some() {
            return;
        }
        self.timestamp.epoch =
            Some(cx.new(|cx| InputState::new(window, cx).default_value("1792461476.171")));
        self.timestamp.date =
            Some(cx.new(|cx| {
                InputState::new(window, cx).placeholder("2026. 10. 20. 오전 10:57:56.171")
            }));
    }
    pub(crate) fn convert_timestamp(
        &mut self,
        from_epoch: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = if from_epoch {
            &self.timestamp.epoch
        } else {
            &self.timestamp.date
        };
        let text = input
            .as_ref()
            .map(|i| i.read(cx).value().to_string())
            .unwrap_or_default();
        let result = if from_epoch {
            crate::timestamp::epoch_to_date(&text, self.timestamp.config.utc)
        } else {
            crate::timestamp::date_to_epoch(&text, self.timestamp.config.utc)
        };
        match result {
            Ok(text) => {
                let output = if from_epoch {
                    &self.timestamp.date
                } else {
                    &self.timestamp.epoch
                };
                if let Some(output) = output {
                    output.update(cx, |i, cx| i.set_value(text, window, cx));
                }
                self.timestamp.failed = false;
                self.timestamp.status = "변환 완료 · 소수 초 정밀도 유지".into();
            }
            Err(error) => {
                self.timestamp.failed = true;
                self.timestamp.status = error.to_string();
            }
        }
        cx.notify();
    }
    pub(crate) fn set_timestamp_utc(&mut self, utc: bool, cx: &mut Context<Self>) {
        self.timestamp.config.utc = utc;
        self.timestamp.status = "시간대 변경 · 원하는 변환 방향 버튼을 누르세요.".into();
        self.timestamp.failed = false;
        #[cfg(test)]
        if !self.sync.external_side_effects_enabled {
            cx.notify();
            return;
        }
        let config = self.timestamp.config.clone();
        self.timestamp.save_error = update_config(|cfg| cfg.timestamp = config).is_err();
        if self.timestamp.save_error {
            log::error!("타임스탬프 시간대 설정 저장 실패");
        }
        cx.notify();
    }
    pub(crate) fn timestamp_now(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = &self.timestamp.epoch {
            let millis = chrono::Utc::now().timestamp_millis();
            input.update(cx, |i, cx| {
                i.set_value(
                    format!("{}.{:03}", millis.div_euclid(1000), millis.rem_euclid(1000)),
                    window,
                    cx,
                )
            });
        }
        self.convert_timestamp(true, window, cx);
    }
    pub(crate) fn copy_timestamp(&mut self, epoch: bool, cx: &mut Context<Self>) {
        let input = if epoch {
            &self.timestamp.epoch
        } else {
            &self.timestamp.date
        };
        if let Some(input) = input {
            cx.write_to_clipboard(ClipboardItem::new_string(
                input.read(cx).value().to_string(),
            ));
        }
        self.timestamp.status = "클립보드로 복사했습니다.".into();
        cx.notify();
    }
}
