use serde::Serialize;

use crate::errors::AppError;
use crate::repositories::{ConversationMessage, DirectMessageKind, ProfileRepository, Reaction};
use crate::services::{
    CreateNotificationInput, FcmNotificationPayload, FcmNotificationService, NotificationService,
    PushNotificationPayload, PushNotificationService,
};
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

/* [174A-78] Fanout unificado de notificaciones.
 * Centraliza la persistencia in-app y los side-effects WS/Web Push/FCM para que
 * likes, follows, comentarios y mensajes no repliquen reglas legacy por handler.
 * El email opt-in queda desactivado hasta que exista una preferencia backend real. */

impl NotificationFanoutService {
    pub async fn dispatch_follow(
        state: &AppState,
        recipient_id: i32,
        actor_id: i32,
    ) -> Result<(), AppError> {
        let actor = load_actor_profile(state, actor_id).await?;
        let message = format!("@{} te ha seguido", actor.username);
        let link = Some(format!("/perfil/{}/", actor.username));
        Self::dispatch_persistent(
            state,
            CreateNotificationInput {
                destinatario_id: recipient_id,
                tipo: "follow".into(),
                titulo: String::new(),
                mensaje: message,
                datos: serde_json::json!({ "seguidor_id": actor_id }),
                actor_id: Some(actor_id),
                enlace: link,
            },
            actor,
        )
        .await
    }

    pub async fn dispatch_sample_reaction(
        state: &AppState,
        recipient_id: i32,
        actor_id: i32,
        sample_id: i32,
        sample_title: &str,
        sample_slug: Option<&str>,
        reaction: Reaction,
    ) -> Result<(), AppError> {
        let actor = load_actor_profile(state, actor_id).await?;
        let is_love = matches!(reaction, Reaction::Encanta);
        Self::dispatch_persistent(
            state,
            CreateNotificationInput {
                destinatario_id: recipient_id,
                tipo: if is_love { "encanta" } else { "like" }.into(),
                titulo: String::new(),
                mensaje: format!(
                    "@{} {} tu sample \"{}\"",
                    actor.username,
                    if is_love { "le encanta" } else { "le gusta" },
                    sample_title
                ),
                datos: serde_json::json!({
                    "liker_id": actor_id,
                    "sample_id": sample_id,
                    "sampleSlug": sample_slug,
                    "sampleTitulo": sample_title,
                    "reaccion": reaction.as_db_str(),
                }),
                actor_id: Some(actor_id),
                enlace: sample_slug.map(|slug| format!("/sample/{slug}/")),
            },
            actor,
        )
        .await
    }

    pub async fn dispatch_post_reaction(
        state: &AppState,
        recipient_id: i32,
        actor_id: i32,
        post_id: i32,
        reaction: Reaction,
    ) -> Result<(), AppError> {
        let actor = load_actor_profile(state, actor_id).await?;
        Self::dispatch_persistent(
            state,
            CreateNotificationInput {
                destinatario_id: recipient_id,
                tipo: "like".into(),
                titulo: String::new(),
                mensaje: format!(
                    "@{} {} tu publicacion",
                    actor.username,
                    if matches!(reaction, Reaction::Encanta) {
                        "le encanta"
                    } else {
                        "le gusta"
                    }
                ),
                datos: serde_json::json!({
                    "liker_id": actor_id,
                    "publicacion_id": post_id,
                    "reaccion": reaction.as_db_str(),
                }),
                actor_id: Some(actor_id),
                enlace: Some(format!("/publicacion/{post_id}/")),
            },
            actor,
        )
        .await
    }

    pub async fn dispatch_comment_reaction(
        state: &AppState,
        recipient_id: i32,
        actor_id: i32,
        comment_id: i32,
    ) -> Result<(), AppError> {
        let actor = load_actor_profile(state, actor_id).await?;
        Self::dispatch_persistent(
            state,
            CreateNotificationInput {
                destinatario_id: recipient_id,
                tipo: "like".into(),
                titulo: String::new(),
                mensaje: format!("@{} le gusta tu comentario", actor.username),
                datos: serde_json::json!({
                    "liker_id": actor_id,
                    "comentario_id": comment_id,
                }),
                actor_id: Some(actor_id),
                enlace: None,
            },
            actor,
        )
        .await
    }

    pub async fn dispatch_sample_comment(
        state: &AppState,
        recipient_id: i32,
        actor_id: i32,
        sample_id: i32,
        sample_title: &str,
        sample_slug: Option<&str>,
    ) -> Result<(), AppError> {
        let actor = load_actor_profile(state, actor_id).await?;
        Self::dispatch_persistent(
            state,
            CreateNotificationInput {
                destinatario_id: recipient_id,
                tipo: "comentario".into(),
                titulo: String::new(),
                mensaje: format!(
                    "@{} comento en tu sample \"{}\"",
                    actor.username, sample_title
                ),
                datos: serde_json::json!({
                    "commenter_id": actor_id,
                    "sample_id": sample_id,
                    "sampleSlug": sample_slug,
                    "sampleTitulo": sample_title,
                }),
                actor_id: Some(actor_id),
                enlace: sample_slug.map(|slug| format!("/sample/{slug}/")),
            },
            actor,
        )
        .await
    }

