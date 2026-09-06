# Telegram Notifier Gateway (Rust)

Pasarela de notificaciones centralizada para Telegram, con soporte para servidor HTTP, gestión interactiva de suscriptores y síntesis de voz en español (Loquendo / TTS).

Permite que cualquier script, cron, herramienta de monitoreo, despliegue CI/CD o webhook envíe avisos a chats de Telegram a través de una API HTTP local simple.

---

## Características

- **Servidor HTTP integrado**:
  - `POST /notify` — Recibe notificaciones (JSON o texto plano) y las retransmite a todos los suscriptores.
  - `POST /alert` — Difunde alertas urgentes inmediatamente con formato destacado.
  - `GET /health` — Verificación de salud y cantidad de suscriptores.
- **Voz de Loquendo / TTS a demanda**:
  - Comando `/loquendo <texto>` para generar audios hablados en español desde Telegram.
- **Interacción y comandos en Telegram**:
  - `/subscribe` — Suscribe el chat actual para recibir avisos.
  - `/unsubscribe` — Cancela la suscripción.
  - `/subscribers` — Muestra la lista de chats suscritos.
  - `/silence [minutos]` — Silencia temporalmente las notificaciones (ideal para mantenimientos).
  - `/notify <chat_id> <mensaje>` — Envía a un chat específico.
  - `/broadcast <mensaje>` — Envía a todos los suscriptores.
  - `/loquendo <texto>` — Genera nota de voz hablada.
  - `/id` — Devuelve el Chat ID y User ID.
  - `/test` — Envía una notificación de prueba.
- **Persistencia**: Conserva los chats suscritos en `subscribers.json` tras reinicios.
- **Seguridad estricta**: Lista blanca `ALLOWED_USERS` — los usuarios **no listados son ignorados en silencio total** (sin respuesta, sin procesamiento). Lista vacía = **nadie** puede usar el bot.

---

## Compilación e Instalación

```bash
cd telegram-notifier
cp .env.example .env
# Configura TELEGRAM_BOT_TOKEN en .env
cargo build --release
```

El binario compilado quedará en `target/release/telegram-notifier`.

---

## Configuración (.env)

| Variable | Valor por defecto | Descripción |
|---|---|---|
| `TELEGRAM_BOT_TOKEN` | — | **Requerido**. Token de @BotFather |
| `TELEGRAM_CHAT_ID` | — | Chat ID inicial a suscribir (opcional) |
| `ALLOWED_USERS` | — | Usuarios autorizados separados por coma. **Vacío = nadie autorizado** (silencio total) |
| `HTTP_HOST` | `127.0.0.1` | Host donde escucha la API HTTP |
| `HTTP_PORT` | `8088` | Puerto de la API HTTP |
| `STORAGE_PATH` | `subscribers.json` | Archivo para persistir chats suscritos |

---

## Documentación interactiva (Swagger UI)

La API expone documentación OpenAPI generada automáticamente con [utoipa](https://github.com/juhaku/utoipa).

| URL | Descripción |
|---|---|
| `http://127.0.0.1:8088/swagger` | **Swagger UI** — interfaz interactiva para explorar y probar los endpoints |
| `http://127.0.0.1:8088/api-docs/openapi.json` | Spec OpenAPI cruda en JSON (importable en Postman/Insomnia/Redoc) |

---

## Envío de notificaciones vía HTTP

### Texto simple:
```bash
curl -X POST http://127.0.0.1:8088/notify -d "Backup completado correctamente."
```

### JSON estructurado:
```bash
curl -X POST http://127.0.0.1:8088/notify \
  -H "Content-Type: application/json" \
  -d '{
    "host": "prod-server-01",
    "title": "Despliegue finalizado",
    "message": "Versión 2.4.0 desplegada con éxito.",
    "priority": "ok"
  }'
```

### Alerta urgente:
```bash
curl -X POST http://127.0.0.1:8088/notify \
  -H "Content-Type: application/json" \
  -d '{
    "host": "prod-db-01",
    "title": "Fallo crítico en base de datos",
    "message": "Conexiones agotadas",
    "priority": "urgent"
  }'
```

### Dirigir a un chat o usuario específico (`chat_id`):
Por defecto `/notify` y `/alert` envían a **todos** los suscriptores. Si el JSON incluye `chat_id`, el mensaje va **solo a ese chat o usuario**.

**A un usuario concreto** (chat privado — su `chat_id` es numérico positivo, igual que su User ID):
```bash
curl -X POST http://127.0.0.1:8088/notify \
  -H "Content-Type: application/json" \
  -d '{
    "chat_id": 123456789,
    "title": "Tarea finalizada",
    "message": "Tu job de backup terminó OK.",
    "priority": "ok"
  }'
```

**A un grupo o canal** (IDs negativos, `-100...`):
```bash
curl -X POST http://127.0.0.1:8088/notify \
  -H "Content-Type: application/json" \
  -d '{
    "chat_id": -100123456789,
    "title": "Mantenimiento",
    "message": "Solo este grupo, los demás no reciben nada.",
    "priority": "warning"
  }'
```

Mismo comportamiento en `/alert` (chat_id opcional). Sin `chat_id` → broadcast a todos.

> **Privacidad de Telegram**: el bot solo puede escribir al chat privado de un usuario si ese usuario ya le escribió antes (o lo agregó y tiene `TELEGRAM_CHAT_ID` inicial). Si usas `chat_id` y el usuario nunca inició conversación, Telegram rechaza el envío con error *"bot can't initiate conversation"*.

---

## Ejecutar como Servicio (systemd)

Crea `/etc/systemd/system/telegram-notifier.service`:
```ini
[Unit]
Description=Telegram Notifier Gateway
After=network.target

[Service]
Type=simple
User=tu-usuario
WorkingDirectory=/home/tu-usuario/git/telegram-notifier
ExecStart=/home/tu-usuario/git/telegram-notifier/target/release/telegram-notifier
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

Habilitar:
```bash
sudo systemctl daemon-reload
sudo systemctl enable --now telegram-notifier
```

---

## Logs

Los logs se escriben en **consola (stdout/stderr)** — no hay archivo propio. Dónde verlos según cómo corras el proceso:

**Terminal directo:**
```bash
./target/release/telegram-notifier
```

**systemd** (si usas el servicio):
```bash
journalctl -u telegram-notifier -f
```

**Redireccionar a archivo** (útil para crontab/daemon):
```bash
# crea el dir la primera vez
mkdir -p /var/log
./target/release/telegram-notifier >> /var/log/telegram-notifier.log 2>&1
tail -f /var/log/telegram-notifier.log
```

**Nivel de detalle** — filtro por `RUST_LOG` (default `info`):
```bash
RUST_LOG=debug ./target/release/telegram-notifier   # + cada envío por chat y motivos
RUST_LOG=warn  ./target/release/telegram-notifier   # solo avisos y errores
RUST_LOG=telegram_notifier=info,axum=warn ./target/release/telegram-notifier  # por módulo
```

---

## Ejecutar con crontab (alternativa a systemd)

```cron
NOTIFIER_DIR=/home/tu-usuario/git/telegram-notifier

@reboot cd $NOTIFIER_DIR && ./target/release/telegram-notifier >> $NOTIFIER_DIR/logs/notifier.log 2>&1
*/5 * * * * pgrep -f "$NOTIFIER_DIR/target/release/telegram-notifier" > /dev/null || (cd $NOTIFIER_DIR && ./target/release/telegram-notifier >> $NOTIFIER_DIR/logs/notifier.log 2>&1)
```
