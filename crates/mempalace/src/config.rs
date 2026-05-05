use crate::error::MpError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MempalaceConfig {
    pub palace_path: String,
    pub collection_name: String,
    pub embedding_device: String,
    pub entity_languages: Vec<String>,
    pub topic_wings: Vec<String>,
    pub hall_keywords: HashMap<String, Vec<String>>,
    pub hook_silent_save: bool,
    pub hook_desktop_toast: bool,
    pub topic_tunnel_min_count: u32,
    // Auto-mining: true by default; set false to opt out
    #[serde(default = "default_true")]
    pub auto_mine_copilot_sessions: bool,
    #[serde(default = "default_true")]
    pub auto_mine_on_git_commit: bool,
}

fn default_true() -> bool { true }

impl Default for MempalaceConfig {
    fn default() -> Self {
        let palace_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".mempalace")
            .join("palace")
            .to_string_lossy()
            .to_string();
        Self {
            palace_path,
            collection_name: "mempalace_drawers".to_string(),
            embedding_device: "auto".to_string(),
            entity_languages: vec!["en".to_string()],
            topic_wings: vec![],
            hall_keywords: HashMap::new(),
            hook_silent_save: true,
            hook_desktop_toast: false,
            topic_tunnel_min_count: 3,
            auto_mine_copilot_sessions: true,
            auto_mine_on_git_commit: true,
        }
    }
}

impl MempalaceConfig {
    pub fn load() -> Self {
        let mut cfg = Self::default();
        if let Some(config_file) = Self::config_file_path() {
            if config_file.exists() {
                if let Ok(content) = std::fs::read_to_string(&config_file) {
                    if let Ok(file_cfg) = serde_json::from_str::<MempalaceConfig>(&content) {
                        cfg = file_cfg;
                    }
                }
            }
        }
        if let Ok(v) = std::env::var("MEMPALACE_PALACE_PATH") {
            if !v.is_empty() { cfg.palace_path = v; }
        }
        if let Ok(v) = std::env::var("MEMPALACE_EMBEDDING_DEVICE") {
            if !v.is_empty() { cfg.embedding_device = v; }
        }
        if let Ok(v) = std::env::var("MEMPALACE_ENTITY_LANGUAGES") {
            if !v.is_empty() {
                cfg.entity_languages = v.split(',').map(|s| s.trim().to_string()).collect();
            }
        }
        cfg
    }

    pub fn config_file_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".mempalace").join("config.json"))
    }

    pub fn save(&self) -> Result<(), MpError> {
        if let Some(config_file) = Self::config_file_path() {
            if let Some(parent) = config_file.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let json = serde_json::to_string_pretty(self)?;
            std::fs::write(&config_file, json)?;
        }
        Ok(())
    }

    pub fn knowledge_graph_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".mempalace")
            .join("knowledge_graph.sqlite3")
    }

    pub fn wal_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".mempalace")
            .join("wal")
    }
}
