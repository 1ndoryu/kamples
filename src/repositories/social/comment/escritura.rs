use sqlx::PgPool;

use super::{CommentRepository, CommentTargetKind, CreateCommentParams};
use crate::errors::AppError;

impl CommentRepository {
    pub async fn target_exists(
        pool: &PgPool,
        kind: CommentTargetKind,
        target_id: i32,
    ) -> Result<bool, AppError> {
        let exists = match kind {
            CommentTargetKind::Sample => sqlx::query_scalar!(
                "SELECT EXISTS(SELECT 1 FROM samples WHERE id = $1 AND eliminado_en IS NULL) AS \"e!\"",
                target_id,
            )
            .fetch_one(pool)
            .await?,
            CommentTargetKind::Publicacion => sqlx::query_scalar!(
                "SELECT EXISTS(SELECT 1 FROM publicaciones WHERE id = $1 AND eliminado_en IS NULL) AS \"e!\"",
                target_id,
            )
            .fetch_one(pool)
            .await?,
            CommentTargetKind::Cancion => sqlx::query_scalar!(
                "SELECT EXISTS(SELECT 1 FROM canciones WHERE id = $1) AS \"e!\"",
                target_id,
            )
            .fetch_one(pool)
            .await?,
            CommentTargetKind::Relacion => sqlx::query_scalar!(
                "SELECT EXISTS(SELECT 1 FROM relaciones_sample WHERE id = $1) AS \"e!\"",
                target_id,
            )
            .fetch_one(pool)
            .await?,
            CommentTargetKind::Articulo => sqlx::query_scalar!(
                "SELECT EXISTS(SELECT 1 FROM articulos WHERE id = $1 AND eliminado_en IS NULL) AS \"e!\"",
                target_id,
            )
            .fetch_one(pool)
            .await?,
        };
        Ok(exists)
    }

    pub async fn validate_parent_context(
        pool: &PgPool,
        parent_id: i32,
        kind: CommentTargetKind,
        target_id: i32,
    ) -> Result<bool, AppError> {
        let exists = sqlx::query_scalar!(
            r#"SELECT EXISTS(
                    SELECT 1
                    FROM comentarios
                    WHERE id = $1
                      AND tipo = $2
                      AND target_id = $3
                      AND (moderacion_estado IS NULL OR moderacion_estado != 'rechazado')
                                ) AS "e!""#,
            parent_id,
            kind.as_db_str(),
            target_id,
        )
        .fetch_one(pool)
        .await?;
        Ok(exists)
    }

    pub async fn create(pool: &PgPool, params: CreateCommentParams<'_>) -> Result<i32, AppError> {
        let id = sqlx::query_scalar!(
            r#"INSERT INTO comentarios (
                    autor_id, tipo, target_id, contenido, tipo_contenido, media_url, media_metadata, parent_id
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                RETURNING id AS "id!""#,
            params.autor_id,
            params.target_kind.as_db_str(),
            params.target_id,
            params.contenido,
            params.content_kind.as_db_str(),
            params.media_url,
            params.media_metadata,
            params.parent_id,
        )
        .fetch_one(pool)
        .await?;
        Ok(id)
    }

    pub async fn update_content(
        pool: &PgPool,
        comment_id: i32,
        autor_id: i32,
        contenido: &str,
    ) -> Result<bool, AppError> {
        let updated = sqlx::query!(
            r#"UPDATE comentarios
               SET contenido = $3,
                   updated_at = NOW()
               WHERE id = $1 AND autor_id = $2"#,
            comment_id,
            autor_id,
            contenido,
        )
        .execute(pool)
        .await?
        .rows_affected();
        Ok(updated > 0)
    }

    pub async fn delete(pool: &PgPool, comment_id: i32) -> Result<bool, AppError> {
        let deleted = sqlx::query!("DELETE FROM comentarios WHERE id = $1", comment_id)
            .execute(pool)
            .await?
            .rows_affected();
        Ok(deleted > 0)
    }

    pub async fn increment_replies(pool: &PgPool, comment_id: i32) -> Result<(), AppError> {
        sqlx::query!(
            "UPDATE comentarios SET total_respuestas = total_respuestas + 1 WHERE id = $1",
            comment_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn decrement_replies(pool: &PgPool, comment_id: i32) -> Result<(), AppError> {
        sqlx::query!(
            "UPDATE comentarios SET total_respuestas = GREATEST(total_respuestas - 1, 0) WHERE id = $1",
            comment_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn recount_target(
        pool: &PgPool,
        kind: CommentTargetKind,
        target_id: i32,
    ) -> Result<(), AppError> {
        match kind {
            CommentTargetKind::Sample => {
                sqlx::query!(
                    "UPDATE samples SET total_comentarios = (SELECT COUNT(*) FROM comentarios WHERE tipo = 'sample' AND target_id = $1) WHERE id = $1",
                    target_id,
                )
                .execute(pool)
                .await?;
            }
            CommentTargetKind::Publicacion => {
                sqlx::query!(
                    "UPDATE publicaciones SET total_comentarios = (SELECT COUNT(*) FROM comentarios WHERE tipo = 'publicacion' AND target_id = $1) WHERE id = $1",
                    target_id,
                )
                .execute(pool)
                .await?;
            }
            CommentTargetKind::Cancion => {
                sqlx::query!(
                    "UPDATE canciones SET total_comentarios = (SELECT COUNT(*) FROM comentarios WHERE tipo = 'cancion' AND target_id = $1) WHERE id = $1",
                    target_id,
                )
                .execute(pool)
                .await?;
            }
            CommentTargetKind::Relacion => {
                sqlx::query!(
                    "UPDATE relaciones_sample SET total_comentarios = (SELECT COUNT(*) FROM comentarios WHERE tipo = 'relacion' AND target_id = $1) WHERE id = $1",
                    target_id,
                )
                .execute(pool)
                .await?;
            }
            CommentTargetKind::Articulo => {
                sqlx::query!(
                    "UPDATE articulos SET total_comentarios = (SELECT COUNT(*) FROM comentarios WHERE tipo = 'articulo' AND target_id = $1) WHERE id = $1",
                    target_id,
                )
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }
}
