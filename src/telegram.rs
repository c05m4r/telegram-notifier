use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub ok: bool,
    pub result: Option<T>,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Update {
    pub update_id: i64,
    pub message: Option<Message>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
pub struct Message {
    pub message_id: i64,
    pub from: Option<User>,
    pub chat: Chat,
    pub text: Option<String>,
    pub date: i64,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
pub struct Chat {
    pub id: i64,
    pub r#type: String,
    pub title: Option<String>,
    pub username: Option<String>,
    pub first_name: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize)]
pub struct User {
    pub id: i64,
    pub is_bot: bool,
    pub first_name: String,
    pub username: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct BotCommand {
    pub command: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize)]
struct SendMessagePayload<'a> {
    chat_id: i64,
    text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_mode: Option<&'a str>,
    disable_notification: bool,
}

#[derive(Clone)]
pub struct TelegramClient {
    client: reqwest::Client,
    base_url: String,
}

impl TelegramClient {
    pub fn new(token: &str) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(45))
            .build()
            .expect("Failed to initialize reqwest client");

        let base_url = format!("https://api.telegram.org/bot{}", token);
        Self { client, base_url }
    }

    pub async fn get_me(&self) -> Result<User, String> {
        let url = format!("{}/getMe", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Network error: {}", e))?;

        let data: ApiResponse<User> = resp
            .json()
            .await
            .map_err(|e| format!("JSON decode error: {}", e))?;

        if data.ok {
            data.result.ok_or_else(|| "Empty user in result".to_string())
        } else {
            Err(data.description.unwrap_or_else(|| "API error".to_string()))
        }
    }

    pub async fn get_updates(&self, offset: i64, timeout: u32) -> Result<Vec<Update>, String> {
        let url = format!("{}/getUpdates?offset={}&timeout={}", self.base_url, offset, timeout);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Network error in getUpdates: {}", e))?;

        let data: ApiResponse<Vec<Update>> = resp
            .json()
            .await
            .map_err(|e| format!("JSON error in getUpdates: {}", e))?;

        if data.ok {
            Ok(data.result.unwrap_or_default())
        } else {
            Err(data.description.unwrap_or_else(|| "API error".to_string()))
        }
    }

    pub async fn send_message(
        &self,
        chat_id: i64,
        text: &str,
        parse_mode: Option<&str>,
        silent: bool,
    ) -> Result<(), String> {
        let url = format!("{}/sendMessage", self.base_url);

        let payload = SendMessagePayload {
            chat_id,
            text,
            parse_mode,
            disable_notification: silent,
        };

        let resp = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Network error sending message: {}", e))?;

        let data: ApiResponse<serde_json::Value> = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        if data.ok {
            return Ok(());
        }

        // If formatted sending failed, retry as plain text (no parse_mode)
        if parse_mode.is_some() {
            let fallback_payload = SendMessagePayload {
                chat_id,
                text,
                parse_mode: None,
                disable_notification: silent,
            };

            let fallback_resp = self
                .client
                .post(&url)
                .json(&fallback_payload)
                .send()
                .await
                .map_err(|e| format!("Network error sending fallback message: {}", e))?;

            let fallback_data: ApiResponse<serde_json::Value> = fallback_resp
                .json()
                .await
                .map_err(|e| format!("Failed to parse fallback response: {}", e))?;

            if fallback_data.ok {
                return Ok(());
            }
        }

        Err(data.description.unwrap_or_else(|| "Failed to send message".to_string()))
    }

    pub fn http_client(&self) -> &reqwest::Client {
        &self.client
    }

    pub async fn send_audio(
        &self,
        chat_id: i64,
        audio_bytes: Vec<u8>,
        filename: &str,
        title: &str,
        performer: &str,
        caption: Option<&str>,
        silent: bool,
    ) -> Result<(), String> {
        let url = format!("{}/sendAudio", self.base_url);

        let part = reqwest::multipart::Part::bytes(audio_bytes)
            .file_name(filename.to_string())
            .mime_str("audio/mpeg")
            .map_err(|e| format!("Error en MIME: {}", e))?;

        let mut form = reqwest::multipart::Form::new()
            .text("chat_id", chat_id.to_string())
            .text("title", title.to_string())
            .text("performer", performer.to_string())
            .text("disable_notification", silent.to_string())
            .part("audio", part);

        if let Some(cap) = caption {
            form = form.text("caption", cap.to_string());
        }

        let resp = self
            .client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| format!("Network error sending audio: {}", e))?;

        let data: ApiResponse<serde_json::Value> = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse sendAudio response: {}", e))?;

        if data.ok {
            Ok(())
        } else {
            Err(data.description.unwrap_or_else(|| "Failed to send audio".to_string()))
        }
    }

    pub async fn set_my_commands(&self, commands: &[(&str, &str)]) -> Result<(), String> {
        let url = format!("{}/setMyCommands", self.base_url);
        let cmds: Vec<BotCommand> = commands
            .iter()
            .map(|(c, d)| BotCommand {
                command: c.to_string(),
                description: d.to_string(),
            })
            .collect();

        let payload = serde_json::json!({ "commands": cmds });

        let resp = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Network error setMyCommands: {}", e))?;

        let data: ApiResponse<bool> = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse setMyCommands: {}", e))?;

        if data.ok {
            Ok(())
        } else {
            Err(data.description.unwrap_or_else(|| "Error setting commands".to_string()))
        }
    }
}
