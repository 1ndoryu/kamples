use chrono::{DateTime, Utc};
use sqlx::PgPool;

pub struct SeoRepository;

pub struct SitemapTimedUrl {
    pub slug: String,
    pub updated_at: Option<DateTime<Utc>>,
}

pub struct SitemapPlainUrl {
    pub slug: String,
}

pub struct SitemapProfileUrl {
    pub username: String,
    pub updated_at: Option<DateTime<Utc>>,
}

pub struct SitemapBlogUrl {
    pub slug: String,
    pub updated_at: DateTime<Utc>,
}

pub struct SeoSampleRecord {
    pub titulo: String,
    pub descripcion: Option<String>,
}

pub struct SeoProfileRecord {
    pub nombre_visible: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
}

pub struct SeoArticleRecord {
    pub titulo: String,
    pub extracto: String,
}

impl SeoRepository {
    pub async fn sitemap_samples(
        pool: &PgPool,
        limit: i64,
    ) -> Result<Vec<SitemapTimedUrl>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT slug, updated_at FROM samples
           WHERE estado = 'activo' AND slug IS NOT NULL
           ORDER BY updated_at DESC NULLS LAST LIMIT $1"#,
            limit
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| SitemapTimedUrl {
                slug: row.slug,
                updated_at: row.updated_at,
            })
            .collect())
    }

    pub async fn sitemap_songs(
        pool: &PgPool,
        limit: i64,
    ) -> Result<Vec<SitemapPlainUrl>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT slug FROM canciones ORDER BY id DESC LIMIT $1"#,
            limit
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| SitemapPlainUrl { slug: row.slug })
            .collect())
    }

    pub async fn sitemap_artists(
        pool: &PgPool,
        limit: i64,
    ) -> Result<Vec<SitemapPlainUrl>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT slug FROM artistas_musicales ORDER BY id DESC LIMIT $1"#,
            limit
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| SitemapPlainUrl { slug: row.slug })
            .collect())
    }

    pub async fn sitemap_profiles(
        pool: &PgPool,
        limit: i64,
    ) -> Result<Vec<SitemapProfileUrl>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT username, updated_at FROM usuarios_ext
           WHERE estado = 'activo'
           ORDER BY updated_at DESC NULLS LAST LIMIT $1"#,
            limit
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| SitemapProfileUrl {
                username: row.username,
                updated_at: row.updated_at,
            })
            .collect())
    }

    pub async fn sitemap_blog(
        pool: &PgPool,
        limit: i64,
    ) -> Result<Vec<SitemapBlogUrl>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT slug, updated_at FROM articulos
           WHERE moderacion_estado = 'aprobado' AND publicado_en IS NOT NULL
             AND eliminado_en IS NULL
           ORDER BY publicado_en DESC LIMIT $1"#,
            limit
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| SitemapBlogUrl {
                slug: row.slug,
                updated_at: row.updated_at,
            })
            .collect())
    }

    pub async fn sample_metadata(
        pool: &PgPool,
        slug: &str,
    ) -> Result<Option<SeoSampleRecord>, sqlx::Error> {
        let row = sqlx::query!(
            r#"SELECT titulo, descripcion, slug
               FROM samples WHERE slug = $1 AND estado = 'activo' LIMIT 1"#,
            slug
        )
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|row| SeoSampleRecord {
            titulo: row.titulo,
            descripcion: row.descripcion,
        }))
    }

    pub async fn profile_metadata(
        pool: &PgPool,
        username: &str,
    ) -> Result<Option<SeoProfileRecord>, sqlx::Error> {
        let row = sqlx::query!(
            r#"SELECT username, nombre_visible, bio, avatar_url
               FROM usuarios_ext WHERE username = $1 AND estado = 'activo' LIMIT 1"#,
            username
        )
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|row| SeoProfileRecord {
            nombre_visible: row.nombre_visible,
            bio: row.bio,
            avatar_url: row.avatar_url,
        }))
    }

    pub async fn article_metadata(
        pool: &PgPool,
        slug: &str,
    ) -> Result<Option<SeoArticleRecord>, sqlx::Error> {
        let row = sqlx::query!(
            r#"SELECT titulo, extracto, slug
               FROM articulos
               WHERE slug = $1 AND moderacion_estado = 'aprobado'
                 AND publicado_en IS NOT NULL AND eliminado_en IS NULL
               LIMIT 1"#,
            slug
        )
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|row| SeoArticleRecord {
            titulo: row.titulo,
            extracto: row.extracto,
        }))
    }
}
