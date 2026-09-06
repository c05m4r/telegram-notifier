use crate::state::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct NotifyRequest {
    pub host: Option<String>,
    pub title: Option<String>,
    pub message: Option<String>,
    pub priority: Option<String>,
    pub silent: Option<bool>,
    /// Chat ID específico al que enviar. Si se omite, se envía a TODOS los suscriptores.
    pub chat_id: Option<i64>,
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Telegram Notifier API",
        description = "Pasarela de notificaciones para Telegram. Envía avisos (JSON o texto plano) a todos los chats suscritos o a un chat específico vía chat_id.",
        version = "0.1.0"
    ),
    paths(health_handler, notify_handler, alert_handler),
    components(schemas(NotifyRequest))
)]
struct ApiDoc;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/notify", post(notify_handler))
        .route("/alert", post(alert_handler))
        .merge(SwaggerUi::new("/swagger").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .with_state(state)
}

pub async fn run_server(
    state: AppState,
    host: &str,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = create_router(state);
    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(address = %addr, "Servidor HTTP de notificaciones escuchando");
    axum::serve(listener, app).await?;
    Ok(())
}

/// Verificación de salud del servicio.
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Servicio operativo con conteo de suscriptores y estado de silencio")
    )
)]
async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    let subs = state.get_subscribers().await;
    let is_silenced = state.is_silenced().await;

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "ok",
            "subscribers_count": subs.len(),
            "is_silenced": is_silenced
        })),
    )
}

/// Recibe una notificación (JSON estructurado o texto plano) y la retransmite a los suscriptores.
#[utoipa::path(
    post,
    path = "/notify",
    responses(
        (status = 200, description = "Notificación transmitida"),
        (status = 400, description = "Cuerpo vacío")
    )
)]
async fn notify_handler(
    State(state): State<AppState>,
    body: String,
) -> impl IntoResponse {
    // Check if silenced
    if state.is_silenced().await {
        tracing::debug!("Notificación recibida pero suprimida por silencio de mantenimiento");
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "ok": true,
                "delivered_to": 0,
                "silenced": true
            })),
        );
    }

    // Try to parse as JSON first
    let (title, message, silent, target_chat) = if let Ok(req) = serde_json::from_str::<NotifyRequest>(&body) {
        let priority_str = req.priority.as_deref().unwrap_or("default").to_lowercase();
        let silent = req.silent.unwrap_or_else(|| priority_str == "min" || priority_str == "low");
        let host_tag = req.host.as_ref().map(|h| format!(" [{}]", h)).unwrap_or_default();
        let title = req.title.unwrap_or_else(|| format!("Notificación{}", host_tag));
        let message = req.message.unwrap_or_default();
        (title, message, silent, req.chat_id)
    } else {
        // Raw text body
        let clean_body = body.trim();
        if clean_body.is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "error": "Empty message body" })),
            );
        }
        ("Notificación".to_string(), clean_body.to_string(), false, None)
    };

    let full_text = if message.is_empty() {
        format!("*{}*", title)
    } else {
        format!("*{}*\n\n{}", title, message)
    };

    let delivered = state
        .deliver(&full_text, Some("Markdown"), silent, target_chat)
        .await;

    tracing::info!(
        delivered_to = delivered,
        target_chat = ?target_chat,
        title = %title,
        message = %full_text,
        "Notificación transmitida"
    );

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "ok": true,
            "delivered_to": delivered
        })),
    )
}

/// Difunde una alerta urgente con formato destacado.
/// Siempre se envía, ignorando el silencio de mantenimiento.
#[utoipa::path(
    post,
    path = "/alert",
    responses(
        (status = 200, description = "Alerta transmitida"),
        (status = 400, description = "Cuerpo vacío")
    )
)]
async fn alert_handler(
    State(state): State<AppState>,
    body: String,
) -> impl IntoResponse {
    let clean_body = body.trim();
    if clean_body.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "ok": false, "error": "Empty alert body" })),
        );
    }

    let full_text = format!(" *ALERTA URGENTE*\n\n{}", clean_body);

    // Si el body es JSON, permite dirigir la alerta a un chat específico (chat_id);
    // texto plano o ausencia de chat_id → a todos los suscriptores.
    let target_chat = serde_json::from_str::<NotifyRequest>(&body)
        .ok()
        .and_then(|r| r.chat_id);

    let delivered = state
        .deliver(&full_text, Some("Markdown"), false, target_chat)
        .await;

    tracing::warn!(
        delivered_to = delivered,
        target_chat = ?target_chat,
        message = %full_text,
        "Alerta urgente transmitida"
    );

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "ok": true,
            "delivered_to": delivered
        })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::telegram::TelegramClient;
    use axum::body::Body;
    use axum::http::Request;
    use std::sync::Arc;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_endpoint() {
        let config = Arc::new(Config {
            bot_token: "fake_token".to_string(),
            default_chat_id: None,
            allowed_users: vec![],
            http_host: "127.0.0.1".to_string(),
            http_port: 8088,
            storage_path: std::env::temp_dir().join("test_health_subs.json"),
        });
        let client = TelegramClient::new("fake_token");
        let app_state = AppState::new(config, client);
        let app = create_router(app_state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_notify_empty_rejected() {
        let config = Arc::new(Config {
            bot_token: "fake_token".to_string(),
            default_chat_id: None,
            allowed_users: vec![],
            http_host: "127.0.0.1".to_string(),
            http_port: 8088,
            storage_path: std::env::temp_dir().join("test_notify_empty.json"),
        });
        let client = TelegramClient::new("fake_token");
        let app_state = AppState::new(config, client);
        let app = create_router(app_state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/notify")
                    .body(Body::from(""))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_notify_request_parses_chat_id() {
        let body = r#"{"chat_id": -100123456789, "message": "solo para este chat", "priority": "urgent"}"#;
        let req: NotifyRequest = serde_json::from_str(body).unwrap();
        assert_eq!(req.chat_id, Some(-100123456789));
        assert_eq!(req.message.as_deref(), Some("solo para este chat"));

        // Sin chat_id → broadcast (None)
        let body2 = r#"{"message": "para todos"}"#;
        let req2: NotifyRequest = serde_json::from_str(body2).unwrap();
        assert_eq!(req2.chat_id, None);
    }
}
