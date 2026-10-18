use sqlx::PgPool;

use super::{
    map_comment_row, CommentContext, CommentContextRow, CommentDetail, CommentRepository,
    CommentRow, CommentTargetKind,
};
use crate::errors::AppError;

impl CommentRepository {
    pub async fn get(
        pool: &PgPool,
        viewer_id: Option<i32>,
        comment_id: i32,
        blocked_ids: &[i32],
    ) -> Result<Option<CommentDetail>, AppError> {
        let row = sqlx::query_as!(
            CommentRow,
            r#"SELECT
                    c.id AS "id!",
                    c.autor_id AS "autor_id!",
                    c.tipo AS "tipo!",
                    c.target_id AS "target_id!",
                    COALESCE(c.contenido, '') AS "contenido!",
                    COALESCE(c.tipo_contenido, 'texto') AS "tipo_contenido!",
                    c.media_url,
                    c.media_metadata AS "media_metadata?: serde_json::Value",
                    c.parent_id,
                    COALESCE(c.total_likes, 0) AS "total_likes!",
                    COALESCE(c.total_respuestas, 0) AS "total_respuestas!",
                    c.created_at AS "created_at!: chrono::DateTime<chrono::Utc>",
                    c.updated_at AS "updated_at?: chrono::DateTime<chrono::Utc>",
                    u.id AS "author_id!",
                    u.username AS "author_username!",
                    u.nombre_visible AS "author_display_name",
                    u.avatar_url AS "author_avatar_url",
                    COALESCE(u.verificado, FALSE) AS "author_verified!",
                    (SELECT reaccion FROM likes WHERE usuario_id = $1 AND tipo = 'comentario' AND target_id = c.id LIMIT 1) AS "mi_reaccion"
               FROM comentarios c
               JOIN usuarios_ext u ON u.id = c.autor_id
               WHERE c.id = $2
                 AND (c.moderacion_estado IS NULL OR c.moderacion_estado != 'rechazado')
                 AND NOT (c.autor_id = ANY($3::int[]))"#,
            viewer_id,
            comment_id,
            blocked_ids,
        )
        .fetch_optional(pool)
        .await?;
        Ok(row.map(map_comment_row))
    }

