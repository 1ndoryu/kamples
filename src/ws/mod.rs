use std::{sync::Arc, time::Duration};

use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use glory_rs::websocket::{WebSocketEnvelope, WebSocketHub};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use uuid::Uuid;

use crate::{errors::AppError, AppState};

const REDIS_WS_CHANNEL_PREFIX: &str = "ws:user:";
const REDIS_WS_CHANNEL_PATTERN: &str = "ws:user:*";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RedisBridgeMessage {
    origin_node_id: Uuid,
    user_id: i32,
    envelope: WebSocketEnvelope,
}

/* [174A-70/174A-72] Runtime websocket del backend principal sobre el hub reusable.
 * El socket gestiona handshake, registro/unregister y ping/pong; la proyección de
 * eventos de dominio queda en helpers para que cada handler emita sin acoplarse al
 * formato interno del hub. En 174A-73 se suma bridge Redis pub/sub para fanout
 * multi-instancia usando canales `ws:user:{id}`. */

pub async fn emit_event<T: Serialize>(
    state: &AppState,
    user_id: i32,
    name: &str,
    payload: &T,
) -> Result<usize, AppError> {
    let payload = serde_json::to_value(payload)
        .map_err(|error| AppError::Internal(format!("serializar payload websocket: {error}")))?;
    let envelope = WebSocketEnvelope::Event {
        name: name.to_owned(),
        payload,
    };
    let delivered = broadcast_envelope(&state.ws_hub, user_id, &envelope)?;
    publish_bridge_message(&state.redis, state.ws_node_id, user_id, &envelope).await?;
    Ok(delivered)
}

