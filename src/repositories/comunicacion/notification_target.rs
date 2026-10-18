use sqlx::PgPool;

use crate::errors::AppError;

pub struct NotificationTargetRepository;

#[derive(Debug, Clone)]
pub struct SampleNotificationMeta {
    pub creator_id: i32,
    pub title: String,
    pub slug: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PostNotificationMeta {
    pub author_id: i32,
    pub content: String,
}

/* [174A-78] Lookups mínimos para fanout de notificaciones.
 * Se separan de handlers para no repetir SQL ad-hoc en likes/comentarios.
 * Mantienen el mismo criterio legacy: sólo autor/creador + texto/link mínimo. */

impl NotificationTargetRepository {
    pub async fn find_sample_meta(
        pool: &PgPool,
        sample_id: i32,
    ) -> Result<Option<SampleNotificationMeta>, AppError> {
        let row = sqlx::query!(
            r#"SELECT creador_id AS "creator_id!",
                      titulo AS "title!",
                      slug
               FROM samples
               WHERE id = $1"#,
            sample_id,
        )
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|row| SampleNotificationMeta {
            creator_id: row.creator_id,
            title: row.title,
            slug: Some(row.slug),
        }))
    }

    pub async fn find_post_meta(
        pool: &PgPool,
        post_id: i32,
    ) -> Result<Option<PostNotificationMeta>, AppError> {
        let row = sqlx::query!(
            r#"SELECT autor_id AS "author_id!",
                      contenido AS "content!"
               FROM publicaciones
               WHERE id = $1"#,
            post_id,
        )
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|row| PostNotificationMeta {
            author_id: row.author_id,
            content: row.content,
        }))
    }
}
