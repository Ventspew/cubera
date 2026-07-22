use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub uuid: String,
    pub name: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub offline: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    pub memory_mb: u32,
    pub java_path: Option<String>,
    pub accounts: Vec<Account>,
    pub active_account: Option<String>,
    pub last_version: Option<String>,
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default)]
    pub jvm_args: String,
    #[serde(default = "default_ingame_branding")]
    pub ingame_branding: bool,
}

fn default_ingame_branding() -> bool {
    true
}

fn default_width() -> u32 {
    1280
}
fn default_height() -> u32 {
    720
}

pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Cubera")
}

pub fn instances_dir() -> PathBuf {
    data_dir().join("instances")
}

pub fn libraries_dir() -> PathBuf {
    data_dir().join("libraries")
}

pub fn assets_dir() -> PathBuf {
    data_dir().join("assets")
}

pub fn versions_dir() -> PathBuf {
    data_dir().join("versions")
}

pub fn java_dir() -> PathBuf {
    data_dir().join("java")
}

pub fn settings_path() -> PathBuf {
    data_dir().join("settings.json")
}

pub fn ensure_dirs() -> Result<(), String> {
    for dir in [
        data_dir(),
        instances_dir(),
        libraries_dir(),
        assets_dir(),
        versions_dir(),
        java_dir(),
    ] {
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn load_settings() -> Settings {
    let path = settings_path();
    if let Ok(raw) = fs::read_to_string(&path) {
        if let Ok(settings) = serde_json::from_str(&raw) {
            return settings;
        }
    }
    Settings {
        memory_mb: 4096,
        width: 1280,
        height: 720,
        ..Default::default()
    }
}

pub fn save_settings(settings: &Settings) -> Result<(), String> {
    ensure_dirs()?;
    let raw = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(settings_path(), raw).map_err(|e| e.to_string())
}
