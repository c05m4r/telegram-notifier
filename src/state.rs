use crate::config::Config;
use crate::telegram::TelegramClient;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

pub struct InnerState {
    pub subscribers: HashSet<i64>,
    pub silenced_until: Option<Instant>,
}

#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<RwLock<InnerState>>,
    pub config: Arc<Config>,
    pub client: TelegramClient,
    pub storage_path: PathBuf,
}

impl AppState {
    pub fn new(config: Arc<Config>, client: TelegramClient) -> Self {
        let mut subscribers = HashSet::new();

        // Load subscribers from file if exists
        let storage_path = config.storage_path.clone();
        if storage_path.exists() {
            if let Ok(content) = fs::read_to_string(&storage_path) {
                if let Ok(loaded) = serde_json::from_str::<HashSet<i64>>(&content) {
                    subscribers = loaded;
                }
            }
        }

        // Add default_chat_id if provided
        if let Some(id) = config.default_chat_id {
            subscribers.insert(id);
        }

        let inner = InnerState {
            subscribers,
            silenced_until: None,
        };

        let state = Self {
            inner: Arc::new(RwLock::new(inner)),
            config,
            client,
            storage_path,
        };

        // Save initial state
        state.save_subscribers_sync();

        state
    }

    pub fn is_authorized(&self, user_id: i64) -> bool {
        // Strict whitelist: empty list = no one is authorized (deny all).
        if self.config.allowed_users.is_empty() {
            return false;
        }
        self.config.allowed_users.contains(&user_id)
    }

    pub async fn add_subscriber(&self, chat_id: i64) -> bool {
        let mut inner = self.inner.write().await;
        let inserted = inner.subscribers.insert(chat_id);
        drop(inner);
        if inserted {
            self.save_subscribers().await;
        }
        inserted
    }

    pub async fn remove_subscriber(&self, chat_id: i64) -> bool {
        let mut inner = self.inner.write().await;
        let removed = inner.subscribers.remove(&chat_id);
        drop(inner);
        if removed {
            self.save_subscribers().await;
        }
        removed
    }

    pub async fn get_subscribers(&self) -> Vec<i64> {
        let inner = self.inner.read().await;
        inner.subscribers.iter().copied().collect()
    }

    pub async fn silence(&self, minutes: u64) {
        let mut inner = self.inner.write().await;
        inner.silenced_until = Some(Instant::now() + Duration::from_secs(minutes * 60));
    }

    pub async fn is_silenced(&self) -> bool {
        let inner = self.inner.read().await;
        if let Some(until) = inner.silenced_until {
            Instant::now() < until
        } else {
            false
        }
    }

    pub async fn broadcast(&self, text: &str, parse_mode: Option<&str>, silent: bool) -> usize {
        let subscribers = self.get_subscribers().await;
        let mut count = 0;
        for chat_id in subscribers {
            if self.client.send_message(chat_id, text, parse_mode, silent).await.is_ok() {
                count += 1;
            }
        }
        count
    }

    /// Envía a un chat específico (`Some(chat_id)`) o a todos los suscriptores (`None`).
    pub async fn deliver(
        &self,
        text: &str,
        parse_mode: Option<&str>,
        silent: bool,
        target_chat: Option<i64>,
    ) -> usize {
        match target_chat {
            Some(chat_id) => {
                if self
                    .client
                    .send_message(chat_id, text, parse_mode, silent)
                    .await
                    .is_ok()
                {
                    1
                } else {
                    0
                }
            }
            None => self.broadcast(text, parse_mode, silent).await,
        }
    }

    fn save_subscribers_sync(&self) {
        if let Ok(inner) = self.inner.try_read() {
            if let Ok(json) = serde_json::to_string_pretty(&inner.subscribers) {
                let _ = fs::write(&self.storage_path, json);
            }
        }
    }

    async fn save_subscribers(&self) {
        let inner = self.inner.read().await;
        if let Ok(json) = serde_json::to_string_pretty(&inner.subscribers) {
            let path = self.storage_path.clone();
            tokio::spawn(async move {
                let _ = tokio::fs::write(path, json).await;
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_state_subscribers_and_silence() {
        let storage_path = std::env::temp_dir().join(format!("test_subscribers_{}.json", std::process::id()));
        let config = Arc::new(Config {
            bot_token: "test_token".to_string(),
            default_chat_id: Some(111),
            allowed_users: vec![111, 222],
            http_host: "127.0.0.1".to_string(),
            http_port: 8088,
            storage_path: storage_path.clone(),
        });

        let client = TelegramClient::new("dummy");
        let state = AppState::new(config, client);

        assert!(state.is_authorized(111));
        assert!(state.is_authorized(222));
        assert!(!state.is_authorized(333));

        // Empty whitelist must deny everyone (strict mode)
        let strict_config = Arc::new(Config {
            bot_token: "test_token".to_string(),
            default_chat_id: None,
            allowed_users: vec![],
            http_host: "127.0.0.1".to_string(),
            http_port: 8088,
            storage_path: std::env::temp_dir().join("test_strict_subs.json"),
        });
        let strict_state = AppState::new(strict_config, TelegramClient::new("dummy"));
        assert!(!strict_state.is_authorized(111));
        assert!(!strict_state.is_authorized(999999));

        let subs = state.get_subscribers().await;
        assert!(subs.contains(&111));

        state.add_subscriber(444).await;
        let subs2 = state.get_subscribers().await;
        assert!(subs2.contains(&444));

        state.remove_subscriber(111).await;
        let subs3 = state.get_subscribers().await;
        assert!(!subs3.contains(&111));

        assert!(!state.is_silenced().await);
        state.silence(10).await;
        assert!(state.is_silenced().await);

        let _ = std::fs::remove_file(storage_path);
    }
}
