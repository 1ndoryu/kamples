/* [174A-52] Fallback del MotorRecomendacion: top reciente por novedad para
 * cold start o cuando el scoring no devuelve filas. */

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::algorithm::signals::{novelty_signal_score, AlgorithmSignalConfig};
use crate::errors::AppError;

use super::consultas::collect_blocked;
use super::contratos::RankedSample;

/// Fila de los SELECT de fallback (la forma de la tupla la fija el `.map`).
type FilaReciente = (
    i32,
    i32,
    String,
    String,
    String,
    Option<i32>,
    Option<String>,
    Option<String>,
    Vec<String>,
    Option<DateTime<Utc>>,
    i32,
    i32,
    i32,
    bool,
);

pub(super) async fn fallback_recientes(
    pool: &PgPool,
    user_id: i32,
    limit: usize,
    candidates: Option<&[i32]>,
) -> Result<Vec<RankedSample>, AppError> {
    /* Cold start o scoring vacío: top trending del último mes. Usa la novedad
     * como score. Replica el espíritu de `feedNuevoUsuario` legacy con menos
     * complejidad (la diversidad la aplica `apply_diversity` posteriormente). */
    let blocked = collect_blocked(pool, user_id).await?;
    let limit_i64 = limit as i64;
    let rows = match candidates {
        Some(ids) => fallback_candidatos(pool, ids, &blocked, limit_i64).await?,
        None => fallback_global(pool, &blocked, limit_i64).await?,
    };
    let dias_boost = AlgorithmSignalConfig::legacy_current()
        .parametros
        .novedad_dias_boost;
    Ok(rows
        .into_iter()
        .map(|r| {
            let dias =
                r.9.map(|ts| ((Utc::now() - ts).num_seconds().max(0) as f64) / 86_400.0);
            let novelty = novelty_signal_score(dias, dias_boost);
            RankedSample {
                id: r.0,
                creador_id: r.1,
                titulo: r.2,
                slug: r.3,
                tipo: r.4,
                bpm: r.5,
                key: r.6,
                escala: r.7,
                tags: r.8,
                publicado_at: r.9,
                total_likes: r.10,
                total_reproducciones: r.11,
                total_descargas: r.12,
                verificado: r.13,
                es_nuevo: true,
                score: novelty + 0.5,
            }
        })
        .collect())
}

/// Candidatos de la página sin bloqueados, por interacciones acumuladas.
async fn fallback_candidatos(
    pool: &PgPool,
    ids: &[i32],
    blocked: &[i32],
    limit_i64: i64,
) -> Result<Vec<FilaReciente>, AppError> {
    let rows = sqlx::query!(
        r#"
            SELECT id AS "id!", creador_id AS "creador_id!", titulo AS "titulo!",
                   slug AS "slug!", tipo AS "tipo!", bpm AS "bpm?", key AS "key?",
                   escala AS "escala?", tags AS "tags!", publicado_at AS "publicado_at?",
                   total_likes, total_reproducciones, total_descargas, verificado
              FROM samples
             WHERE estado = 'activo'
               AND id = ANY($1::int[])
               AND creador_id <> ALL($2::int[])
             ORDER BY (total_likes * 2 + total_reproducciones + total_descargas * 3) DESC
             LIMIT $3
            "#,
        ids,
        blocked,
        limit_i64,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| {
        (
            r.id,
            r.creador_id,
            r.titulo,
            r.slug,
            r.tipo,
            r.bpm,
            r.key,
            r.escala,
            r.tags,
            r.publicado_at,
            r.total_likes.unwrap_or(0),
            r.total_reproducciones.unwrap_or(0),
            r.total_descargas.unwrap_or(0),
            r.verificado.unwrap_or(false),
        )
    })
    .collect::<Vec<_>>();
    Ok(rows)
}

/// Sin candidatos: top reciente global sin bloqueados.
async fn fallback_global(
    pool: &PgPool,
    blocked: &[i32],
    limit_i64: i64,
) -> Result<Vec<FilaReciente>, AppError> {
    let rows = sqlx::query!(
        r#"
            SELECT id AS "id!", creador_id AS "creador_id!", titulo AS "titulo!",
                   slug AS "slug!", tipo AS "tipo!", bpm AS "bpm?", key AS "key?",
                   escala AS "escala?", tags AS "tags!", publicado_at AS "publicado_at?",
                   total_likes, total_reproducciones, total_descargas, verificado
              FROM samples
             WHERE estado = 'activo'
               AND creador_id <> ALL($1::int[])
             ORDER BY (total_likes * 2 + total_reproducciones + total_descargas * 3) DESC
             LIMIT $2
            "#,
        blocked,
        limit_i64,
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| {
        (
            r.id,
            r.creador_id,
            r.titulo,
            r.slug,
            r.tipo,
            r.bpm,
            r.key,
            r.escala,
            r.tags,
            r.publicado_at,
            r.total_likes.unwrap_or(0),
            r.total_reproducciones.unwrap_or(0),
            r.total_descargas.unwrap_or(0),
            r.verificado.unwrap_or(false),
        )
    })
    .collect::<Vec<_>>();
    Ok(rows)
}
