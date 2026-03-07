use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SourceSide {
    Base,
    Target,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeleteBehavior {
    Trash,
    Delete,
    Ignore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveId {
    pub label: Option<String>,
    pub uuid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundConfig {
    pub sync_start: Option<PathBuf>,
    pub sync_done: Option<PathBuf>,
    pub sync_error: Option<PathBuf>,
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self {
            sync_start: None,
            sync_done: None,
            sync_error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairConfig {
    pub name: String,
    pub base: PathBuf,
    pub target: PathBuf,
    pub source: SourceSide,
    pub drive_id: Option<DriveId>,
    pub ignore: Vec<String>,
    pub delete_behavior: DeleteBehavior,
    pub sounds: SoundConfig,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub version: u32,
    pub pairs: Vec<PairConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            pairs: Vec::new(),
        }
    }
}

// ── Config file path ───────────────────────────────────────────────────────────

pub fn get_config_path() -> Result<PathBuf, String> {
    config_path()
}

pub fn reset_config() -> Result<(), String> {
    let path = config_path()?;
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| format!("Failed to delete config: {}", e))?;
    }
    Ok(())
}

fn config_path() -> Result<PathBuf, String> {
    let dir = dirs::config_dir()
        .ok_or_else(|| "Could not determine config directory".to_string())?
        .join("hard-sync");
    Ok(dir.join("config.json"))
}

// ── Read / Write ───────────────────────────────────────────────────────────────

pub fn load_config() -> Result<AppConfig, String> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read config: {}", e))?;
    serde_json::from_str(&raw)
        .map_err(|e| format!("Failed to parse config: {}", e))
}

pub fn save_config(config: &AppConfig) -> Result<(), String> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config directory: {}", e))?;
    }
    let raw = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    std::fs::write(&path, raw)
        .map_err(|e| format!("Failed to write config: {}", e))
}

// ── CRUD ───────────────────────────────────────────────────────────────────────

pub fn add_pair(pair: PairConfig) -> Result<(), String> {
    let mut config = load_config()?;
    if config.pairs.iter().any(|p| p.name == pair.name) {
        return Err(format!("A pair named \"{}\" already exists", pair.name));
    }
    config.pairs.push(pair);
    save_config(&config)
}

pub fn get_pair(name: &str) -> Result<PairConfig, String> {
    let config = load_config()?;
    config
        .pairs
        .into_iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("No pair named \"{}\"", name))
}

pub fn list_pairs() -> Result<Vec<PairConfig>, String> {
    let config = load_config()?;
    Ok(config.pairs)
}

pub fn remove_pair(name: &str) -> Result<(), String> {
    let mut config = load_config()?;
    let before = config.pairs.len();
    config.pairs.retain(|p| p.name != name);
    if config.pairs.len() == before {
        return Err(format!("No pair named \"{}\"", name));
    }
    save_config(&config)
}

pub fn set_source(name: &str, source: SourceSide) -> Result<(), String> {
    let mut config = load_config()?;
    let pair = config
        .pairs
        .iter_mut()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("No pair named \"{}\"", name))?;
    pair.source = source;
    save_config(&config)
}

pub fn update_pair(updated: PairConfig) -> Result<(), String> {
    let mut config = load_config()?;
    let pair = config
        .pairs
        .iter_mut()
        .find(|p| p.name == updated.name)
        .ok_or_else(|| format!("No pair named \"{}\"", updated.name))?;
    *pair = updated;
    save_config(&config)
}