    pub async fn dispatch_post_comment(
        state: &AppState,
        recipient_id: i32,
        actor_id: i32,
        post_id: i32,
        post_content: &str,
    ) -> Result<(), AppError> {
        let actor = load_actor_profile(state, actor_id).await?;
        let snippet = compact_snippet(post_content, 40);
        let message = if snippet.is_empty() {
            format!("@{} comento en tu publicacion", actor.username)
        } else {
            format!(
                "@{} comento en tu publicacion \"{}\"",
                actor.username, snippet
            )
        };

        Self::dispatch_persistent(
            state,
            CreateNotificationInput {
                destinatario_id: recipient_id,
                tipo: "comentario".into(),
                titulo: String::new(),
                mensaje: message,
                datos: serde_json::json!({
                    "commenter_id": actor_id,
                    "publicacion_id": post_id,
                }),
                actor_id: Some(actor_id),
                enlace: Some(format!("/publicacion/{post_id}/")),
            },
            actor,
        )
        .await
    }

    pub async fn dispatch_comment_reply(
        state: &AppState,
        recipient_id: i32,
        actor_id: i32,
        parent_comment_id: i32,
        sample_id: Option<i32>,
        sample_slug: Option<&str>,
    ) -> Result<(), AppError> {
        let actor = load_actor_profile(state, actor_id).await?;
        Self::dispatch_persistent(
            state,
            CreateNotificationInput {
                destinatario_id: recipient_id,
                tipo: "comentario".into(),
                titulo: String::new(),
                mensaje: format!("@{} respondio a tu comentario", actor.username),
                datos: serde_json::json!({
                    "replier_id": actor_id,
                    "comentario_padre_id": parent_comment_id,
                    "sample_id": sample_id,
                    "sampleSlug": sample_slug,
                }),
                actor_id: Some(actor_id),
                enlace: sample_slug.map(|slug| format!("/sample/{slug}/")),
            },
            actor,
        )
        .await
    }

    pub async fn dispatch_new_message(
        state: &AppState,
        recipient_id: i32,
        sender_id: i32,
        message: &ConversationMessage,
    ) -> Result<(), AppError> {
        let sender = load_actor_profile(state, sender_id).await?;
        emit_message_event(state, recipient_id, message).await;

        let preview = message_preview(message);
        let body = if preview.is_empty() {
            format!("@{} te ha enviado un mensaje", sender.username)
        } else {
            format!("@{}: {preview}", sender.username)
        };
        let route = format!("/mensajes/{}/", message.conversacion_id);
        let data = serde_json::json!({
            "tipo": "mensaje_nuevo",
            "enlace": route,
            "ruta": format!("/mensajes/{}/", message.conversacion_id),
            "conversacionId": message.conversacion_id,
            "remitenteId": sender_id,
            "mensajeId": message.id,
        });

        let _ = PushNotificationService::send_to_user(
            state,
            recipient_id,
            PushNotificationPayload {
                title: "Nuevo mensaje".into(),
                body: body.clone(),
                data: data.clone(),
                tag: Some("mensaje_nuevo".into()),
                icon_url: None,
                badge_url: None,
            },
        )
        .await;

        let _ = FcmNotificationService::send_to_user(
            state,
            recipient_id,
            FcmNotificationPayload {
                title: "Nuevo mensaje".into(),
                body,
                data,
            },
        )
        .await;

        Ok(())
    }

    async fn dispatch_persistent(
        state: &AppState,
        input: CreateNotificationInput,
        actor: ActorProfile,
    ) -> Result<(), AppError> {
        let created = NotificationService::create(&state.pool, input.clone()).await?;
        if created.is_none() {
            return Ok(());
        }

        let actor_avatar = asset_to_public_url(state.public_base_url.as_deref(), actor.avatar_url);
        let payload = NotificationWsPayload {
            tipo: input.tipo.clone(),
            titulo: input.titulo.clone(),
            mensaje: input.mensaje.clone(),
            datos: input.datos.clone(),
            enlace: input.enlace.clone(),
            actor: Some(NotificationWsActor {
                username: actor.username.clone(),
                avatar_url: actor_avatar.clone(),
            }),
        };

        let _ = ws::emit_event(state, input.destinatario_id, "notificacion", &payload).await;

        let push_data = merge_push_data(&input, actor_avatar.clone());
        let push_title = if input.titulo.trim().is_empty() {
            input.tipo.clone()
        } else {
            input.titulo.clone()
        };
        let fcm_title = if input.titulo.trim().is_empty() {
            human_title_for_type(&input.tipo).to_string()
        } else {
            input.titulo.clone()
        };

        let _ = PushNotificationService::send_to_user(
            state,
            input.destinatario_id,
            PushNotificationPayload {
                title: push_title,
                body: input.mensaje.clone(),
                data: push_data.clone(),
                tag: Some(input.tipo.clone()),
                icon_url: None,
                badge_url: None,
            },
        )
        .await;

        let _ = FcmNotificationService::send_to_user(
            state,
            input.destinatario_id,
            FcmNotificationPayload {
                title: fcm_title,
                body: input.mensaje.clone(),
                data: push_data,
            },
        )
        .await;

        if email_notifications_enabled() {
            maybe_send_email_opt_in(state, input.destinatario_id, &input);
        }

        Ok(())
    }
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
