/* [174A-52] Consultas del MotorRecomendacion: bloqueos y enriquecimiento de
 * samples. Cada rama devuelve filas listas para el scoring. */

use sqlx::PgPool;

use crate::errors::AppError;

use super::contratos::SampleRow;

pub(super) async fn collect_blocked(pool: &PgPool, user_id: i32) -> Result<Vec<i32>, AppError> {
    let mut blocked =
        crate::repositories::ModerationRepository::list_blocked(pool, user_id).await?;
    let blockers = crate::repositories::ModerationRepository::list_blockers(pool, user_id).await?;
    blocked.extend(blockers);
    blocked.sort_unstable();
    blocked.dedup();
    Ok(blocked)
}

pub(super) async fn fetch_enriched(
    pool: &PgPool,
    user_id: i32,
    candidates: Option<&[i32]>,
    limit: usize,
) -> Result<Vec<SampleRow>, AppError> {
    /* Una sola query agregada que devuelve todo lo que el scoring necesita.
     * Filtra por candidatos si están definidos, o por `estado = 'activo'`
     * cruzando bloqueos bidireccionales. Las métricas de comportamiento se
     * agregan con LEFT JOIN LATERAL para no inflar la cardinalidad. */
    let limit_i64 = limit as i64;
    let blocked = collect_blocked(pool, user_id).await?;
    match candidates {
        Some(ids) => fetch_enriched_candidatos(pool, user_id, ids, &blocked, limit_i64).await,
        None => fetch_enriched_global(pool, user_id, &blocked, limit_i64).await,
    }
}

/// Rama con candidatos: solo los ids pedidos, sin bloqueados.
async fn fetch_enriched_candidatos(
    pool: &PgPool,
    user_id: i32,
    ids: &[i32],
    blocked: &[i32],
    limit_i64: i64,
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
                NULL::float8    AS "embedding_distance?",
                COALESCE(mv.likes_24h, 0)::float8          AS "likes_24h!",
                COALESCE(mv.reproducciones_24h, 0)::float8 AS "reproducciones_24h!",
                COALESCE(mv.descargas_7d, 0)::float8       AS "descargas_7d!",
                COALESCE(mv.follows_creador_7d, 0)::float8 AS "follows_creador_7d!",
                EXISTS (SELECT 1 FROM follows f WHERE f.seguidor_id = $1 AND f.seguido_id = s.creador_id) AS "creador_seguido!",
                COALESCE(b.likes_dados, 0)::float8         AS "likes_dados!",
                COALESCE(b.reproducciones, 0)::float8      AS "reproducciones!",
                COALESCE(b.tiempo_escucha, 0)::float8      AS "tiempo_escucha!",
                COALESCE(b.descargas_propias, 0)::float8   AS "descargas_propias!",
                COALESCE(b.completadas, 0)::float8         AS "completadas!",
                COALESCE(b.dislike_penalty, 0)::float8     AS "dislike_penalty!",
                NOT EXISTS (SELECT 1 FROM reproducciones r WHERE r.usuario_id = $1 AND r.sample_id = s.id) AS "nunca_reproducido!"
            FROM samples s
            LEFT JOIN mv_trending_samples mv ON mv.sample_id = s.id
            LEFT JOIN LATERAL (
                SELECT
                    COUNT(DISTINCT lk.target_id) FILTER (WHERE lk.target_id = s.id) AS likes_dados,
                    COUNT(*) FILTER (WHERE rp.sample_id = s.id) AS reproducciones,
                    COALESCE(SUM(rp.duracion_escuchada) FILTER (WHERE rp.sample_id = s.id), 0)::float8 AS tiempo_escucha,
                    COUNT(*) FILTER (WHERE dl.sample_id = s.id) AS descargas_propias,
                    COUNT(*) FILTER (WHERE rp.sample_id = s.id AND rp.completada) AS completadas,
                    COUNT(*) FILTER (WHERE lk.target_id = s.id AND lk.reaccion = 'dislike') AS dislike_penalty
                FROM (SELECT 1) _
                LEFT JOIN likes lk          ON lk.usuario_id = $1 AND lk.tipo = 'sample'
                LEFT JOIN reproducciones rp ON rp.usuario_id = $1
                LEFT JOIN descargas dl      ON dl.usuario_id = $1
            ) b ON TRUE
            WHERE s.estado = 'activo'
              AND s.id = ANY($2::int[])
              AND s.creador_id <> ALL($3::int[])
            ORDER BY s.publicado_at DESC NULLS LAST
            LIMIT $4
            "#,
            user_id,
            ids,
            blocked,
            limit_i64,
        )
        .fetch_all(pool)
        .await?;

    return Ok(rows
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
            likes_24h: r.likes_24h,
            reproducciones_24h: r.reproducciones_24h,
            descargas_7d: r.descargas_7d,
            follows_creador_7d: r.follows_creador_7d,
            creador_seguido: r.creador_seguido,
            likes_dados: r.likes_dados,
            reproducciones: r.reproducciones,
            tiempo_escucha: r.tiempo_escucha,
            descargas_propias: r.descargas_propias,
            completadas: r.completadas,
            dislike_penalty: r.dislike_penalty,
            nunca_reproducido: r.nunca_reproducido,
        })
        .collect());
}

