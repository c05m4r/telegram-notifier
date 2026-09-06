use crate::state::AppState;
use crate::telegram::Update;

pub struct BotHandler {
    state: AppState,
}

impl BotHandler {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn handle_update(&self, update: Update) {
        let message = match update.message {
            Some(m) => m,
            None => return,
        };

        let chat_id = message.chat.id;
        let user_id = message.from.as_ref().map(|u| u.id).unwrap_or(chat_id);
        let user_name = message
            .from
            .as_ref()
            .map(|u| u.first_name.clone())
            .unwrap_or_else(|| "Usuario".to_string());

        // Authorization check: users not in ALLOWED_USERS are ignored in
        // complete silence — no reply, no processing, no trace.
        if !self.state.is_authorized(user_id) {
            return;
        }

        let text = match message.text {
            Some(t) => t.trim().to_string(),
            None => return,
        };

        if !text.starts_with('/') {
            return;
        }

        let parts: Vec<&str> = text.split_whitespace().collect();
        let cmd = parts[0]
            .split('@')
            .next()
            .unwrap_or("")
            .to_lowercase();
        let args = &parts[1..];

        match cmd.as_str() {
            "/start" => self.cmd_start(chat_id, &user_name).await,
            "/help" => self.cmd_help(chat_id).await,
            "/id" => self.cmd_id(chat_id, user_id).await,
            "/subscribe" => self.cmd_subscribe(chat_id).await,
            "/unsubscribe" => self.cmd_unsubscribe(chat_id).await,
            "/subscribers" => self.cmd_subscribers(chat_id).await,
            "/silence" => self.cmd_silence(chat_id, args).await,
            "/test" => self.cmd_test(chat_id).await,
            "/loquendo" => self.cmd_loquendo(chat_id, args).await,
            "/notify" => self.cmd_notify(chat_id, args).await,
            "/broadcast" => self.cmd_broadcast(chat_id, args).await,
            _ => {
                let _ = self
                    .state
                    .client
                    .send_message(
                        chat_id,
                        "Comando no reconocido. Escribe /help para ver la lista de comandos disponibles.",
                        None,
                        false,
                    )
                    .await;
            }
        }
    }

    async fn cmd_start(&self, chat_id: i64, name: &str) {
        self.state.add_subscriber(chat_id).await;

        let text = format!(
            "*¡Hola, {}!*\n\n\
            Este bot es tu pasarela de notificaciones de Telegram.\n\n\
            *Tu Chat ID:* `{}`\n\
            *Notificaciones:* Suscrito automáticamente.\n\n\
            *Comandos útiles:*\n\
            • `/broadcast <mensaje>` - Enviar aviso a todos los suscriptores\n\
            • `/silence <min>` - Silenciar notificaciones temporalmente\n\
            • `/loquendo <texto>` - Enviar mensaje hablado con voz Loquendo\n\
            • `/test` - Enviar notificación de prueba\n\
            • `/help` - Ver todos los comandos",
            name, chat_id
        );

        let _ = self
            .state
            .client
            .send_message(chat_id, &text, Some("Markdown"), false)
            .await;
    }

    async fn cmd_help(&self, chat_id: i64) {
        let text = "*Comandos disponibles:*\n\n\
            *Gestión de Notificaciones:*\n\
            • `/subscribe` — Suscribir este chat a notificaciones\n\
            • `/unsubscribe` — Cancelar suscripción a notificaciones\n\
            • `/subscribers` — Ver lista de chats suscritos\n\
            • `/silence [minutos]` — Silenciar notificaciones durante N minutos (default: 30)\n\n\
            *Mensajería y TTS:*\n\
            • `/notify <chat_id> <mensaje>` — Enviar notificación a un chat específico\n\
            • `/broadcast <mensaje>` — Enviar anuncio a todos los chats suscritos\n\
            • `/loquendo <texto>` — Sintetizar audio con voz Loquendo\n\
            • `/test` — Enviar notificación de prueba\n\
            • `/id` — Mostrar tu Chat ID y User ID\n\
            • `/help` — Mostrar este mensaje de ayuda";

        let _ = self
            .state
            .client
            .send_message(chat_id, text, Some("Markdown"), false)
            .await;
    }

    async fn cmd_id(&self, chat_id: i64, user_id: i64) {
        let text = format!(
            "*Identificadores:*\n• *Chat ID:* `{}`\n• *User ID:* `{}`",
            chat_id, user_id
        );
        let _ = self
            .state
            .client
            .send_message(chat_id, &text, Some("Markdown"), false)
            .await;
    }

    async fn cmd_subscribe(&self, chat_id: i64) {
        let added = self.state.add_subscriber(chat_id).await;
        let text = if added {
            "*Suscripción activada.* Este chat recibirá notificaciones."
        } else {
            "Este chat ya estaba suscrito a las notificaciones."
        };
        let _ = self
            .state
            .client
            .send_message(chat_id, text, Some("Markdown"), false)
            .await;
    }

