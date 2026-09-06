use std::env;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Config {
    pub bot_token: String,
    pub default_chat_id: Option<i64>,
    pub allowed_users: Vec<i64>,
    pub http_host: String,
    pub http_port: u16,
    pub storage_path: PathBuf,
}

impl Config {
    pub fn load() -> Result<Self, String> {
        // Try loading .env from several possible locations
        let possible_env_files = [
            ".env",
            "bot.env",
            "../notifier.env",
            "../monitor.env",
            "../.env",
        ];

        for env_path in &possible_env_files {
            let path = Path::new(env_path);
            if path.exists() {
                let _ = dotenvy::from_path(path);
            }
        }
        let _ = dotenvy::dotenv();

        let bot_token = env::var("TELEGRAM_BOT_TOKEN")
            .map_err(|_| "TELEGRAM_BOT_TOKEN environment variable is required".to_string())?;

        let default_chat_id = env::var("TELEGRAM_CHAT_ID")
            .ok()
            .and_then(|v| v.trim().parse::<i64>().ok());

        let allowed_users = env::var("ALLOWED_USERS")
            .unwrap_or_default()
            .split(',')
            .filter_map(|s| s.trim().parse::<i64>().ok())
            .collect();

        let http_host = env::var("HTTP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let http_port = env::var("HTTP_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(8088);

        let storage_path = env::var("STORAGE_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("subscribers.json"));

        Ok(Self {
            bot_token,
            default_chat_id,
            allowed_users,
            http_host,
            http_port,
            storage_path,
        })
    }
}
