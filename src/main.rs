mod bot_handler;
mod config;
mod http_server;
mod logging;
mod state;
mod telegram;
mod tts;

use bot_handler::BotHandler;
use config::Config;
use state::AppState;
use std::sync::Arc;
use std::time::Duration;
use telegram::TelegramClient;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    logging::init();
    info!("Iniciando Telegram Notifier Gateway...");

    let config = match Config::load() {
        Ok(c) => Arc::new(c),
        Err(e) => {
            error!("Error de configuración: {}", e);
            error!("Asegúrate de definir TELEGRAM_BOT_TOKEN en .env");
            std::process::exit(1);
        }
    };

    let client = TelegramClient::new(&config.bot_token);

    // Verificar token con Telegram
    match client.get_me().await {
        Ok(me) => {
            info!(
                username = me.username.as_deref().unwrap_or("desconocido"),
                bot_id = me.id,
                "Conectado a Telegram"
            );
        }
        Err(e) => {
            warn!("No se pudo verificar el bot con Telegram: {}", e);
            warn!("Verifica tu conexión y que el token de TELEGRAM_BOT_TOKEN sea válido.");
        }
    }

    // Registrar menú de comandos en Telegram
    let bot_commands = [
        ("subscribe", "Suscribirse a notificaciones"),
        ("unsubscribe", "Desuscribirse de notificaciones"),
        ("subscribers", "Ver lista de chats suscritos"),
        ("silence", "Silenciar notificaciones por N minutos"),
        ("notify", "Enviar notificación a un chat específico"),
        ("broadcast", "Enviar aviso a todos los suscriptores"),
        ("loquendo", "Sintetizar mensaje con voz Loquendo"),
        ("id", "Mostrar tu Chat ID y User ID"),
        ("test", "Enviar notificación de prueba"),
        ("help", "Mostrar lista de comandos"),
    ];

    if let Err(e) = client.set_my_commands(&bot_commands).await {
        warn!("No se pudieron registrar los comandos en el menú de Telegram: {}", e);
    } else {
        info!("Comandos registrados en el menú de Telegram");
    }

    let state = AppState::new(config.clone(), client.clone());

    // Servidor HTTP de notificaciones
    let http_state = state.clone();
    let host = config.http_host.clone();
    let port = config.http_port;

    tokio::spawn(async move {
        if let Err(e) = http_server::run_server(http_state, &host, port).await {
            error!("Error en servidor HTTP: {}", e);
        }
    });

    // Handler de mensajes del bot
    let handler = Arc::new(BotHandler::new(state.clone()));

    info!("Telegram Notifier listo. Esperando mensajes en Telegram y peticiones HTTP...");

    let mut offset: i64 = 0;

    loop {
        match client.get_updates(offset, 30).await {
            Ok(updates) => {
                for update in updates {
                    offset = update.update_id + 1;
                    let h = handler.clone();
                    tokio::spawn(async move {
                        h.handle_update(update).await;
                    });
                }
            }
            Err(e) => {
                warn!("Error al obtener actualizaciones de Telegram: {}. Reintentando en 5s...", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}