    async fn cmd_unsubscribe(&self, chat_id: i64) {
        let removed = self.state.remove_subscriber(chat_id).await;
        let text = if removed {
            "*Suscripción cancelada.* No recibirás más notificaciones en este chat."
        } else {
            "Este chat no estaba suscrito."
        };
        let _ = self
            .state
            .client
            .send_message(chat_id, text, Some("Markdown"), false)
            .await;
    }

    async fn cmd_subscribers(&self, chat_id: i64) {
        let subs = self.state.get_subscribers().await;
        let mut text = format!("*Chats suscritos:* `{}`\n\n", subs.len());
        for id in subs {
            text.push_str(&format!("  • `{}`\n", id));
        }
        let _ = self
            .state
            .client
            .send_message(chat_id, &text, Some("Markdown"), false)
            .await;
    }

    async fn cmd_silence(&self, chat_id: i64, args: &[&str]) {
        let minutes: u64 = args
            .first()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        self.state.silence(minutes).await;
        let text = format!(
            "*Notificaciones silenciadas por {} minutos.*\nNo se enviarán avisos hasta cumplirse el tiempo.",
            minutes
        );
        let _ = self
            .state
            .client
            .send_message(chat_id, &text, Some("Markdown"), false)
            .await;
    }

    async fn cmd_test(&self, chat_id: i64) {
        let text = format!(
            "*Notificación de prueba*\n\n\
            *Hora:* `{}`\n\n\
            ¡La pasarela de notificaciones de Telegram funciona correctamente!",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        );
        let _ = self
            .state
            .client
            .send_message(chat_id, &text, Some("Markdown"), false)
            .await;
    }

    async fn cmd_loquendo(&self, chat_id: i64, args: &[&str]) {
        let phrase = args.join(" ");
        if phrase.trim().is_empty() {
            let _ = self
                .state
                .client
                .send_message(
                    chat_id,
                    "Uso: `/loquendo <texto a pronunciar>`",
                    Some("Markdown"),
                    false,
                )
                .await;
            return;
        }

        let _ = self
            .state
            .client
            .send_message(
                chat_id,
                "⏳ _Generando voz de Loquendo..._",
                Some("Markdown"),
                true,
            )
            .await;

        match crate::tts::synthesize_speech(self.state.client.http_client(), &phrase).await {
            Ok(audio) => {
                let _ = self
                    .state
                    .client
                    .send_audio(
                        chat_id,
                        audio,
                        "loquendo.mp3",
                        "Audio Loquendo",
                        "Loquendo",
                        Some(&phrase),
                        false,
                    )
                    .await;
            }
            Err(e) => {
                let _ = self
                    .state
                    .client
                    .send_message(
                        chat_id,
                        &format!("Error generando audio: {}", e),
                        None,
                        false,
                    )
                    .await;
            }
        }
    }

    async fn cmd_notify(&self, chat_id: i64, args: &[&str]) {
        let Some(target) = args.first().and_then(|v| v.parse::<i64>().ok()) else {
            let _ = self
                .state
                .client
                .send_message(
                    chat_id,
                    "Uso: `/notify <chat_id> <mensaje>`",
                    Some("Markdown"),
                    false,
                )
                .await;
            return;
        };

        let body = args[1..].join(" ");
        if body.is_empty() {
            let _ = self
                .state
                .client
                .send_message(
                    chat_id,
                    "Uso: `/notify <chat_id> <mensaje>`",
                    Some("Markdown"),
                    false,
                )
                .await;
            return;
        }

        let full_text = format!("*Notificación:*\n\n{}", body);
        match self
            .state
            .client
            .send_message(target, &full_text, Some("Markdown"), false)
            .await
        {
            Ok(_) => {
                let feedback = format!("Notificación enviada al chat `{}`.", target);
                let _ = self
                    .state
                    .client
                    .send_message(chat_id, &feedback, Some("Markdown"), false)
                    .await;
            }
            Err(e) => {
                let _ = self
                    .state
                    .client
                    .send_message(
                        chat_id,
                        &format!("Error enviando al chat {}: {}", target, e),
                        None,
                        false,
                    )
                    .await;
            }
        }
    }

    async fn cmd_broadcast(&self, chat_id: i64, args: &[&str]) {
        if args.is_empty() {
            let _ = self
                .state
                .client
                .send_message(
                    chat_id,
                    "Uso: `/broadcast <mensaje>`",
                    Some("Markdown"),
                    false,
                )
                .await;
            return;
        }

        let body = args.join(" ");
        let full_text = format!("*Aviso general:*\n\n{}", body);
        let sent = self
            .state
            .broadcast(&full_text, Some("Markdown"), false)
            .await;
        let feedback = format!("Mensaje transmitido a {} suscriptores.", sent);
        let _ = self
            .state
            .client
            .send_message(chat_id, &feedback, None, false)
            .await;
    }
}
