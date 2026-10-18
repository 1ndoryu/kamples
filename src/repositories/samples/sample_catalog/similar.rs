use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};

use super::query::{push_guardado_en_coleccion_select, push_reaccion_select, SampleSummaryRow, SAMPLE_SUMMARY_SELECT};
use super::{SampleCatalogSummaryRecord, SampleRepository};
use crate::repositories::AUTO_HIDE_SAMPLE_REPORT_THRESHOLD;

const MAX_SIMILARITY_TAGS: usize = 10;
const SIMILAR_CONTENT_WEIGHT: f64 = 0.55;
const SIMILAR_CONTEXT_WEIGHT: f64 = 0.10;
const SIMILAR_TRENDING_WEIGHT: f64 = 0.20;
const SIMILAR_NOVELTY_WEIGHT: f64 = 0.15;
const SIMILAR_CONTEXT_BPM_WEIGHT: f64 = 0.25;
const SIMILAR_CONTEXT_KEY_WEIGHT: f64 = 0.25;
const SIMILAR_CONTEXT_TYPE_WEIGHT: f64 = 0.50;
const SIMILAR_BPM_TOLERANCE: i32 = 15;
const SIMILAR_NOVELTY_DAYS_BOOST: i32 = 14;

#[derive(Debug, FromRow)]
struct SimilaritySourceRow {
    id: i32,
    tags: Vec<String>,
    bpm: Option<i32>,
    music_key: Option<String>,
    sample_type: String,
    has_embedding: bool,
}

impl SampleRepository {
    pub async fn find_public_similar_samples(
        pool: &PgPool,
        sample_id: i32,
        limit: i64,
        viewer_id: Option<i32>,
    ) -> Result<Option<Vec<SampleCatalogSummaryRecord>>, sqlx::Error> {
        let Some(source) = Self::find_public_similarity_source(pool, sample_id, viewer_id).await?
        else {
            return Ok(None);
        };

        if source.has_embedding {
            let vector_matches =
                Self::find_public_similar_samples_by_embedding(pool, sample_id, limit, viewer_id)
                    .await?;

            if !vector_matches.is_empty() {
                return Ok(Some(vector_matches));
            }
        }

        let fallback_matches =
            Self::find_public_similar_samples_by_fallback(pool, &source, limit, viewer_id).await?;

        Ok(Some(fallback_matches))
    }

    async fn find_public_similarity_source(
        pool: &PgPool,
        sample_id: i32,
        viewer_id: Option<i32>,
    ) -> Result<Option<SimilaritySourceRow>, sqlx::Error> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT
                s.id,
                COALESCE(s.tags, ARRAY[]::text[]) AS tags,
                s.bpm,
                s.key AS music_key,
                s.tipo AS sample_type,
                (s.embedding IS NOT NULL) AS has_embedding
             FROM samples s
             WHERE s.id = ",
        );
        builder.push_bind(sample_id);
        builder.push(
            "
               AND s.eliminado_en IS NULL
               AND s.estado = 'activo'",
        );
        push_auto_hide_filter(&mut builder, "s.id", "s.creador_id", viewer_id);
        builder.push(
            "
             LIMIT 1",
        );

        builder
            .build_query_as::<SimilaritySourceRow>()
            .fetch_optional(pool)
            .await
    }

    async fn find_public_similar_samples_by_embedding(
        pool: &PgPool,
        sample_id: i32,
        limit: i64,
        viewer_id: Option<i32>,
    ) -> Result<Vec<SampleCatalogSummaryRecord>, sqlx::Error> {
        let mut builder = QueryBuilder::<Postgres>::new(SAMPLE_SUMMARY_SELECT);
        push_guardado_en_coleccion_select(&mut builder, viewer_id);
        push_reaccion_select(&mut builder, viewer_id);
        builder.push(
            " FROM samples s
              INNER JOIN usuarios_ext u ON u.id = s.creador_id
              WHERE s.eliminado_en IS NULL
                AND s.estado = 'activo'
                AND s.id != ",
        );
        builder.push_bind(sample_id);
        push_auto_hide_filter(&mut builder, "s.id", "s.creador_id", viewer_id);
        builder.push(
            "
                AND s.embedding IS NOT NULL
              ORDER BY s.embedding <=> (SELECT embedding FROM samples WHERE id = ",
        );
        builder.push_bind(sample_id);
        builder.push(
            "),
                     s.publicado_at DESC NULLS LAST,
                     s.created_at DESC,
                     s.id DESC
              LIMIT ",
        );
        builder.push_bind(limit);

        let rows = builder
            .build_query_as::<SampleSummaryRow>()
            .fetch_all(pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(SampleCatalogSummaryRecord::from)
            .collect())
    }

    async fn find_public_similar_samples_by_fallback(
        pool: &PgPool,
        source: &SimilaritySourceRow,
        limit: i64,
        viewer_id: Option<i32>,
    ) -> Result<Vec<SampleCatalogSummaryRecord>, sqlx::Error> {
        let mut builder = QueryBuilder::<Postgres>::new(SAMPLE_SUMMARY_SELECT);
        push_guardado_en_coleccion_select(&mut builder, viewer_id);
        push_reaccion_select(&mut builder, viewer_id);
        builder.push(
            " FROM samples s
              INNER JOIN usuarios_ext u ON u.id = s.creador_id
              WHERE s.eliminado_en IS NULL
                AND s.estado = 'activo'
                AND s.id != ",
        );
        builder.push_bind(source.id);
        push_auto_hide_filter(&mut builder, "s.id", "s.creador_id", viewer_id);

        builder.push(" ORDER BY (");
        builder.push(format!("{SIMILAR_CONTENT_WEIGHT} * "));
        push_similarity_tag_score(&mut builder, source);
        builder.push(" + ");
        builder.push(format!("{SIMILAR_CONTEXT_WEIGHT} * ("));
        builder.push(format!("{SIMILAR_CONTEXT_BPM_WEIGHT} * "));
        push_similarity_bpm_score(&mut builder, source);
        builder.push(" + ");
        builder.push(format!("{SIMILAR_CONTEXT_KEY_WEIGHT} * "));
        push_similarity_key_score(&mut builder, source);
        builder.push(" + ");
        builder.push(format!("{SIMILAR_CONTEXT_TYPE_WEIGHT} * "));
        push_similarity_type_score(&mut builder, source);
        builder.push(") + ");
        builder.push(format!("{SIMILAR_TRENDING_WEIGHT} * "));
        builder.push(similarity_trending_score_sql());
        builder.push(" + ");
        builder.push(format!("{SIMILAR_NOVELTY_WEIGHT} * "));
        builder.push(similarity_novelty_score_sql());
        builder.push(
            ") DESC,
            s.publicado_at DESC NULLS LAST,
            s.created_at DESC,
            s.id DESC",
        );
        builder.push(" LIMIT ");
        builder.push_bind(limit);

        let rows = builder
            .build_query_as::<SampleSummaryRow>()
            .fetch_all(pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(SampleCatalogSummaryRecord::from)
            .collect())
    }
}

