use sqlx::PgPool;

use super::types::{
    map_detail_row, map_summary_row, ArticleCategoryCount, ArticleDetail, ArticleDetailRow,
    ArticleMeta, ArticleMetaRow, ArticleSummary, ArticleSummaryRow,
};
use super::ArticleRepository;
use crate::errors::AppError;

impl ArticleRepository {
    pub async fn list_published(
        pool: &PgPool,
        categoria: Option<&str>,
        viewer_id: Option<i32>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ArticleSummary>, AppError> {
        let rows = sqlx::query_as!(
            ArticleSummaryRow,
            r#"SELECT
                    a.id AS "id!",
                    a.autor_id AS "autor_id!",
                    a.titulo AS "titulo!",
                    a.slug AS "slug!",
                    a.extracto AS "extracto!",
                    a.portada_url,
                    a.categoria AS "categoria!",
                    COALESCE(a.total_likes, 0) AS "total_likes!",
                    COALESCE(a.total_comentarios, 0) AS "total_comentarios!",
                    a.moderacion_estado AS "moderacion_estado!",
                    a.created_at AS "created_at!: chrono::DateTime<chrono::Utc>",
                    a.updated_at AS "updated_at!: chrono::DateTime<chrono::Utc>",
                    a.publicado_en AS "publicado_en?: chrono::DateTime<chrono::Utc>",
                    u.id AS "author_id!",
                    u.username AS "author_username!",
                    u.nombre_visible AS "author_display_name",
                    u.avatar_url AS "author_avatar_url",
                    COALESCE(u.verificado, FALSE) AS "author_verified!",
                    COALESCE(al.usuario_id IS NOT NULL, FALSE) AS "liked_por_mi!"
               FROM articulos a
               JOIN usuarios_ext u ON u.id = a.autor_id
               LEFT JOIN articulos_likes al ON al.articulo_id = a.id AND al.usuario_id = $3
               WHERE a.eliminado_en IS NULL
                 AND a.moderacion_estado = 'aprobado'
                 AND ($1::text IS NULL OR a.categoria = $1)
               ORDER BY a.publicado_en DESC NULLS LAST, a.created_at DESC, a.id DESC
               LIMIT $2 OFFSET $4"#,
            categoria,
            limit,
            viewer_id,
            offset,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(map_summary_row).collect())
    }

    pub async fn count_published(pool: &PgPool, categoria: Option<&str>) -> Result<i64, AppError> {
        let total = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!"
               FROM articulos
               WHERE eliminado_en IS NULL
                 AND moderacion_estado = 'aprobado'
                 AND ($1::text IS NULL OR categoria = $1)"#,
            categoria,
        )
        .fetch_one(pool)
        .await?;
        Ok(total)
    }

    pub async fn get_by_slug(
        pool: &PgPool,
        slug: &str,
        viewer_id: Option<i32>,
    ) -> Result<Option<ArticleDetail>, AppError> {
        let row = sqlx::query_as!(
            ArticleDetailRow,
            r#"SELECT
                    a.id AS "id!",
                    a.autor_id AS "autor_id!",
                    a.titulo AS "titulo!",
                    a.slug AS "slug!",
                    a.contenido AS "contenido!",
                    a.extracto AS "extracto!",
                    a.portada_url,
                    a.categoria AS "categoria!",
                    a.embeds AS "embeds!: serde_json::Value",
                    a.descarga_publica AS "descarga_publica!",
                    COALESCE(a.total_likes, 0) AS "total_likes!",
                    COALESCE(a.total_comentarios, 0) AS "total_comentarios!",
                    a.moderacion_estado AS "moderacion_estado!",
                    a.created_at AS "created_at!: chrono::DateTime<chrono::Utc>",
                    a.updated_at AS "updated_at!: chrono::DateTime<chrono::Utc>",
                    a.publicado_en AS "publicado_en?: chrono::DateTime<chrono::Utc>",
                    u.id AS "author_id!",
                    u.username AS "author_username!",
                    u.nombre_visible AS "author_display_name",
                    u.avatar_url AS "author_avatar_url",
                    COALESCE(u.verificado, FALSE) AS "author_verified!",
                    COALESCE(al.usuario_id IS NOT NULL, FALSE) AS "liked_por_mi!"
               FROM articulos a
               JOIN usuarios_ext u ON u.id = a.autor_id
               LEFT JOIN articulos_likes al ON al.articulo_id = a.id AND al.usuario_id = $2
               WHERE a.slug = $1
                 AND a.eliminado_en IS NULL
               LIMIT 1"#,
            slug,
            viewer_id,
        )
        .fetch_optional(pool)
        .await?;

        Ok(row.map(map_detail_row))
    }

