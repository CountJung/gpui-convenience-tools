use anyhow::Result;
use gpui_component::theme::ThemeMode;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

use crate::app::TargetApp;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub service_enabled: bool,
    pub targets: Vec<TargetApp>,
    #[serde(default)]
    pub light_theme_name: Option<String>,
    #[serde(default)]
    pub dark_theme_name: Option<String>,
    #[serde(default = "default_scan_interval_secs")]
    pub scan_interval_secs: u32,
    #[serde(default)]
    pub favorite_services: Vec<String>,
}

fn default_scan_interval_secs() -> u32 {
    10
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            service_enabled: true,
            targets: vec![TargetApp {
                process_name: "KakaoTalk.exe".to_string(),
                display_name: "KakaoTalk".to_string(),
                enabled: true,
                ad_window_class: "Chrome_WidgetWin_1".to_string(),
            }],
            light_theme_name: None,
            dark_theme_name: None,
            scan_interval_secs: default_scan_interval_secs(),
            favorite_services: Vec::new(),
        }
    }
}

const BUNDLED_THEMES: [(&str, &str); 21] = [
    (
        "adventure.json",
        include_str!("../assets/themes/adventure.json"),
    ),
    ("alduin.json", include_str!("../assets/themes/alduin.json")),
    (
        "asciinema.json",
        include_str!("../assets/themes/asciinema.json"),
    ),
    ("ayu.json", include_str!("../assets/themes/ayu.json")),
    (
        "tokyonight.json",
        include_str!("../assets/themes/tokyonight.json"),
    ),
    (
        "catppuccin.json",
        include_str!("../assets/themes/catppuccin.json"),
    ),
    (
        "gruvbox.json",
        include_str!("../assets/themes/gruvbox.json"),
    ),
    (
        "everforest.json",
        include_str!("../assets/themes/everforest.json"),
    ),
    (
        "flexoki.json",
        include_str!("../assets/themes/flexoki.json"),
    ),
    (
        "fahrenheit.json",
        include_str!("../assets/themes/fahrenheit.json"),
    ),
    ("harper.json", include_str!("../assets/themes/harper.json")),
    ("hybrid.json", include_str!("../assets/themes/hybrid.json")),
    (
        "jellybeans.json",
        include_str!("../assets/themes/jellybeans.json"),
    ),
    ("kibble.json", include_str!("../assets/themes/kibble.json")),
    (
        "macos-classic.json",
        include_str!("../assets/themes/macos-classic.json"),
    ),
    ("matrix.json", include_str!("../assets/themes/matrix.json")),
    (
        "mellifluous.json",
        include_str!("../assets/themes/mellifluous.json"),
    ),
    ("molokai.json", include_str!("../assets/themes/molokai.json")),
    (
        "solarized.json",
        include_str!("../assets/themes/solarized.json"),
    ),
    (
        "spaceduck.json",
        include_str!("../assets/themes/spaceduck.json"),
    ),
    ("twilight.json", include_str!("../assets/themes/twilight.json")),
];

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gpui-convenience-tools")
        .join("config.json")
}

pub fn themes_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("gpui-convenience-tools")
        .join("themes")
}

pub fn ensure_bundled_themes() -> Result<PathBuf> {
    let path = themes_path();
    fs::create_dir_all(&path)?;

    for (name, contents) in BUNDLED_THEMES {
        let target = path.join(name);
        if !target.exists() {
            fs::write(target, contents)?;
        }
    }

    Ok(path)
}

pub fn save_config(config: &AppConfig) -> Result<()> {
    let path = config_path();

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(config)?;
    fs::write(path, json)?;
    Ok(())
}

pub fn load_config() -> Result<Option<AppConfig>> {
    let path = config_path();

    if !path.exists() {
        return Ok(None);
    }

    let data = fs::read_to_string(path)?;
    let config = serde_json::from_str::<AppConfig>(&data)?;
    Ok(Some(config))
}

pub fn save_theme_selection(mode: ThemeMode, theme_name: &str) -> Result<()> {
    let mut config = load_config()?.unwrap_or_default();
    match mode {
        ThemeMode::Light => config.light_theme_name = Some(theme_name.to_string()),
        ThemeMode::Dark => config.dark_theme_name = Some(theme_name.to_string()),
    }
    save_config(&config)
}