fn push_auto_hide_filter(
    builder: &mut QueryBuilder<'_, Postgres>,
    target_expr: &str,
    creator_expr: &str,
    viewer_id: Option<i32>,
) {
    builder.push(" AND ((SELECT COUNT(*) FROM reportes r WHERE r.tipo = 'sample' AND COALESCE(r.estado, 'pendiente') = 'pendiente' AND r.target_id = ");
    builder.push(target_expr);
    builder.push(") < ");
    builder.push(AUTO_HIDE_SAMPLE_REPORT_THRESHOLD.to_string());
    if let Some(viewer_id) = viewer_id {
        builder.push(" OR ");
        builder.push(creator_expr);
        builder.push(" = ");
        builder.push_bind(viewer_id);
    }
    builder.push(")");
}

fn push_similarity_tag_score(
    builder: &mut QueryBuilder<'_, Postgres>,
    source: &SimilaritySourceRow,
) {
    let tags: Vec<String> = source
        .tags
        .iter()
        .take(MAX_SIMILARITY_TAGS)
        .cloned()
        .collect();
    if tags.is_empty() {
        builder.push("0.0");
        return;
    }

    let tag_count = tags.len();
    builder.push("((");
    let mut separated = builder.separated(" + ");
    for tag in &tags {
        separated.push("CASE WHEN ");
        separated.push_bind(tag.clone());
        separated.push(" = ANY(COALESCE(s.tags, ARRAY[]::text[])) THEN 1 ELSE 0 END");
    }
    builder.push(")::double precision / ");
    builder.push(tag_count.to_string());
    builder.push(")");
}

fn push_similarity_bpm_score(
    builder: &mut QueryBuilder<'_, Postgres>,
    source: &SimilaritySourceRow,
) {
    if let Some(bpm) = source.bpm {
        builder.push("GREATEST(0.0, (");
        builder.push(SIMILAR_BPM_TOLERANCE.to_string());
        builder.push(" - ABS(COALESCE(s.bpm, 0) - ");
        builder.push_bind(bpm);
        builder.push(") )::double precision / ");
        builder.push(SIMILAR_BPM_TOLERANCE.to_string());
        builder.push(")");
    } else {
        builder.push("0.5");
    }
}

fn push_similarity_key_score(
    builder: &mut QueryBuilder<'_, Postgres>,
    source: &SimilaritySourceRow,
) {
    if let Some(music_key) = &source.music_key {
        builder.push("CASE WHEN s.key = ");
        builder.push_bind(music_key.clone());
        builder.push(" THEN 1.0 ELSE 0.0 END");
    } else {
        builder.push("0.5");
    }
}

fn push_similarity_type_score(
    builder: &mut QueryBuilder<'_, Postgres>,
    source: &SimilaritySourceRow,
) {
    builder.push("CASE WHEN s.tipo = ");
    builder.push_bind(source.sample_type.clone());
    builder.push(" THEN 1.0 ELSE 0.0 END");
}

fn similarity_trending_score_sql() -> &'static str {
    "LEAST(
        1.0,
        (
            COALESCE(s.total_likes, 0) * 2
            + COALESCE(s.total_reproducciones, 0)
            + COALESCE(s.total_descargas, 0) * 3
        )::double precision / GREATEST(
            1.0,
            COALESCE(
                (
                    SELECT AVG(
                        COALESCE(base.total_likes, 0) * 2
                        + COALESCE(base.total_reproducciones, 0)
                        + COALESCE(base.total_descargas, 0) * 3
                    )::double precision
                    FROM samples base
                    WHERE base.eliminado_en IS NULL
                      AND base.estado = 'activo'
                ),
                1.0
            )
        )
    )"
}

fn similarity_novelty_score_sql() -> String {
    format!(
        "GREATEST(
            0.0,
            1.0 - LN(
                GREATEST(
                    1.0,
                    EXTRACT(EPOCH FROM NOW() - COALESCE(s.publicado_at, s.created_at, NOW())) / 86400.0
                )
            ) / LN({SIMILAR_NOVELTY_DAYS_BOOST}.0)
        )",
    )
}
