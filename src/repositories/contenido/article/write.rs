use chrono::{DateTime, Utc};
use sqlx::PgPool;

use super::types::{CreateArticleParams, UpdateArticleParams};
use super::ArticleRepository;
use crate::errors::AppError;

impl ArticleRepository {
    pub async fn count_recent_by_author(
        pool: &PgPool,
        author_id: i32,
        since: DateTime<Utc>,
    ) -> Result<i64, AppError> {
        let total = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!"
               FROM articulos
               WHERE autor_id = $1
                 AND eliminado_en IS NULL
                 AND created_at >= $2"#,
            author_id,
            since,
        )
        .fetch_one(pool)
        .await?;
        Ok(total)
    }

    pub async fn generate_unique_slug(
        pool: &PgPool,
        title: &str,
        exclude_id: Option<i32>,
    ) -> Result<String, AppError> {
        let mut base_slug = slug::slugify(title);
        if base_slug.is_empty() {
            base_slug = "articulo".to_string();
        }
        if base_slug.len() > 280 {
            base_slug.truncate(280);
            base_slug = base_slug.trim_matches('-').to_string();
        }
        if base_slug.is_empty() {
            base_slug = "articulo".to_string();
        }

        for counter in 0..100 {
            let candidate = if counter == 0 {
                base_slug.clone()
            } else {
                format!("{base_slug}-{counter}")
            };
            let exists = sqlx::query_scalar!(
                r#"SELECT COUNT(*) AS "count!"
                   FROM articulos
                   WHERE slug = $1
                     AND ($2::int IS NULL OR id <> $2)"#,
                candidate,
                exclude_id,
            )
            .fetch_one(pool)
            .await?;
            if exists == 0 {
                return Ok(candidate);
            }
        }

        Ok(format!("{}-{}", base_slug, uuid::Uuid::new_v4().simple()))
    }

    pub async fn create(pool: &PgPool, params: &CreateArticleParams) -> Result<i32, AppError> {
        let article_id = sqlx::query_scalar!(
            r#"INSERT INTO articulos (
                    autor_id,
                    titulo,
                    slug,
                    contenido,
                    extracto,
                    portada_url,
                    categoria,
                    embeds,
                    descarga_publica,
                    moderacion_estado,
                    publicado_en
               )
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
               RETURNING id AS "id!""#,
            params.autor_id,
            params.titulo,
            params.slug,
            params.contenido,
            params.extracto,
            params.portada_url,
            params.categoria,
            params.embeds.clone(),
            params.descarga_publica,
            params.moderacion_estado,
            params.publicado_en,
        )
        .fetch_one(pool)
        .await?;
        Ok(article_id)
    }

    pub async fn update(
        pool: &PgPool,
        article_id: i32,
        params: &UpdateArticleParams,
    ) -> Result<bool, AppError> {
        let updated = sqlx::query_scalar!(
            r#"UPDATE articulos
               SET titulo = COALESCE($2, titulo),
                   slug = COALESCE($3, slug),
                   contenido = COALESCE($4, contenido),
                   extracto = COALESCE($5, extracto),
                   categoria = COALESCE($6, categoria),
                   portada_url = COALESCE($7, portada_url),
                   embeds = COALESCE($8::jsonb, embeds),
                   descarga_publica = COALESCE($9, descarga_publica),
                   updated_at = NOW()
               WHERE id = $1
                 AND eliminado_en IS NULL
               RETURNING id AS "id!""#,
            article_id,
            params.titulo,
            params.slug,
            params.contenido,
            params.extracto,
            params.categoria,
            params.portada_url,
            params.embeds.clone(),
            params.descarga_publica,
        )
        .fetch_optional(pool)
        .await?;
        Ok(updated.is_some())
    }

    pub async fn soft_delete(pool: &PgPool, article_id: i32) -> Result<bool, AppError> {
        let deleted = sqlx::query!(
            r#"UPDATE articulos
               SET eliminado_en = NOW(),
                   updated_at = NOW()
               WHERE id = $1
                 AND eliminado_en IS NULL"#,
            article_id,
        )
        .execute(pool)
        .await?
        .rows_affected();

        Ok(deleted > 0)
    }

    pub async fn toggle_like(
        pool: &PgPool,
        user_id: i32,
        article_id: i32,
    ) -> Result<(bool, i32), AppError> {
        let mut tx = pool.begin().await?;

        let exists = sqlx::query_scalar!(
            r#"SELECT EXISTS(
                    SELECT 1
                    FROM articulos_likes
                    WHERE usuario_id = $1 AND articulo_id = $2
               ) AS "exists!""#,
            user_id,
            article_id,
        )
        .fetch_one(&mut *tx)
        .await?;

        let liked = if exists {
            sqlx::query!(
                r#"DELETE FROM articulos_likes
                   WHERE usuario_id = $1 AND articulo_id = $2"#,
                user_id,
                article_id,
            )
            .execute(&mut *tx)
            .await?;
            false
        } else {
            sqlx::query!(
                r#"INSERT INTO articulos_likes (usuario_id, articulo_id)
                   VALUES ($1, $2)
                   ON CONFLICT (usuario_id, articulo_id) DO NOTHING"#,
                user_id,
                article_id,
            )
            .execute(&mut *tx)
            .await?;
            true
        };

        let total = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!"
               FROM articulos_likes
               WHERE articulo_id = $1"#,
            article_id,
        )
        .fetch_one(&mut *tx)
        .await?;
        let total_i32 = i32::try_from(total).unwrap_or(i32::MAX);

        sqlx::query!(
            r#"UPDATE articulos
               SET total_likes = $2,
                   updated_at = NOW()
               WHERE id = $1"#,
            article_id,
            total_i32,
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok((liked, total_i32))
    }
}