    pub async fn list_roots(
        pool: &PgPool,
        viewer_id: Option<i32>,
        target_kind: CommentTargetKind,
        target_id: i32,
        blocked_ids: &[i32],
        limit: i64,
        offset: i64,
    ) -> Result<Vec<CommentDetail>, AppError> {
        let rows = sqlx::query_as!(
            CommentRow,
            r#"SELECT
                    c.id AS "id!",
                    c.autor_id AS "autor_id!",
                    c.tipo AS "tipo!",
                    c.target_id AS "target_id!",
                    COALESCE(c.contenido, '') AS "contenido!",
                    COALESCE(c.tipo_contenido, 'texto') AS "tipo_contenido!",
                    c.media_url,
                    c.media_metadata AS "media_metadata?: serde_json::Value",
                    c.parent_id,
                    COALESCE(c.total_likes, 0) AS "total_likes!",
                    COALESCE(c.total_respuestas, 0) AS "total_respuestas!",
                    c.created_at AS "created_at!: chrono::DateTime<chrono::Utc>",
                    c.updated_at AS "updated_at?: chrono::DateTime<chrono::Utc>",
                    u.id AS "author_id!",
                    u.username AS "author_username!",
                    u.nombre_visible AS "author_display_name",
                    u.avatar_url AS "author_avatar_url",
                    COALESCE(u.verificado, FALSE) AS "author_verified!",
                    (SELECT reaccion FROM likes WHERE usuario_id = $1 AND tipo = 'comentario' AND target_id = c.id LIMIT 1) AS "mi_reaccion"
               FROM comentarios c
               JOIN usuarios_ext u ON u.id = c.autor_id
               WHERE c.tipo = $2
                 AND c.target_id = $3
                 AND c.parent_id IS NULL
                 AND (c.moderacion_estado IS NULL OR c.moderacion_estado != 'rechazado')
                 AND NOT (c.autor_id = ANY($4::int[]))
               ORDER BY c.total_likes DESC, c.created_at DESC
               LIMIT $5 OFFSET $6"#,
            viewer_id,
            target_kind.as_db_str(),
            target_id,
            blocked_ids,
            limit,
            offset,
        )
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(map_comment_row).collect())
    }

    pub async fn list_replies(
        pool: &PgPool,
        viewer_id: Option<i32>,
        parent_id: i32,
        blocked_ids: &[i32],
        limit: i64,
    ) -> Result<Vec<CommentDetail>, AppError> {
        let rows = sqlx::query_as!(
            CommentRow,
            r#"SELECT
                    c.id AS "id!",
                    c.autor_id AS "autor_id!",
                    c.tipo AS "tipo!",
                    c.target_id AS "target_id!",
                    COALESCE(c.contenido, '') AS "contenido!",
                    COALESCE(c.tipo_contenido, 'texto') AS "tipo_contenido!",
                    c.media_url,
                    c.media_metadata AS "media_metadata?: serde_json::Value",
                    c.parent_id,
                    COALESCE(c.total_likes, 0) AS "total_likes!",
                    COALESCE(c.total_respuestas, 0) AS "total_respuestas!",
                    c.created_at AS "created_at!: chrono::DateTime<chrono::Utc>",
                    c.updated_at AS "updated_at?: chrono::DateTime<chrono::Utc>",
                    u.id AS "author_id!",
                    u.username AS "author_username!",
                    u.nombre_visible AS "author_display_name",
                    u.avatar_url AS "author_avatar_url",
                    COALESCE(u.verificado, FALSE) AS "author_verified!",
                    (SELECT reaccion FROM likes WHERE usuario_id = $1 AND tipo = 'comentario' AND target_id = c.id LIMIT 1) AS "mi_reaccion"
               FROM comentarios c
               JOIN usuarios_ext u ON u.id = c.autor_id
               WHERE c.parent_id = $2
                 AND (c.moderacion_estado IS NULL OR c.moderacion_estado != 'rechazado')
                 AND NOT (c.autor_id = ANY($3::int[]))
               ORDER BY c.created_at ASC
               LIMIT $4"#,
            viewer_id,
            parent_id,
            blocked_ids,
            limit,
        )
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(map_comment_row).collect())
    }

    pub async fn find_context(
        pool: &PgPool,
        comment_id: i32,
    ) -> Result<Option<CommentContext>, AppError> {
        let row = sqlx::query_as!(
            CommentContextRow,
            r#"SELECT
                    id AS "id!",
                    autor_id AS "autor_id!",
                    tipo AS "tipo!",
                    target_id AS "target_id!",
                    parent_id,
                    media_url,
                    COALESCE(tipo_contenido, 'texto') AS "tipo_contenido!"
               FROM comentarios
               WHERE id = $1"#,
            comment_id,
        )
        .fetch_optional(pool)
        .await?;

        row.map(TryInto::try_into).transpose()
    }

    pub async fn list_media_urls_for_thread(
        pool: &PgPool,
        comment_id: i32,
    ) -> Result<Vec<String>, AppError> {
        let urls = sqlx::query_scalar!(
            r#"WITH RECURSIVE comment_tree AS (
                    SELECT id, media_url
                    FROM comentarios
                    WHERE id = $1
                    UNION ALL
                    SELECT c.id, c.media_url
                    FROM comentarios c
                    JOIN comment_tree ct ON c.parent_id = ct.id
                )
                SELECT media_url AS "media_url!"
                FROM comment_tree
                WHERE media_url IS NOT NULL"#,
            comment_id,
        )
        .fetch_all(pool)
        .await?;
        Ok(urls)
    }
}
