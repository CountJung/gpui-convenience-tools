//! 캡처 동작 설정만 영속화한다. 이미지·선택 영역·파일 이력은 저장하지 않는다.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureTarget {
    #[default]
    AllDisplays,
    Monitor(usize),
}

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScreenshotConfig {
    pub target: CaptureTarget,
    pub save_folder: String,
    /// 사용자가 명시적으로 켠 경우에만 수동 캡처 직후 PNG를 저장한다.
    pub auto_save: bool,
    /// 구형 인덱스는 표시/호환용이다. 실제 단일 모니터 캡처에는 이 검증값이 필수다.
    pub monitor_binding: Option<MonitorBinding>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorBinding {
    pub device_id: String,
    pub topology: Vec<String>,
}

impl ScreenshotConfig {
    pub fn bind_monitor(&mut self, device_id: String, topology: Vec<String>) {
        self.monitor_binding = Some(MonitorBinding {
            device_id,
            topology,
        });
    }
}

impl std::fmt::Debug for ScreenshotConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScreenshotConfig")
            .field("target", &self.target)
            .field("save_folder", &"[REDACTED]")
            .field("auto_save", &self.auto_save)
            .field(
                "monitor_binding",
                &self.monitor_binding.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screenshot_config_updates_preserve_other_fields_and_legacy_loads() {
        let _isolation = crate::config::tests::IsolatedDataDir::new("screenshot-config");
        crate::config::update_config(|cfg| {
            cfg.scan_interval_secs = 39;
            cfg.screenshot.target = CaptureTarget::Monitor(1);
            cfg.screenshot.save_folder = "fixture-folder".into();
            cfg.screenshot.auto_save = true;
        })
        .unwrap();
        crate::config::update_config(|cfg| cfg.sidebar_width = 269.0).unwrap();
        let restored = crate::config::load_config().unwrap().unwrap();
        assert_eq!(restored.screenshot.target, CaptureTarget::Monitor(1));
        assert_eq!(restored.screenshot.save_folder, "fixture-folder");
        assert!(restored.screenshot.auto_save);
        assert_eq!(restored.scan_interval_secs, 39);
        assert_eq!(restored.sidebar_width, 269.0);
        let legacy: crate::config::AppConfig =
            serde_json::from_str(r#"{"service_enabled":false,"targets":[]}"#).unwrap();
        assert_eq!(legacy.screenshot, ScreenshotConfig::default());
    }

    #[test]
    fn screenshot_config_has_private_compatible_defaults() {
        let config: ScreenshotConfig = serde_json::from_str("{}").unwrap();
        assert!(!config.auto_save);
        assert_eq!(config.target, CaptureTarget::AllDisplays);
        assert!(config.save_folder.is_empty());
        let mut config = ScreenshotConfig {
            target: CaptureTarget::Monitor(2),
            save_folder: "private-folder".into(),
            auto_save: true,
            monitor_binding: None,
        };
        config.bind_monitor("fixture-device-id".into(), vec!["fixture-topology".into()]);
        let json = serde_json::to_string(&config).unwrap();
        assert_eq!(
            serde_json::from_str::<ScreenshotConfig>(&json).unwrap(),
            config
        );
        assert!(!format!("{config:?}").contains("private-folder"));
        assert!(!format!("{config:?}").contains("fixture-device-id"));
        assert_eq!(
            serde_json::to_value(config)
                .unwrap()
                .as_object()
                .unwrap()
                .len(),
            4
        );
    }
}
