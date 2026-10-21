use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessSort {
    #[default]
    Cpu,
    Memory,
}

/// Only preferences are persisted, never process names, paths or the search query.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProcessMonitorConfig {
    pub sort: ProcessSort,
    pub refresh_interval_secs: u32,
    pub top_count: usize,
}

impl Default for ProcessMonitorConfig {
    fn default() -> Self {
        Self {
            sort: ProcessSort::Cpu,
            refresh_interval_secs: 3,
            top_count: 50,
        }
    }
}

impl ProcessMonitorConfig {
    pub fn normalize(&mut self) {
        self.refresh_interval_secs = self.refresh_interval_secs.clamp(1, 60);
        self.top_count = self.top_count.clamp(10, 200);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_preferences_defaults_roundtrip_and_bounds() {
        let legacy: ProcessMonitorConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(legacy, ProcessMonitorConfig::default());
        let mut config = ProcessMonitorConfig {
            sort: ProcessSort::Memory,
            refresh_interval_secs: 0,
            top_count: usize::MAX,
        };
        config.normalize();
        assert_eq!(config.refresh_interval_secs, 1);
        assert_eq!(config.top_count, 200);
        let saved = serde_json::to_string(&config).unwrap();
        assert_eq!(
            serde_json::from_str::<ProcessMonitorConfig>(&saved).unwrap(),
            config
        );
        assert!(!saved.contains("query"));
    }

    #[test]
    fn process_preferences_isolated_store_preserves_other_fields() {
        let _isolation = crate::config::tests::IsolatedDataDir::new("process-monitor");
        crate::config::update_config(|cfg| {
            cfg.sidebar_width = 271.0;
            cfg.process_monitor = ProcessMonitorConfig {
                sort: ProcessSort::Memory,
                refresh_interval_secs: 11,
                top_count: 100,
            };
        })
        .unwrap();
        crate::config::update_config(|cfg| cfg.scan_interval_secs = 29).unwrap();
        let restored = crate::config::load_config().unwrap().unwrap();
        assert_eq!(restored.process_monitor.refresh_interval_secs, 11);
        assert_eq!(restored.process_monitor.sort, ProcessSort::Memory);
        assert_eq!(restored.process_monitor.top_count, 100);
        assert_eq!(restored.sidebar_width, 271.0);
        assert_eq!(restored.scan_interval_secs, 29);
    }
}
