use sqlx::PgPool;

use crate::errors::AppError;
use crate::repositories::{CreateNotificationRecord, NotificationRepository, UserNotification};

pub const DEFAULT_NOTIFICATION_PAGE_SIZE: i64 = 30;

#[derive(Debug, Clone)]
pub struct CreateNotificationInput {
    pub destinatario_id: i32,
    pub tipo: String,
    pub titulo: String,
    pub mensaje: String,
    pub datos: serde_json::Value,
    pub actor_id: Option<i32>,
    pub enlace: Option<String>,
}

pub struct NotificationService;

/* [174A-74] Base de notificaciones persistentes.
 * Este servicio concentra exclusión de auto-notificaciones, deduplicación
 * temporal y las operaciones de lectura/marcado usadas por el dropdown legacy.
 * El fanout WS/push/email queda para 174A-78. */

impl NotificationService {
    pub async fn list_for_user(
        pool: &PgPool,
        user_id: i32,
        hidden_actor_ids: &[i32],
        page: i64,
    ) -> Result<Vec<UserNotification>, AppError> {
        let page = page.max(1);
        let offset = (page - 1) * DEFAULT_NOTIFICATION_PAGE_SIZE;
        NotificationRepository::list_for_user(
            pool,
            user_id,
            hidden_actor_ids,
            DEFAULT_NOTIFICATION_PAGE_SIZE,
            offset,
        )
        .await
    }

    pub async fn mark_read(
        pool: &PgPool,
        user_id: i32,
        notification_id: i32,
    ) -> Result<(), AppError> {
        NotificationRepository::mark_read(pool, notification_id, user_id).await
    }

    pub async fn mark_all_read(pool: &PgPool, user_id: i32) -> Result<(), AppError> {
        NotificationRepository::mark_all_read(pool, user_id).await
    }

    pub async fn unread_count(pool: &PgPool, user_id: i32) -> Result<i64, AppError> {
        NotificationRepository::unread_count(pool, user_id).await
    }

    pub async fn create(
        pool: &PgPool,
        input: CreateNotificationInput,
    ) -> Result<Option<i32>, AppError> {
        if should_skip_self_notification(input.destinatario_id, input.actor_id) {
            return Ok(None);
        }

        let payload = normalize_payload(input.datos);
        if let (Some(actor_id), Some(window)) = (input.actor_id, dedup_window_seconds(&input.tipo))
        {
            let exists = NotificationRepository::exists_recent(
                pool,
                input.destinatario_id,
                &input.tipo,
                Some(actor_id),
                window,
                Some(payload.clone()),
            )
            .await?;
            if exists {
                return Ok(None);
            }
        }

        let notification_id = NotificationRepository::create_complete(
            pool,
            CreateNotificationRecord {
                recipient_id: input.destinatario_id,
                notification_type: &input.tipo,
                title: &input.titulo,
                message: &input.mensaje,
                data: &payload,
                actor_id: input.actor_id,
                link: input.enlace.as_deref(),
            },
        )
        .await?;

        Ok(Some(notification_id))
    }
}

fn dedup_window_seconds(notification_type: &str) -> Option<i32> {
    match notification_type {
        "like" | "encanta" | "follow" => Some(86_400),
        "comentario" => Some(300),
        "venta" => Some(3_600),
        _ => None,
    }
}

fn normalize_payload(payload: serde_json::Value) -> serde_json::Value {
    if payload.is_null() {
        serde_json::json!({})
    } else {
        payload
    }
}

const fn should_skip_self_notification(recipient_id: i32, actor_id: Option<i32>) -> bool {
    matches!(actor_id, Some(actor_id) if actor_id == recipient_id)
}

#[cfg(test)]
mod tests {
    use super::{dedup_window_seconds, normalize_payload, should_skip_self_notification};

    #[test]
    fn dedup_windows_match_legacy_rules() {
        assert_eq!(dedup_window_seconds("like"), Some(86_400));
        assert_eq!(dedup_window_seconds("encanta"), Some(86_400));
        assert_eq!(dedup_window_seconds("follow"), Some(86_400));
        assert_eq!(dedup_window_seconds("comentario"), Some(300));
        assert_eq!(dedup_window_seconds("venta"), Some(3_600));
        assert_eq!(dedup_window_seconds("mensaje"), None);
        assert_eq!(dedup_window_seconds("sistema"), None);
    }

    #[test]
    fn self_notifications_are_skipped() {
        assert!(should_skip_self_notification(7, Some(7)));
        assert!(!should_skip_self_notification(7, Some(9)));
        assert!(!should_skip_self_notification(7, None));
    }

    #[test]
    fn null_payload_becomes_empty_object() {
        let payload = normalize_payload(serde_json::Value::Null);
        assert_eq!(payload, serde_json::json!({}));
    }
}
