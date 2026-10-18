/* [174A-52] Motor "más como esto": similares por embedding (pgvector) o, si
 * no hay embedding, por overlap de tags y tipo. */

use std::collections::HashSet;

use pgvector::Vector;
use sqlx::PgPool;

use crate::algorithm::signals::content_similarity_score;
use crate::errors::AppError;

use super::contratos::{RankedSample, SampleRow};
use super::puntuacion::row_to_ranked;
use super::service::RecommenderService;

impl RecommenderService {
    /// "Más como esto": top N samples similares a uno dado, vía pgvector.
    /// Si el sample no tiene embedding o pgvector está deshabilitado, cae a
    /// scoring por overlap de tags + match de tipo (más simple que el legado).
    pub async fn similar_to_sample(
        pool: &PgPool,
        sample_id: i32,
        limit: i64,
        viewer_id: Option<i32>,
    ) -> Result<Vec<RankedSample>, AppError> {
        let blocked: Vec<i32> = if let Some(uid) = viewer_id {
            crate::repositories::ModerationRepository::list_blocked(pool, uid).await?
        } else {
            Vec::new()
        };

        let seed = sqlx::query!(
            r#"SELECT embedding AS "embedding?: Vector", tipo AS "tipo!", tags AS "tags!" FROM samples WHERE id = $1"#,
            sample_id
        )
        .fetch_optional(pool)
        .await?;

        let Some(seed) = seed else {
            return Ok(Vec::new());
        };

        if let Some(vector) = seed.embedding.as_ref() {
            let rows = fetch_similar_by_embedding(pool, sample_id, vector, &blocked, limit).await?;
            if !rows.is_empty() {
                return Ok(rows
                    .into_iter()
                    .map(|row| {
                        let dist = row.embedding_distance.unwrap_or(2.0);
                        let score = content_similarity_score(Some(dist));
                        row_to_ranked(row, score, true)
                    })
                    .collect());
            }
        }

        let rows =
            fetch_similar_by_tags(pool, sample_id, &seed.tags, &seed.tipo, &blocked, limit).await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let overlap = jaccard_overlap(&row.tags, &seed.tags);
                let tipo_match = if row.tipo == seed.tipo { 0.4 } else { 0.0 };
                row_to_ranked(row, overlap * 0.6 + tipo_match, true)
            })
            .collect())
    }
}

async fn fetch_similar_by_embedding(
    pool: &PgPool,
    sample_id: i32,
    vector: &Vector,
    blocked: &[i32],
    limit: i64,
) -> Result<Vec<SampleRow>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT
            s.id            AS "id!",
            s.creador_id    AS "creador_id!",
            s.titulo        AS "titulo!",
            s.slug          AS "slug!",
            s.tipo          AS "tipo!",
            s.bpm           AS "bpm?",
            s.key           AS "key?",
            s.escala        AS "escala?",
            s.tags          AS "tags!",
            s.publicado_at  AS "publicado_at?",
            s.total_likes,
            s.total_reproducciones,
            s.total_descargas,
            s.verificado,
            (s.embedding <=> $1)::float8 AS "embedding_distance?"
        FROM samples s
        WHERE s.estado = 'activo'
          AND s.id <> $2
          AND s.embedding IS NOT NULL
          AND s.creador_id <> ALL($3::int[])
        ORDER BY s.embedding <=> $1
        LIMIT $4
        "#,
        vector as &Vector,
        sample_id,
        blocked,
        limit,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| SampleRow {
            id: r.id,
            creador_id: r.creador_id,
            titulo: r.titulo,
            slug: r.slug,
            tipo: r.tipo,
            bpm: r.bpm,
            key: r.key,
            escala: r.escala,
            tags: r.tags,
            publicado_at: r.publicado_at,
            total_likes: r.total_likes.unwrap_or(0),
            total_reproducciones: r.total_reproducciones.unwrap_or(0),
            total_descargas: r.total_descargas.unwrap_or(0),
            verificado: r.verificado.unwrap_or(false),
            embedding_distance: r.embedding_distance,
            likes_24h: 0.0,
            reproducciones_24h: 0.0,
            descargas_7d: 0.0,
            follows_creador_7d: 0.0,
            creador_seguido: false,
            likes_dados: 0.0,
            reproducciones: 0.0,
            tiempo_escucha: 0.0,
            descargas_propias: 0.0,
            completadas: 0.0,
            dislike_penalty: 0.0,
            nunca_reproducido: true,
        })
        .collect())
}

async fn fetch_similar_by_tags(
    pool: &PgPool,
    sample_id: i32,
    tags: &[String],
    tipo: &str,
    blocked: &[i32],
    limit: i64,
) -> Result<Vec<SampleRow>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT
            s.id            AS "id!",
            s.creador_id    AS "creador_id!",
            s.titulo        AS "titulo!",
            s.slug          AS "slug!",
            s.tipo          AS "tipo!",
            s.bpm           AS "bpm?",
            s.key           AS "key?",
            s.escala        AS "escala?",
            s.tags          AS "tags!",
            s.publicado_at  AS "publicado_at?",
            s.total_likes,
            s.total_reproducciones,
            s.total_descargas,
            s.verificado
        FROM samples s
        WHERE s.estado = 'activo'
          AND s.id <> $1
          AND (s.tags && $2::text[] OR s.tipo = $3)
          AND s.creador_id <> ALL($4::int[])
        ORDER BY s.publicado_at DESC NULLS LAST
        LIMIT $5
        "#,
        sample_id,
        tags,
        tipo,
        blocked,
        limit,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| SampleRow {
            id: r.id,
            creador_id: r.creador_id,
            titulo: r.titulo,
            slug: r.slug,
            tipo: r.tipo,
            bpm: r.bpm,
            key: r.key,
            escala: r.escala,
            tags: r.tags,
            publicado_at: r.publicado_at,
            total_likes: r.total_likes.unwrap_or(0),
            total_reproducciones: r.total_reproducciones.unwrap_or(0),
            total_descargas: r.total_descargas.unwrap_or(0),
            verificado: r.verificado.unwrap_or(false),
            embedding_distance: None,
            likes_24h: 0.0,
            reproducciones_24h: 0.0,
            descargas_7d: 0.0,
            follows_creador_7d: 0.0,
            creador_seguido: false,
            likes_dados: 0.0,
            reproducciones: 0.0,
            tiempo_escucha: 0.0,
            descargas_propias: 0.0,
            completadas: 0.0,
            dislike_penalty: 0.0,
            nunca_reproducido: true,
        })
        .collect())
}

pub(super) fn jaccard_overlap(a: &[String], b: &[String]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 0.0;
    }
    let set_a: HashSet<String> = a.iter().map(|t| t.to_lowercase()).collect();
    let set_b: HashSet<String> = b.iter().map(|t| t.to_lowercase()).collect();
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}
