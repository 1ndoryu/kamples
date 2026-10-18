use serde::Serialize;

use crate::repositories::{ConversationMessage, DirectMessageKind};
use crate::AppState;

const WS_EVENT_MESSAGE_CREATED: &str = "mensaje_nuevo";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MessageCreatedEventPayload {
    conversacion_id: i32,
    mensaje: RealtimeConversationMessage,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RealtimeConversationMessage {
    id: i32,
    conversacion_id: i32,
    remitente_id: i32,
    contenido: String,
    tipo: DirectMessageKind,
    media_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    media_metadata: Option<serde_json::Value>,
    leido: bool,
    creado_at: chrono::DateTime<chrono::Utc>,
}

pub async fn emit_new_message_event(
    state: &AppState,
    recipient_id: i32,
    message: &ConversationMessage,
) {
    let payload = build_message_created_event_payload(message);
    match crate::ws::emit_event(state, recipient_id, WS_EVENT_MESSAGE_CREATED, &payload).await {
        Ok(delivered) => {
            tracing::debug!(
                recipient_id,
                conversacion_id = message.conversacion_id,
                message_id = message.id,
                delivered,
                "evento websocket mensaje_nuevo emitido"
            );
        }
        Err(error) => {
            tracing::warn!(
                recipient_id,
                conversacion_id = message.conversacion_id,
                message_id = message.id,
                error = %error,
                "falló la emisión websocket mensaje_nuevo"
            );
        }
    }
}

fn build_message_created_event_payload(
    message: &ConversationMessage,
) -> MessageCreatedEventPayload {
    MessageCreatedEventPayload {
        conversacion_id: message.conversacion_id,
        mensaje: RealtimeConversationMessage {
            id: message.id,
            conversacion_id: message.conversacion_id,
            remitente_id: message.remitente_id,
            contenido: message.contenido.clone(),
            tipo: message.tipo,
            media_url: message.media_url.clone(),
            media_metadata: map_media_metadata_for_ws(
                message.tipo,
                message.media_metadata.as_ref(),
            ),
            leido: message.leido,
            creado_at: message.created_at,
        },
    }
}

fn map_media_metadata_for_ws(
    kind: DirectMessageKind,
    media_metadata: Option<&serde_json::Value>,
) -> Option<serde_json::Value> {
    let media_metadata = media_metadata?;
    match kind {
        DirectMessageKind::Texto => None,
        DirectMessageKind::Imagen | DirectMessageKind::Audio => {
            map_binary_media_metadata_for_ws(media_metadata)
        }
        DirectMessageKind::Sample => map_sample_media_metadata_for_ws(media_metadata),
    }
}

fn map_binary_media_metadata_for_ws(
    media_metadata: &serde_json::Value,
) -> Option<serde_json::Value> {
    let mut mapped = serde_json::Map::new();
    if let Some(extension) = media_metadata
        .get("extension")
        .and_then(serde_json::Value::as_str)
    {
        mapped.insert(
            "formato".into(),
            serde_json::Value::String(extension.to_owned()),
        );
    }
    if let Some(size_bytes) = media_metadata
        .get("size_bytes")
        .and_then(serde_json::Value::as_u64)
    {
        mapped.insert("tamano".into(), serde_json::Value::from(size_bytes));
    }
    if let Some(content_type) = media_metadata
        .get("content_type")
        .and_then(serde_json::Value::as_str)
    {
        mapped.insert(
            "mimeType".into(),
            serde_json::Value::String(content_type.to_owned()),
        );
    }

    (!mapped.is_empty()).then_some(serde_json::Value::Object(mapped))
}

fn map_sample_media_metadata_for_ws(
    media_metadata: &serde_json::Value,
) -> Option<serde_json::Value> {
    let mut mapped = serde_json::Map::new();
    if let Some(sample_id) = media_metadata
        .get("sample_id")
        .and_then(serde_json::Value::as_i64)
    {
        mapped.insert("sampleId".into(), serde_json::Value::from(sample_id));
    }
    if let Some(title) = media_metadata
        .get("titulo")
        .and_then(serde_json::Value::as_str)
    {
        mapped.insert("titulo".into(), serde_json::Value::String(title.to_owned()));
    }
    if let Some(short_id) = media_metadata
        .get("id_corto")
        .and_then(serde_json::Value::as_str)
    {
        mapped.insert(
            "idCorto".into(),
            serde_json::Value::String(short_id.to_owned()),
        );
    }
    if let Some(slug) = media_metadata
        .get("slug")
        .and_then(serde_json::Value::as_str)
    {
        mapped.insert("slug".into(), serde_json::Value::String(slug.to_owned()));
    }
    if let Some(sample_type) = media_metadata
        .get("tipo")
        .and_then(serde_json::Value::as_str)
    {
        mapped.insert(
            "tipo".into(),
            serde_json::Value::String(sample_type.to_owned()),
        );
    }
    if let Some(bpm) = media_metadata
        .get("bpm")
        .and_then(serde_json::Value::as_i64)
    {
        mapped.insert("bpm".into(), serde_json::Value::from(bpm));
    }
    if let Some(music_key) = media_metadata
        .get("key")
        .and_then(serde_json::Value::as_str)
    {
        mapped.insert(
            "key".into(),
            serde_json::Value::String(music_key.to_owned()),
        );
    }

    (!mapped.is_empty()).then_some(serde_json::Value::Object(mapped))
}

#[cfg(test)]
mod tests {
    use super::build_message_created_event_payload;
    use crate::repositories::{ConversationMessage, DirectMessageKind};
    use chrono::{TimeZone, Utc};
    use serde_json::json;

    #[test]
    fn serializes_message_created_event_with_legacy_shape() {
        let payload = build_message_created_event_payload(&ConversationMessage {
            id: 9,
            conversacion_id: 21,
            remitente_id: 7,
            contenido: "hola".into(),
            tipo: DirectMessageKind::Sample,
            media_url: Some("https://cdn.example/messages/9".into()),
            media_metadata: Some(json!({
                "sample_id": 51,
                "titulo": "Loop",
                "id_corto": "abc123",
                "slug": "loop-51",
                "tipo": "loop",
                "bpm": 128,
                "key": "Am"
            })),
            leido: false,
            created_at: Utc
                .with_ymd_and_hms(2026, 4, 18, 21, 0, 0)
                .single()
                .expect("fecha"),
        });

        let value = serde_json::to_value(payload).expect("payload json");

        assert_eq!(value["conversacionId"], 21);
        assert_eq!(value["mensaje"]["conversacionId"], 21);
        assert_eq!(value["mensaje"]["remitenteId"], 7);
        assert_eq!(
            value["mensaje"]["mediaUrl"],
            "https://cdn.example/messages/9"
        );
        assert_eq!(value["mensaje"]["mediaMetadata"]["sampleId"], 51);
        assert_eq!(value["mensaje"]["mediaMetadata"]["idCorto"], "abc123");
        assert!(value["mensaje"].get("conversacion_id").is_none());
        assert!(value["mensaje"]["creadoAt"].is_string());
    }
}
