use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;

use crate::errors::AppError;

pub struct NotificationRepository;

pub struct CreateNotificationRecord<'a> {
    pub recipient_id: i32,
    pub notification_type: &'a str,
    pub title: &'a str,
    pub message: &'a str,
    pub data: &'a serde_json::Value,
    pub actor_id: Option<i32>,
    pub link: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NotificationActor {
    pub username: String,
    pub nombre_visible: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct UserNotification {
    pub id: i32,
    pub tipo: String,
    pub titulo: String,
    pub mensaje: String,
    #[schema(value_type = Object)]
    pub datos: serde_json::Value,
    pub leida: bool,
    pub enlace: Option<String>,
    #[serde(rename = "creadaAt")]
    #[schema(value_type = String, format = DateTime)]
    pub creada_at: chrono::DateTime<chrono::Utc>,
    pub actor: Option<NotificationActor>,
}

#[derive(Debug)]
struct NotificationRow {
    id: i32,
    tipo: String,
    titulo: String,
    mensaje: String,
    datos: serde_json::Value,
    leida: bool,
    enlace: Option<String>,
    creada_at: chrono::DateTime<chrono::Utc>,
    actor_username: Option<String>,
    actor_display_name: Option<String>,
    actor_avatar_url: Option<String>,
}

impl NotificationRepository {
    pub async fn list_for_user(
        pool: &PgPool,
        user_id: i32,
        hidden_actor_ids: &[i32],
        limit: i64,
        offset: i64,
    ) -> Result<Vec<UserNotification>, AppError> {
        let rows: Vec<NotificationRow> = sqlx::query_as!(
            NotificationRow,
            r#"SELECT n.id AS "id!",
                      COALESCE(n.tipo, 'sistema') AS "tipo!",
                      COALESCE(n.titulo, '') AS "titulo!",
                      COALESCE(n.mensaje, '') AS "mensaje!",
                      COALESCE(n.datos, '{}'::jsonb) AS "datos!: serde_json::Value",
                      COALESCE(n.leida, FALSE) AS "leida!",
                      n.enlace AS "enlace",
                      n.created_at AS "creada_at!: chrono::DateTime<chrono::Utc>",
                      u.username AS "actor_username?",
                      COALESCE(u.nombre_visible, u.username) AS "actor_display_name?",
                      u.avatar_url AS "actor_avatar_url?"
               FROM notificaciones n
               LEFT JOIN usuarios_ext u
                 ON u.id = n.actor_id
                AND u.estado = 'activo'
               WHERE n.usuario_id = $1
                 AND (n.actor_id IS NULL OR u.id IS NOT NULL)
                 AND NOT COALESCE(n.actor_id = ANY($2::int4[]), FALSE)
               ORDER BY n.created_at DESC, n.id DESC
               LIMIT $3 OFFSET $4"#,
            user_id,
            hidden_actor_ids,
            limit,
            offset,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(map_notification_row).collect())
    }

    pub async fn mark_read(
        pool: &PgPool,
        notification_id: i32,
        user_id: i32,
    ) -> Result<(), AppError> {
        sqlx::query!(
            r#"UPDATE notificaciones
               SET leida = TRUE
               WHERE id = $1 AND usuario_id = $2"#,
            notification_id,
            user_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn mark_all_read(pool: &PgPool, user_id: i32) -> Result<(), AppError> {
        sqlx::query!(
            r#"UPDATE notificaciones
               SET leida = TRUE
               WHERE usuario_id = $1 AND leida = FALSE"#,
            user_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn unread_count(pool: &PgPool, user_id: i32) -> Result<i64, AppError> {
        let total = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!"
               FROM notificaciones
               WHERE usuario_id = $1 AND leida = FALSE"#,
            user_id,
        )
        .fetch_one(pool)
        .await?;
        Ok(total)
    }

    pub async fn create_complete(
        pool: &PgPool,
        record: CreateNotificationRecord<'_>,
    ) -> Result<i32, AppError> {
        let notification_id = sqlx::query_scalar!(
            r#"INSERT INTO notificaciones (
                    usuario_id,
                    tipo,
                    titulo,
                    mensaje,
                    datos,
                    actor_id,
                    enlace
               )
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING id AS "id!""#,
            record.recipient_id,
            record.notification_type,
            record.title,
            record.message,
            record.data,
            record.actor_id,
            record.link,
        )
        .fetch_one(pool)
        .await?;

        Ok(notification_id)
    }

    pub async fn exists_recent(
        pool: &PgPool,
        recipient_id: i32,
        notification_type: &str,
        actor_id: Option<i32>,
        interval_seconds: i32,
        payload: Option<serde_json::Value>,
    ) -> Result<bool, AppError> {
        let exists = sqlx::query_scalar!(
            r#"SELECT EXISTS(
                    SELECT 1
                    FROM notificaciones
                    WHERE usuario_id = $1
                      AND tipo = $2
                                            AND created_at > NOW() - ($3::int4 * INTERVAL '1 second')
                      AND (($4::int4 IS NULL AND actor_id IS NULL) OR actor_id = $4)
                      AND ($5::jsonb IS NULL OR datos = $5)
                ) AS "exists!""#,
            recipient_id,
            notification_type,
            interval_seconds,
            actor_id,
            payload,
        )
        .fetch_one(pool)
        .await?;

        Ok(exists)
    }
}

fn map_notification_row(row: NotificationRow) -> UserNotification {
    let actor = row.actor_username.map(|username| NotificationActor {
        nombre_visible: row.actor_display_name.unwrap_or_else(|| username.clone()),
        avatar_url: row.actor_avatar_url,
        username,
    });

    UserNotification {
        id: row.id,
        tipo: row.tipo,
        titulo: row.titulo,
        mensaje: row.mensaje,
        datos: row.datos,
        leida: row.leida,
        enlace: row.enlace,
        creada_at: row.creada_at,
        actor,
    }
}
