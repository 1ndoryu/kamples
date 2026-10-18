use serde::Serialize;
use utoipa::ToSchema;

use super::SampleRepository;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TagAggregateItem {
    pub tag: String,
    pub conteo: i64,
}

#[derive(Debug, Clone, Default)]
pub struct TagAggregateFilters {
    pub genero: Option<String>,
    pub bpm_min: Option<i32>,
    pub bpm_max: Option<i32>,
    pub music_key: Option<String>,
    pub sample_type: Option<String>,
    pub limit: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TagAggregatesResult {
    pub genero: Vec<TagAggregateItem>,
    pub instrumento: Vec<TagAggregateItem>,
    pub sentimiento: Vec<TagAggregateItem>,
    pub tipo: Vec<TagAggregateItem>,
    pub otro: Vec<TagAggregateItem>,
}

impl SampleRepository {
    pub async fn aggregate_public_tags(
        pool: &sqlx::PgPool,
        filters: &TagAggregateFilters,
    ) -> Result<TagAggregatesResult, sqlx::Error> {
        let genero = fetch_genero(pool, filters).await?;
        let instrumento = fetch_instrumento(pool, filters).await?;
        let sentimiento = fetch_sentimiento(pool, filters).await?;
        let tipo = fetch_tipo(pool, filters).await?;
        let otro = fetch_otro(pool, filters).await?;

        Ok(TagAggregatesResult {
            genero,
            instrumento,
            sentimiento,
            tipo,
            otro,
        })
    }
}

async fn fetch_genero(
    pool: &sqlx::PgPool,
    filters: &TagAggregateFilters,
) -> Result<Vec<TagAggregateItem>, sqlx::Error> {
    sqlx::query_as!(
        TagAggregateItem,
        r#"SELECT g.val AS "tag!", COUNT(*)::bigint AS "conteo!"
           FROM samples s
           CROSS JOIN LATERAL jsonb_array_elements_text(
                CASE WHEN jsonb_typeof(s.metadata->'genero') = 'array'
                    THEN s.metadata->'genero'
                    ELSE '[]'::jsonb
                END
           ) AS g(val)
           WHERE s.eliminado_en IS NULL
             AND s.estado = 'activo'
             AND ($1::text IS NULL OR (s.metadata->'genero') ? $1)
             AND ($2::int IS NULL OR s.bpm >= $2)
             AND ($3::int IS NULL OR s.bpm <= $3)
             AND ($4::text IS NULL OR s.key = $4)
             AND ($5::text IS NULL OR s.tipo = $5)
             AND btrim(g.val) <> ''
           GROUP BY g.val
           ORDER BY COUNT(*) DESC, g.val ASC
           LIMIT $6"#,
        filters.genero,
        filters.bpm_min,
        filters.bpm_max,
        filters.music_key,
        filters.sample_type,
        filters.limit,
    )
    .fetch_all(pool)
    .await
}

async fn fetch_instrumento(
    pool: &sqlx::PgPool,
    filters: &TagAggregateFilters,
) -> Result<Vec<TagAggregateItem>, sqlx::Error> {
    sqlx::query_as!(
        TagAggregateItem,
        r#"SELECT i.val AS "tag!", COUNT(*)::bigint AS "conteo!"
           FROM samples s
           CROSS JOIN LATERAL jsonb_array_elements_text(
                CASE WHEN jsonb_typeof(s.metadata->'instrumentos') = 'array'
                    THEN s.metadata->'instrumentos'
                    ELSE '[]'::jsonb
                END
           ) AS i(val)
           WHERE s.eliminado_en IS NULL
             AND s.estado = 'activo'
             AND ($1::text IS NULL OR (s.metadata->'genero') ? $1)
             AND ($2::int IS NULL OR s.bpm >= $2)
             AND ($3::int IS NULL OR s.bpm <= $3)
             AND ($4::text IS NULL OR s.key = $4)
             AND ($5::text IS NULL OR s.tipo = $5)
             AND btrim(i.val) <> ''
           GROUP BY i.val
           ORDER BY COUNT(*) DESC, i.val ASC
           LIMIT $6"#,
        filters.genero,
        filters.bpm_min,
        filters.bpm_max,
        filters.music_key,
        filters.sample_type,
        filters.limit,
    )
    .fetch_all(pool)
    .await
}

async fn fetch_sentimiento(
    pool: &sqlx::PgPool,
    filters: &TagAggregateFilters,
) -> Result<Vec<TagAggregateItem>, sqlx::Error> {
    sqlx::query_as!(
        TagAggregateItem,
        r#"SELECT s.metadata->>'emocion' AS "tag!", COUNT(*)::bigint AS "conteo!"
           FROM samples s
           WHERE s.eliminado_en IS NULL
             AND s.estado = 'activo'
             AND ($1::text IS NULL OR (s.metadata->'genero') ? $1)
             AND ($2::int IS NULL OR s.bpm >= $2)
             AND ($3::int IS NULL OR s.bpm <= $3)
             AND ($4::text IS NULL OR s.key = $4)
             AND ($5::text IS NULL OR s.tipo = $5)
             AND COALESCE(btrim(s.metadata->>'emocion'), '') <> ''
           GROUP BY s.metadata->>'emocion'
           ORDER BY COUNT(*) DESC, s.metadata->>'emocion' ASC
           LIMIT $6"#,
        filters.genero,
        filters.bpm_min,
        filters.bpm_max,
        filters.music_key,
        filters.sample_type,
        filters.limit,
    )
    .fetch_all(pool)
    .await
}

async fn fetch_tipo(
    pool: &sqlx::PgPool,
    filters: &TagAggregateFilters,
) -> Result<Vec<TagAggregateItem>, sqlx::Error> {
    sqlx::query_as!(
        TagAggregateItem,
        r#"SELECT s.tipo AS "tag!", COUNT(*)::bigint AS "conteo!"
           FROM samples s
           WHERE s.eliminado_en IS NULL
             AND s.estado = 'activo'
             AND ($1::text IS NULL OR (s.metadata->'genero') ? $1)
             AND ($2::int IS NULL OR s.bpm >= $2)
             AND ($3::int IS NULL OR s.bpm <= $3)
             AND ($4::text IS NULL OR s.key = $4)
             AND ($5::text IS NULL OR s.tipo = $5)
           GROUP BY s.tipo
           ORDER BY COUNT(*) DESC, s.tipo ASC
           LIMIT $6"#,
        filters.genero,
        filters.bpm_min,
        filters.bpm_max,
        filters.music_key,
        filters.sample_type,
        filters.limit,
    )
    .fetch_all(pool)
    .await
}

async fn fetch_otro(
    pool: &sqlx::PgPool,
    filters: &TagAggregateFilters,
) -> Result<Vec<TagAggregateItem>, sqlx::Error> {
    sqlx::query_as!(
        TagAggregateItem,
        r#"SELECT t.val AS "tag!", COUNT(*)::bigint AS "conteo!"
           FROM samples s
           CROSS JOIN LATERAL jsonb_array_elements_text(
                COALESCE(
                    CASE WHEN jsonb_typeof(s.metadata->'tags') = 'array' THEN s.metadata->'tags' END,
                    '[]'::jsonb
                )
                ||
                COALESCE(
                    CASE WHEN jsonb_typeof(s.metadata->'artista_vibes') = 'array' THEN s.metadata->'artista_vibes' END,
                    '[]'::jsonb
                )
           ) AS t(val)
           WHERE s.eliminado_en IS NULL
             AND s.estado = 'activo'
             AND ($1::text IS NULL OR (s.metadata->'genero') ? $1)
             AND ($2::int IS NULL OR s.bpm >= $2)
             AND ($3::int IS NULL OR s.bpm <= $3)
             AND ($4::text IS NULL OR s.key = $4)
             AND ($5::text IS NULL OR s.tipo = $5)
             AND btrim(t.val) <> ''
           GROUP BY t.val
           ORDER BY COUNT(*) DESC, t.val ASC
           LIMIT $6"#,
        filters.genero,
        filters.bpm_min,
        filters.bpm_max,
        filters.music_key,
        filters.sample_type,
        filters.limit,
    )
    .fetch_all(pool)
    .await
}