pub fn spawn_pubsub_bridge(redis_url: &str, hub: Arc<WebSocketHub>, node_id: Uuid) {
    let redis_url = redis_url.to_owned();
    tokio::spawn(async move {
        loop {
            match run_pubsub_bridge(&redis_url, Arc::clone(&hub), node_id).await {
                Ok(()) => {
                    tracing::warn!(%node_id, "bridge websocket redis finalizado");
                    break;
                }
                Err(error) => {
                    tracing::error!(
                        %node_id,
                        error = %error,
                        "falló el bridge websocket redis; reintentando"
                    );
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    });
}

async fn run_pubsub_bridge(
    redis_url: &str,
    hub: Arc<WebSocketHub>,
    node_id: Uuid,
) -> Result<(), AppError> {
    let client = redis::Client::open(redis_url)
        .map_err(|error| AppError::Internal(format!("crear cliente redis websocket: {error}")))?;
    let mut pubsub = client
        .get_async_pubsub()
        .await
        .map_err(|error| AppError::Internal(format!("abrir pubsub websocket: {error}")))?;
    pubsub
        .psubscribe(REDIS_WS_CHANNEL_PATTERN)
        .await
        .map_err(|error| AppError::Internal(format!("suscribir bridge websocket: {error}")))?;
    tracing::info!(%node_id, pattern = REDIS_WS_CHANNEL_PATTERN, "bridge websocket redis suscrito");

    let mut stream = pubsub.on_message();
    while let Some(message) = stream.next().await {
        let channel = message.get_channel_name().to_owned();
        let payload = match message.get_payload::<String>() {
            Ok(payload) => payload,
            Err(error) => {
                tracing::warn!(channel, error = %error, "payload redis websocket inválido");
                continue;
            }
        };

        let bridge_message = match parse_bridge_message(&channel, &payload) {
            Ok(bridge_message) => bridge_message,
            Err(error) => {
                tracing::warn!(channel, error = %error, "mensaje redis websocket descartado");
                continue;
            }
        };

        if bridge_message.origin_node_id == node_id {
            continue;
        }

        match broadcast_envelope(&hub, bridge_message.user_id, &bridge_message.envelope) {
            Ok(delivered) => {
                tracing::debug!(
                    %node_id,
                    user_id = bridge_message.user_id,
                    delivered,
                    "bridge websocket redis reenviado"
                );
            }
            Err(error) => {
                tracing::warn!(
                    %node_id,
                    user_id = bridge_message.user_id,
                    error = %error,
                    "falló fanout local del bridge websocket"
                );
            }
        }
    }

    Err(AppError::Internal(
        "stream pubsub websocket finalizado inesperadamente".into(),
    ))
}

pub async fn serve_socket(socket: WebSocket, hub: Arc<WebSocketHub>, user_id: i32) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = unbounded_channel::<Message>();
    let connection = match hub.register(user_id, tx.clone()) {
        Ok(connection) => connection,
        Err(error) => {
            let envelope = WebSocketEnvelope::Error {
                code: "too_many_connections".into(),
                message: error.to_string(),
            };
            if let Ok(message) = envelope.to_message() {
                let _ = sender.send(message).await;
            }
            let _ = sender.send(Message::Close(None)).await;
            return;
        }
    };

    let authenticated = WebSocketEnvelope::Authenticated {
        user_id,
        connection_id: connection.connection_id,
    };
    /* Se serializa una sola vez: la version anterior validaba con una llamada
     * y enviaba el resultado de otra. */
    let mensaje = match authenticated.to_message() {
        Ok(mensaje) => mensaje,
        Err(error) => {
            tracing::error!(user_id, error = %error, "no se pudo serializar mensaje authenticated websocket");
            hub.unregister(connection);
            return;
        }
    };
    if sender.send(mensaje).await.is_err() {
        hub.unregister(connection);
        return;
    }

    loop {
        tokio::select! {
            outbound = rx.recv() => {
                match outbound {
                    Some(message) => {
                        if sender.send(message).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
            inbound = receiver.next() => {
                match inbound {
                    Some(Ok(message)) => {
                        if !handle_incoming_message(&tx, message) {
                            break;
                        }
                    }
                    Some(Err(error)) => {
                        tracing::warn!(user_id, error = %error, "falló la recepción websocket");
                        break;
                    }
                    None => break,
                }
            }
        }
    }

    hub.unregister(connection);
}

fn handle_incoming_message(tx: &UnboundedSender<Message>, message: Message) -> bool {
    match message {
        Message::Text(text) => {
            handle_text_message(tx, &text);
            true
        }
        Message::Binary(_) => {
            send_envelope(
                tx,
                &WebSocketEnvelope::Error {
                    code: "unsupported_message".into(),
                    message: "Sólo se aceptan mensajes JSON tipo websocket envelope".into(),
                },
            );
            true
        }
        Message::Ping(payload) => tx.send(Message::Pong(payload)).is_ok(),
        Message::Pong(_) => true,
        Message::Close(_) => false,
    }
}

fn handle_text_message(tx: &UnboundedSender<Message>, text: &str) {
    match serde_json::from_str::<WebSocketEnvelope>(text) {
        Ok(WebSocketEnvelope::Ping) => {
            send_envelope(tx, &WebSocketEnvelope::Pong);
        }
        Ok(_) => {
            send_envelope(
                tx,
                &WebSocketEnvelope::Error {
                    code: "unsupported_message".into(),
                    message: "Este endpoint todavía no acepta eventos de cliente".into(),
                },
            );
        }
        Err(_) => {
            send_envelope(
                tx,
                &WebSocketEnvelope::Error {
                    code: "invalid_payload".into(),
                    message: "Payload websocket inválido".into(),
                },
            );
        }
    }
}

fn send_envelope(tx: &UnboundedSender<Message>, envelope: &WebSocketEnvelope) {
    match envelope.to_message() {
        Ok(message) => {
            let _ = tx.send(message);
        }
        Err(error) => {
            tracing::error!(error = %error, "no se pudo serializar envelope websocket");
        }
    }
}

fn broadcast_envelope(
    hub: &WebSocketHub,
    user_id: i32,
    envelope: &WebSocketEnvelope,
) -> Result<usize, AppError> {
    hub.broadcast_user(user_id, envelope)
        .map_err(|error| AppError::Internal(format!("emitir evento websocket: {error}")))
}

async fn publish_bridge_message(
    redis: &Option<deadpool_redis::Pool>,
    origin_node_id: Uuid,
    user_id: i32,
    envelope: &WebSocketEnvelope,
) -> Result<(), AppError> {
    let Some(redis) = redis else {
        return Ok(());
    };

    let message = RedisBridgeMessage {
        origin_node_id,
        user_id,
        envelope: envelope.clone(),
    };
    let payload = serde_json::to_string(&message)
        .map_err(|error| AppError::Internal(format!("serializar bridge websocket: {error}")))?;
    let mut connection = redis.get().await.map_err(|error| {
        AppError::Internal(format!("obtener conexión redis websocket: {error}"))
    })?;
    let _: i64 = connection
        .publish(redis_ws_channel(user_id), payload)
        .await
        .map_err(|error| AppError::Internal(format!("publicar evento websocket: {error}")))?;
    Ok(())
}

fn parse_bridge_message(channel: &str, payload: &str) -> Result<RedisBridgeMessage, AppError> {
    let message: RedisBridgeMessage = serde_json::from_str(payload)
        .map_err(|error| AppError::Internal(format!("deserializar bridge websocket: {error}")))?;
    let channel_user_id = parse_channel_user_id(channel)?;
    if channel_user_id != message.user_id {
        return Err(AppError::Internal(format!(
            "bridge websocket user_id mismatch canal={channel_user_id} payload={}",
            message.user_id,
        )));
    }
    Ok(message)
}

fn parse_channel_user_id(channel: &str) -> Result<i32, AppError> {
    channel
        .strip_prefix(REDIS_WS_CHANNEL_PREFIX)
        .ok_or_else(|| AppError::Internal(format!("canal websocket inválido: {channel}")))?
        .parse::<i32>()
        .map_err(|error| {
            AppError::Internal(format!("user_id inválido en canal websocket: {error}"))
        })
}

fn redis_ws_channel(user_id: i32) -> String {
    format!("{REDIS_WS_CHANNEL_PREFIX}{user_id}")
}

#[cfg(test)]
mod tests {
    use super::{
        broadcast_envelope, parse_bridge_message, parse_channel_user_id, RedisBridgeMessage,
    };
    use axum::extract::ws::Message;
    use glory_rs::websocket::{HubConfig, WebSocketEnvelope, WebSocketHub};
    use serde_json::json;
    use tokio::sync::mpsc::unbounded_channel;
    use uuid::Uuid;

    #[test]
    fn emits_domain_event_to_registered_user() {
        let hub = WebSocketHub::new(HubConfig::default());
        let (tx, mut rx) = unbounded_channel::<Message>();
        let _connection = hub.register(17, tx).expect("register");

        let delivered = broadcast_envelope(
            &hub,
            17,
            &WebSocketEnvelope::Event {
                name: "mensaje_nuevo".into(),
                payload: json!({ "id": 42 }),
            },
        )
        .expect("emit event");

        assert_eq!(delivered, 1);
        let Message::Text(payload) = rx.try_recv().expect("payload recibido") else {
            panic!("mensaje websocket no textual");
        };
        assert!(payload.contains("\"type\":\"event\""));
        assert!(payload.contains("\"name\":\"mensaje_nuevo\""));
        assert!(payload.contains("\"id\":42"));
    }

    #[test]
    fn parses_channel_user_id_from_expected_prefix() {
        assert_eq!(parse_channel_user_id("ws:user:17").expect("user id"), 17);
        assert!(parse_channel_user_id("ws:other:17").is_err());
    }

    #[test]
    fn parses_bridge_message_and_rejects_mismatched_channel() {
        let payload = serde_json::to_string(&RedisBridgeMessage {
            origin_node_id: Uuid::nil(),
            user_id: 23,
            envelope: WebSocketEnvelope::Event {
                name: "mensaje_nuevo".into(),
                payload: json!({ "id": 9 }),
            },
        })
        .expect("payload");

        let parsed = parse_bridge_message("ws:user:23", &payload).expect("bridge message");
        assert_eq!(parsed.user_id, 23);
        assert!(matches!(parsed.envelope, WebSocketEnvelope::Event { .. }));
        assert!(parse_bridge_message("ws:user:99", &payload).is_err());
    }
}
