//! 신뢰한 실행 대상과 opt-in 단축키만 저장한다. 검색어·편집 초안은 저장하지 않는다.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LauncherConfig {
    pub entries: Vec<LauncherEntry>,
    /// 전역 단축키 활성화. 앱 시작만으로 실행 대상을 시작하지 않는다.
    pub enabled: bool,
    pub hotkey: LauncherHotkey,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LauncherHotkey {
    #[default]
    CtrlAltSpace,
    // F12 is debugger-reserved. Preserve old settings while writing only the new key name.
    #[serde(alias = "ctrl_shift_f12")]
    CtrlAltF9,
}

impl LauncherHotkey {
    pub fn label(self) -> &'static str {
        match self {
            Self::CtrlAltSpace => "Ctrl+Alt+Space",
            Self::CtrlAltF9 => "Ctrl+Alt+F9",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::CtrlAltSpace => Self::CtrlAltF9,
            Self::CtrlAltF9 => Self::CtrlAltSpace,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LauncherKind {
    #[default]
    Application,
    Folder,
    Script,
}

impl LauncherKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Application => "앱 (.exe)",
            Self::Folder => "폴더",
            Self::Script => "스크립트 (.ps1)",
        }
    }
}

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LauncherEntry {
    pub name: String,
    pub path: String,
    pub kind: LauncherKind,
    pub args: Vec<String>,
    pub trusted: bool,
}

// 경로와 인수가 AppConfig 디버그 로그에 유출되지 않게 한다.
impl std::fmt::Debug for LauncherEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LauncherEntry")
            .field("kind", &self.kind)
            .field("trusted", &self.trusted)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_default_off_legacy_and_roundtrip() {
        assert_eq!(
            serde_json::from_str::<LauncherConfig>("{}").unwrap(),
            LauncherConfig::default()
        );
        assert!(!LauncherConfig::default().enabled);
        let config = LauncherConfig {
            enabled: true,
            hotkey: LauncherHotkey::CtrlAltF9,
            entries: vec![LauncherEntry {
                name: "fixture-private".into(),
                path: "private-path".into(),
                args: vec!["secret".into()],
                trusted: true,
                ..Default::default()
            }],
        };
        assert_eq!(
            serde_json::from_str::<LauncherConfig>(&serde_json::to_string(&config).unwrap())
                .unwrap(),
            config
        );
        assert!(!format!("{config:?}").contains("private"));
        let partial: LauncherEntry = serde_json::from_str(r#"{"name":"old"}"#).unwrap();
        assert!(!partial.trusted);
    }

    #[test]
    fn launcher_legacy_f12_migrates_without_losing_entries_or_opt_in() {
        let mut original = serde_json::to_value(LauncherConfig {
            enabled: true,
            hotkey: LauncherHotkey::CtrlAltF9,
            entries: vec![LauncherEntry {
                name: "legacy fixture".into(),
                path: r"C:\fixture\app.exe".into(),
                args: vec!["literal argument".into()],
                trusted: true,
                ..Default::default()
            }],
        })
        .unwrap();
        original["hotkey"] = "ctrl_shift_f12".into();
        let migrated: LauncherConfig = serde_json::from_value(original.clone()).unwrap();
        assert!(migrated.enabled);
        assert_eq!(migrated.hotkey, LauncherHotkey::CtrlAltF9);
        assert_eq!(migrated.hotkey.label(), "Ctrl+Alt+F9");
        assert_eq!(LauncherHotkey::default().next(), migrated.hotkey);
        assert_eq!(migrated.hotkey.next(), LauncherHotkey::default());
        let saved = serde_json::to_value(&migrated).unwrap();
        assert_eq!(saved["entries"], original["entries"]);
        assert_eq!(saved["hotkey"], "ctrl_alt_f9");
        assert_eq!(
            serde_json::from_value::<LauncherConfig>(saved).unwrap(),
            migrated
        );
    }
}
