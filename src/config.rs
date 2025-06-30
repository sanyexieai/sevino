use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub host: String,
    pub port: u16,
    pub data_dir: String,
    pub max_file_size: u64,
    pub enable_cors: bool,
    pub cors_origins: Vec<String>,
    pub cors_methods: Vec<String>,
    pub cors_headers: Vec<String>,
    pub cors_allow_credentials: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8000,
            data_dir: "./data".to_string(),
            max_file_size: 100 * 1024 * 1024, // 100MB
            enable_cors: true,
            cors_origins: vec![
                "http://localhost:3000".to_string(),
                "http://127.0.0.1:3000".to_string(),
                "http://localhost:8080".to_string(),
                "http://127.0.0.1:8080".to_string(),
                "*".to_string(), // 允许所有域名（开发环境）
            ],
            cors_methods: vec![
                "GET".to_string(),
                "POST".to_string(),
                "PUT".to_string(),
                "DELETE".to_string(),
                "OPTIONS".to_string(),
            ],
            cors_headers: vec![
                "Content-Type".to_string(),
                "Authorization".to_string(),
                "X-Requested-With".to_string(),
                "Accept".to_string(),
                "Origin".to_string(),
            ],
            cors_allow_credentials: false,
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
        
        // CORS配置
        if let Ok(cors_origins) = env::var("SEVINO_CORS_ORIGINS") {
            settings.cors_origins = cors_origins
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        
        if let Ok(cors_methods) = env::var("SEVINO_CORS_METHODS") {
            settings.cors_methods = cors_methods
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        
        if let Ok(cors_headers) = env::var("SEVINO_CORS_HEADERS") {
            settings.cors_headers = cors_headers
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        
        if let Ok(allow_credentials) = env::var("SEVINO_CORS_ALLOW_CREDENTIALS") {
            settings.cors_allow_credentials = allow_credentials.to_lowercase() == "true";
        }
        
        settings
    }
} 