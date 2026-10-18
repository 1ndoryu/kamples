use crate::errors::AppError;
use crate::repositories::{ConversationMessage, Reaction};
use crate::services::{
    CreateNotificationInput, FcmNotificationPayload, FcmNotificationService, NotificationService,
    PushNotificationPayload, PushNotificationService,
};
use crate::ws;
use crate::AppState;

use super::{
    asset_to_public_url, compact_snippet, emit_message_event, human_title_for_type,
    load_actor_profile, maybe_send_email_opt_in, merge_push_data, message_preview, ActorProfile,
    email_notifications_enabled, NotificationFanoutService, NotificationWsActor,
    NotificationWsPayload,
};

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
