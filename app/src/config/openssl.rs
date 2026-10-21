use serde::{Deserialize, Serialize};

/// 호환 작업에 필요한 설정만 저장한다. 암호 문구는 settings.json에 평문으로 기록된다.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OpenSslConfig {
    pub passphrase: String,
    pub append_newline: bool,
}

impl Default for OpenSslConfig {
    fn default() -> Self {
        Self {
            passphrase: "911".to_string(),
            append_newline: true,
        }
    }
}

// AppConfig의 Debug 출력에도 암호 문구를 노출하지 않는다.
impl std::fmt::Debug for OpenSslConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenSslConfig")
            .field("passphrase", &"[REDACTED]")
            .field("append_newline", &self.append_newline)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{load_config, update_config, AppConfig};

    #[test]
    fn openssl_settings_restore_nondefault_and_preserve_other_fields() {
        let _isolation = crate::config::tests::IsolatedDataDir::new("openssl");
        update_config(|cfg| {
            cfg.sidebar_width = 277.0;
            cfg.openssl.passphrase = "fixture-code-123".into();
            cfg.openssl.append_newline = false;
        })
        .unwrap();
        update_config(|cfg| cfg.scan_interval_secs = 27).unwrap();
        let restored = load_config().unwrap().unwrap();
        assert_eq!(restored.openssl.passphrase, "fixture-code-123");
        assert!(!restored.openssl.append_newline);
        assert_eq!(restored.sidebar_width, 277.0);
        assert_eq!(restored.scan_interval_secs, 27);
        let serialized = serde_json::to_value(&restored).unwrap();
        assert_eq!(serialized["openssl"].as_object().unwrap().len(), 2);
        assert!(!format!("{restored:?}").contains("fixture-code-123"));
    }

    #[test]
    fn openssl_legacy_and_partial_settings_have_compatible_defaults() {
        let old: AppConfig =
            serde_json::from_str(r#"{"service_enabled":false,"targets":[]}"#).unwrap();
        assert_eq!(old.openssl, OpenSslConfig::default());
        let partial: OpenSslConfig = serde_json::from_str(r#"{"passphrase":""}"#).unwrap();
        assert_eq!(partial.passphrase, "");
        assert!(partial.append_newline);
    }
}
