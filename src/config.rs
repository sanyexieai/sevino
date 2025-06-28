use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub host: String,
    pub port: u16,
    pub data_dir: String,
    pub max_file_size: u64,
    pub enable_cors: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8000,
            data_dir: "./data".to_string(),
            max_file_size: 100 * 1024 * 1024, // 100MB
            enable_cors: true,
        }
    }
}

impl Settings {
    pub fn from_env() -> Self {
        let mut settings = Self::default();
        
        if let Ok(host) = env::var("SEVINO_HOST") {
            settings.host = host;
        }
        
        if let Ok(port) = env::var("SEVINO_PORT") {
            if let Ok(port_num) = port.parse() {
                settings.port = port_num;
            }
        }
        
        if let Ok(data_dir) = env::var("SEVINO_DATA_DIR") {
            settings.data_dir = data_dir;
        }
        
        if let Ok(max_file_size) = env::var("SEVINO_MAX_FILE_SIZE") {
            if let Ok(size) = max_file_size.parse() {
                settings.max_file_size = size;
            }
        }
        
        if let Ok(enable_cors) = env::var("SEVINO_ENABLE_CORS") {
            settings.enable_cors = enable_cors.to_lowercase() == "true";
        }
        
        settings
    }
} 