/// Rama global: scan con JOIN a la MV de tendencias, sin bloqueados.
async fn fetch_enriched_global(
    pool: &PgPool,
    user_id: i32,
    blocked: &[i32],
    limit_i64: i64,
) -> Result<Vec<SampleRow>, AppError> {
    /* Sin candidatos: scan completo con JOIN a la MV de tendencias. Limita
     * por `publicado_at DESC` para priorizar fresh y mantener cardinalidad
     * acotada. La MV puede no tener todas las filas (samples nuevos sin
     * refrescar aún); COALESCE garantiza fallback a 0. */
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
            NULL::float8    AS "embedding_distance?",
            COALESCE(mv.likes_24h, 0)::float8          AS "likes_24h!",
            COALESCE(mv.reproducciones_24h, 0)::float8 AS "reproducciones_24h!",
            COALESCE(mv.descargas_7d, 0)::float8       AS "descargas_7d!",
            COALESCE(mv.follows_creador_7d, 0)::float8 AS "follows_creador_7d!",
            EXISTS (SELECT 1 FROM follows f WHERE f.seguidor_id = $1 AND f.seguido_id = s.creador_id) AS "creador_seguido!",
            0.0::float8 AS "likes_dados!",
            0.0::float8 AS "reproducciones!",
            0.0::float8 AS "tiempo_escucha!",
            0.0::float8 AS "descargas_propias!",
            0.0::float8 AS "completadas!",
            0.0::float8 AS "dislike_penalty!",
            NOT EXISTS (SELECT 1 FROM reproducciones r WHERE r.usuario_id = $1 AND r.sample_id = s.id) AS "nunca_reproducido!"
        FROM samples s
        LEFT JOIN mv_trending_samples mv ON mv.sample_id = s.id
        WHERE s.estado = 'activo'
          AND s.creador_id <> ALL($2::int[])
        ORDER BY s.publicado_at DESC NULLS LAST
        LIMIT $3
        "#,
        user_id,
        blocked,
        limit_i64,
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
            likes_24h: r.likes_24h,
            reproducciones_24h: r.reproducciones_24h,
            descargas_7d: r.descargas_7d,
            follows_creador_7d: r.follows_creador_7d,
            creador_seguido: r.creador_seguido,
            likes_dados: r.likes_dados,
            reproducciones: r.reproducciones,
            tiempo_escucha: r.tiempo_escucha,
            descargas_propias: r.descargas_propias,
            completadas: r.completadas,
            dislike_penalty: r.dislike_penalty,
            nunca_reproducido: r.nunca_reproducido,
        })
        .collect())
}
