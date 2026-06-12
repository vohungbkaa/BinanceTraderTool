use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppConfig {
    pub timeframes: Vec<String>,
    pub altcoin_count: usize,
    pub altcoin_analysis_timeframe: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            timeframes: vec!["15m".to_string(), "4h".to_string(), "1d".to_string()],
            altcoin_count: 50,
            altcoin_analysis_timeframe: "1d".to_string(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        // Try reading from custom_config.json first
        if let Ok(content) = fs::read_to_string("custom_config.json") {
            if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                return config;
            }
        }

        // Fallback to config.json
        if let Ok(content) = fs::read_to_string("config.json") {
            // We use generic JSON parsing and then merge with default to allow missing fields
            if let Ok(mut json_val) = serde_json::from_str::<serde_json::Value>(&content) {
                let default_val = serde_json::to_value(AppConfig::default()).unwrap();
                if let Some(obj) = json_val.as_object_mut() {
                    if !obj.contains_key("altcoin_analysis_timeframe") {
                        obj.insert(
                            "altcoin_analysis_timeframe".to_string(),
                            default_val["altcoin_analysis_timeframe"].clone(),
                        );
                    }
                    if !obj.contains_key("altcoin_count") {
                        obj.insert(
                            "altcoin_count".to_string(),
                            default_val["altcoin_count"].clone(),
                        );
                    }
                    if !obj.contains_key("timeframes") {
                        obj.insert("timeframes".to_string(), default_val["timeframes"].clone());
                    }
                }
                if let Ok(config) = serde_json::from_value::<AppConfig>(json_val) {
                    return config;
                }
            }
        }

        AppConfig::default()
    }
}