    pub async fn get_by_id(
        pool: &PgPool,
        article_id: i32,
        viewer_id: Option<i32>,
    ) -> Result<Option<ArticleDetail>, AppError> {
        let row = sqlx::query_as!(
            ArticleDetailRow,
            r#"SELECT
                    a.id AS "id!",
                    a.autor_id AS "autor_id!",
                    a.titulo AS "titulo!",
                    a.slug AS "slug!",
                    a.contenido AS "contenido!",
                    a.extracto AS "extracto!",
                    a.portada_url,
                    a.categoria AS "categoria!",
                    a.embeds AS "embeds!: serde_json::Value",
                    a.descarga_publica AS "descarga_publica!",
                    COALESCE(a.total_likes, 0) AS "total_likes!",
                    COALESCE(a.total_comentarios, 0) AS "total_comentarios!",
                    a.moderacion_estado AS "moderacion_estado!",
                    a.created_at AS "created_at!: chrono::DateTime<chrono::Utc>",
                    a.updated_at AS "updated_at!: chrono::DateTime<chrono::Utc>",
                    a.publicado_en AS "publicado_en?: chrono::DateTime<chrono::Utc>",
                    u.id AS "author_id!",
                    u.username AS "author_username!",
                    u.nombre_visible AS "author_display_name",
                    u.avatar_url AS "author_avatar_url",
                    COALESCE(u.verificado, FALSE) AS "author_verified!",
                    COALESCE(al.usuario_id IS NOT NULL, FALSE) AS "liked_por_mi!"
               FROM articulos a
               JOIN usuarios_ext u ON u.id = a.autor_id
               LEFT JOIN articulos_likes al ON al.articulo_id = a.id AND al.usuario_id = $2
               WHERE a.id = $1
                 AND a.eliminado_en IS NULL
               LIMIT 1"#,
            article_id,
            viewer_id,
        )
        .fetch_optional(pool)
        .await?;

        Ok(row.map(map_detail_row))
    }

    pub async fn find_meta_by_id(
        pool: &PgPool,
        article_id: i32,
    ) -> Result<Option<ArticleMeta>, AppError> {
        let row = sqlx::query_as!(
            ArticleMetaRow,
            r#"SELECT
                    id AS "id!",
                    autor_id AS "autor_id!",
                    moderacion_estado AS "moderacion_estado!",
                    eliminado_en AS "eliminado_en?: chrono::DateTime<chrono::Utc>"
               FROM articulos
               WHERE id = $1
               LIMIT 1"#,
            article_id,
        )
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|row| ArticleMeta {
            id: row.id,
            autor_id: row.autor_id,
            moderacion_estado: row.moderacion_estado,
            eliminado_en: row.eliminado_en,
        }))
    }

    pub async fn list_by_author(
        pool: &PgPool,
        author_id: i32,
        moderacion_estado: Option<&str>,
        viewer_id: Option<i32>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ArticleSummary>, AppError> {
        let rows = sqlx::query_as!(
            ArticleSummaryRow,
            r#"SELECT
                    a.id AS "id!",
                    a.autor_id AS "autor_id!",
                    a.titulo AS "titulo!",
                    a.slug AS "slug!",
                    a.extracto AS "extracto!",
                    a.portada_url,
                    a.categoria AS "categoria!",
                    COALESCE(a.total_likes, 0) AS "total_likes!",
                    COALESCE(a.total_comentarios, 0) AS "total_comentarios!",
                    a.moderacion_estado AS "moderacion_estado!",
                    a.created_at AS "created_at!: chrono::DateTime<chrono::Utc>",
                    a.updated_at AS "updated_at!: chrono::DateTime<chrono::Utc>",
                    a.publicado_en AS "publicado_en?: chrono::DateTime<chrono::Utc>",
                    u.id AS "author_id!",
                    u.username AS "author_username!",
                    u.nombre_visible AS "author_display_name",
                    u.avatar_url AS "author_avatar_url",
                    COALESCE(u.verificado, FALSE) AS "author_verified!",
                    COALESCE(al.usuario_id IS NOT NULL, FALSE) AS "liked_por_mi!"
               FROM articulos a
               JOIN usuarios_ext u ON u.id = a.autor_id
               LEFT JOIN articulos_likes al ON al.articulo_id = a.id AND al.usuario_id = $3
               WHERE a.autor_id = $1
                 AND a.eliminado_en IS NULL
                 AND ($2::text IS NULL OR a.moderacion_estado = $2)
               ORDER BY a.created_at DESC, a.id DESC
               LIMIT $4 OFFSET $5"#,
            author_id,
            moderacion_estado,
            viewer_id,
            limit,
            offset,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(map_summary_row).collect())
    }

    pub async fn count_by_author(
        pool: &PgPool,
        author_id: i32,
        moderacion_estado: Option<&str>,
    ) -> Result<i64, AppError> {
        let total = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!"
               FROM articulos
               WHERE autor_id = $1
                 AND eliminado_en IS NULL
                 AND ($2::text IS NULL OR moderacion_estado = $2)"#,
            author_id,
            moderacion_estado,
        )
        .fetch_one(pool)
        .await?;
        Ok(total)
    }

    pub async fn count_by_category(pool: &PgPool) -> Result<Vec<ArticleCategoryCount>, AppError> {
        let rows = sqlx::query!(
            r#"SELECT categoria AS "categoria!", COUNT(*)::bigint AS "total!"
               FROM articulos
               WHERE eliminado_en IS NULL
                 AND moderacion_estado = 'aprobado'
               GROUP BY categoria
                             ORDER BY COUNT(*) DESC, categoria ASC"#,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| ArticleCategoryCount {
                categoria: row.categoria,
                total: row.total,
            })
            .collect())
    }
}
