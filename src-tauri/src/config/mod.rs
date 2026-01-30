use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub shortcut: String,
    pub language: String,
    pub auto_paste: bool,
    pub model: ModelConfig,
    pub api: ApiConfig,
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub model_type: String,  // "local" or "api"
    pub local_model: String, // tiny, base, small, medium, large, or HF URL
    pub use_quantized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub provider: String, // openai, openrouter, groq
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub theme: String, // glass, minimal, dark
    pub opacity: f32,
    pub position: String,     // cursor, center
    pub auto_hide_delay: u64, // ms
}

impl Default for Config {
    fn default() -> Self {
        Self {
            shortcut: "Super+Shift+T".to_string(),
            language: "pt".to_string(),
            auto_paste: true,
            model: ModelConfig {
                model_type: "local".to_string(),
                local_model: "base".to_string(),
                use_quantized: true,
            },
            api: ApiConfig {
                provider: "openai".to_string(),
                api_key: String::new(),
                model: "whisper-1".to_string(),
            },
            ui: UiConfig {
                theme: "glass".to_string(),
                opacity: 0.9,
                position: "cursor".to_string(),
                auto_hide_delay: 3000,
            },
        }
    }
}

impl Config {
    pub fn load_or_create() -> Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config from {:?}", config_path))?;
            let config: Config =
                toml::from_str(&content).with_context(|| "Failed to parse config file")?;
            Ok(config)
        } else {
            let config = Self::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        // Create config directory if needed
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)
            .with_context(|| format!("Failed to write config to {:?}", config_path))?;

        Ok(())
    }

    pub fn config_path() -> Result<PathBuf> {
        let proj_dirs = ProjectDirs::from("com", "whisperia", "whisperia")
            .context("Failed to determine config directory")?;
        Ok(proj_dirs.config_dir().join("config.toml"))
    }

    #[allow(dead_code)]
    pub fn models_dir() -> Result<PathBuf> {
        let proj_dirs = ProjectDirs::from("com", "whisperia", "whisperia")
            .context("Failed to determine data directory")?;
        let models_dir = proj_dirs.data_dir().join("models");
        std::fs::create_dir_all(&models_dir)?;
        Ok(models_dir)
    }
}
