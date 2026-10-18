mod dispatch;

use serde::Serialize;

use crate::errors::AppError;
use crate::repositories::{ConversationMessage, DirectMessageKind, ProfileRepository};
use crate::services::CreateNotificationInput;
use crate::ws;
use crate::AppState;


pub struct NotificationFanoutService;

#[derive(Debug, Clone, Serialize)]
struct NotificationWsPayload {
    tipo: String,
    titulo: String,
    mensaje: String,
    datos: serde_json::Value,
    enlace: Option<String>,
    actor: Option<NotificationWsActor>,
}

#[derive(Debug, Clone, Serialize)]
struct NotificationWsActor {
    username: String,
    #[serde(rename = "avatarUrl")]
    avatar_url: Option<String>,
}

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

#[derive(Debug, Clone)]
struct ActorProfile {
    username: String,
    avatar_url: Option<String>,
}

async fn load_actor_profile(state: &AppState, user_id: i32) -> Result<ActorProfile, AppError> {
    let profile = ProfileRepository::find_by_id(&state.pool, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("usuario {user_id} no existe")))?;
    Ok(ActorProfile {
        username: profile.username,
        avatar_url: profile.avatar_url,
    })
}

fn maybe_send_email_opt_in(state: &AppState, recipient_id: i32, input: &CreateNotificationInput) {
    let _ = (&state.email_runtime, recipient_id, input);
}

const fn email_notifications_enabled() -> bool {
    false
}

async fn emit_message_event(state: &AppState, recipient_id: i32, message: &ConversationMessage) {
    let payload = MessageCreatedEventPayload {
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
    };

    let _ = ws::emit_event(state, recipient_id, "mensaje_nuevo", &payload).await;
}

fn map_media_metadata_for_ws(
    kind: DirectMessageKind,
    media_metadata: Option<&serde_json::Value>,
) -> Option<serde_json::Value> {
    let media_metadata = media_metadata?;
    match kind {
        DirectMessageKind::Texto => None,
        DirectMessageKind::Imagen | DirectMessageKind::Audio => Some(serde_json::json!({
            "formato": media_metadata.get("extension")?.as_str()?,
            "tamano": media_metadata.get("size_bytes")?.as_u64()?,
            "mimeType": media_metadata.get("content_type")?.as_str()?,
        })),
        DirectMessageKind::Sample => Some(serde_json::json!({
            "sampleId": media_metadata.get("sample_id")?.as_i64()?,
            "titulo": media_metadata.get("titulo")?.as_str()?,
            "idCorto": media_metadata.get("id_corto")?.as_str()?,
            "slug": media_metadata.get("slug")?.as_str()?,
            "tipo": media_metadata.get("tipo")?.as_str()?,
            "bpm": media_metadata.get("bpm")?.as_i64()?,
            "key": media_metadata.get("key")?.as_str()?,
        })),
    }
}

fn message_preview(message: &ConversationMessage) -> String {
    match message.tipo {
        DirectMessageKind::Imagen => "[Imagen]".into(),
        DirectMessageKind::Audio => "[Audio]".into(),
        DirectMessageKind::Sample => "[Sample]".into(),
        DirectMessageKind::Texto => message.contenido.clone(),
    }
}

fn merge_push_data(
    input: &CreateNotificationInput,
    actor_avatar_url: Option<String>,
) -> serde_json::Value {
    let mut data = input.datos.clone();
    if !data.is_object() {
        data = serde_json::json!({});
    }
    if let Some(object) = data.as_object_mut() {
        object.insert("tipo".into(), serde_json::Value::String(input.tipo.clone()));
        object.insert(
            "enlace".into(),
            input
                .enlace
                .clone()
                .map_or(serde_json::Value::Null, serde_json::Value::String),
        );
        object.insert(
            "actorAvatarUrl".into(),
            actor_avatar_url.map_or(serde_json::Value::Null, serde_json::Value::String),
        );
    }
    data
}

fn human_title_for_type(notification_type: &str) -> &'static str {
    match notification_type {
        "like" => "Nuevo like",
        "encanta" => "Le encanta tu sample",
        "follow" => "Nuevo seguidor",
        "comentario" => "Nuevo comentario",
        "mencion" => "Te mencionaron",
        "descarga" => "Descargaron tu sample",
        "repost" => "Repost de tu sample",
        "venta" => "Venta de sample",
        "pago" => "Pago recibido",
        "moderacion" => "Aviso de moderación",
        _ => "Kamples",
    }
}

fn compact_snippet(raw: &str, max_chars: usize) -> String {
    let normalized = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        normalized
    } else {
        format!(
            "{}…",
            normalized.chars().take(max_chars).collect::<String>()
        )
    }
}

fn asset_to_public_url(public_base_url: Option<&str>, raw: Option<String>) -> Option<String> {
    let raw = raw?.trim().replace('\\', "/");
    if raw.is_empty() {
        return None;
    }
    if raw.starts_with("http://") || raw.starts_with("https://") {
        return Some(raw);
    }

    let path = if raw.starts_with('/') {
        raw
    } else {
        format!("/uploads/{raw}")
    };
    Some(match public_base_url {
        Some(base) => format!("{}{}", base.trim_end_matches('/'), path),
        None => path,
    })
}
