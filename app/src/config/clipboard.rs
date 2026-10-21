use serde::{Deserialize, Serialize};

/// 내용·검색어·선택 상태는 설정에 저장하지 않는다. 수집은 유형별 명시적 opt-in이다.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClipboardConfig {
    pub collect_text: bool,
    pub collect_images: bool,
    pub max_entries: usize,
    pub max_age_hours: u64,
    pub max_bytes: u64,
    pub excluded_phrases: Vec<String>,
    pub exclude_sensitive: bool,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            collect_text: false,
            collect_images: false,
            max_entries: 100,
            max_age_hours: 24,
            max_bytes: 8 * 1024 * 1024,
            excluded_phrases: Vec::new(),
            exclude_sensitive: true,
        }
    }
}

impl ClipboardConfig {
    pub fn normalized(mut self) -> Self {
        self.max_entries = self.max_entries.clamp(1, 500);
        self.max_age_hours = self.max_age_hours.clamp(1, 24 * 30);
        self.max_bytes = self.max_bytes.clamp(64 * 1024, 16 * 1024 * 1024);
        self.excluded_phrases = self
            .excluded_phrases
            .into_iter()
            .map(|phrase| phrase.trim().chars().take(256).collect::<String>())
            .filter(|phrase| !phrase.is_empty())
            .take(100)
            .collect();
        self
    }

    pub fn enabled(&self) -> bool {
        self.collect_text || self.collect_images
    }
}

impl std::fmt::Debug for ClipboardConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClipboardConfig")
            .field("collect_text", &self.collect_text)
            .field("collect_images", &self.collect_images)
            .field("max_entries", &self.max_entries)
            .field("max_age_hours", &self.max_age_hours)
            .field("max_bytes", &self.max_bytes)
            .field("excluded_phrases", &"[REDACTED]")
            .field("exclude_sensitive", &self.exclude_sensitive)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_config_legacy_limits_and_redaction() {
        let mut config: ClipboardConfig = serde_json::from_str("{}").unwrap();
        assert!(!config.enabled());
        config.max_entries = usize::MAX;
        config.max_age_hours = 0;
        config.max_bytes = u64::MAX;
        config.excluded_phrases = vec!["secret-fixture".into()];
        let config = config.normalized();
        assert_eq!(config.max_entries, 500);
        assert_eq!(config.max_age_hours, 1);
        assert_eq!(config.max_bytes, 16 * 1024 * 1024);
        assert!(!format!("{config:?}").contains("secret-fixture"));
        assert_eq!(
            serde_json::from_str::<ClipboardConfig>(&serde_json::to_string(&config).unwrap())
                .unwrap(),
            config
        );
    }
}
