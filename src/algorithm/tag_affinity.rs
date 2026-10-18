/* [174A-54] TagAffinityService — pre-calcula afinidad tag↔usuario.
 *
 * Reemplazo Rust del `TagAffinityService.php` legado. Mantiene la decisión
 * arquitectónica original: el cómputo pesado (UNNEST de tags + 7 JOINs sobre
 * likes/reproducciones/descargas) corre 1× en el servidor de BD vía un único
 * INSERT-FROM-WITH (CTEs `utag_*` + `merged`), y los lectores (`recommender`,
 * scoring) consultan `user_tag_scores` con un JOIN indexado de O(1).
 *
 * Schema requerido (ya existe en migration 20260417000011):
 *   user_tag_scores(user_id, tag, w_likes, w_repro, w_tiempo, w_descargas,
 *                   w_completadas, w_dislikes, w_ctx, updated_at)
 *
 * Trade-off: La query es enorme pero corre en O(N) sobre la BD (donde N =
 * samples activos), no O(N×M) por request. El alternativo "materializar en
 * Rust" obligaría a traer 26K filas al backend → red + memoria. Rechazado.
 *
 * Hooks:
 *   - 174A-55 (worker `algo_planner`): invoca `recalculate_active` cada N min.
 *   - 174A-58 (`POST /samples/{id}/play`): invoca `schedule_recalc` post-200. */

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use sqlx::{PgPool, Postgres, Transaction};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::errors::AppError;

/// Pesos por tag para un usuario. Espejo de `user_tag_scores`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TagWeights {
    pub w_likes: f32,
    pub w_repro: f32,
    pub w_tiempo: f32,
    pub w_descargas: f32,
    pub w_completadas: f32,
    pub w_dislikes: f32,
    pub w_ctx: f32,
}

pub struct TagAffinityService;

/* Lock global de recálculos en vuelo (per user_id). Análogo al
 * `adquirirLock` del legado pero in-process. Para varios procesos Rust en
 * paralelo, usar Redis SETNX (174A-55). */
pub type InflightLocks = Arc<Mutex<HashSet<i32>>>;

impl TagAffinityService {
    /// Devuelve true si el usuario tiene al menos un score reciente.
    /// Usado por `recommender` para elegir path optimizado vs cold-start.
    pub async fn has_recent_scores(pool: &PgPool, user_id: i32) -> Result<bool, AppError> {
        let exists: Option<i32> =
            sqlx::query_scalar("SELECT 1 FROM user_tag_scores WHERE user_id = $1 LIMIT 1")
                .bind(user_id)
                .fetch_optional(pool)
                .await?;
        Ok(exists.is_some())
    }

    /// Lookup de pesos para un usuario. Devuelve HashMap vacío si nunca se
    /// recalculó; el caller decide si caer a cold-start o programar recálculo.
    pub async fn fetch_for_user(
        pool: &PgPool,
        user_id: i32,
    ) -> Result<HashMap<String, TagWeights>, AppError> {
        let rows = sqlx::query!(
            r#"
            SELECT
                tag           AS "tag!",
                w_likes       AS "w_likes!",
                w_repro       AS "w_repro!",
                w_tiempo      AS "w_tiempo!",
                w_descargas   AS "w_descargas!",
                w_completadas AS "w_completadas!",
                w_dislikes    AS "w_dislikes!",
                w_ctx         AS "w_ctx!"
            FROM user_tag_scores
            WHERE user_id = $1
            "#,
            user_id,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| {
                (
                    r.tag,
                    TagWeights {
                        w_likes: r.w_likes,
                        w_repro: r.w_repro,
                        w_tiempo: r.w_tiempo,
                        w_descargas: r.w_descargas,
                        w_completadas: r.w_completadas,
                        w_dislikes: r.w_dislikes,
                        w_ctx: r.w_ctx,
                    },
                )
            })
            .collect())
    }

    /// Recalcula TODOS los pesos de tags para un usuario en una transacción.
    /// (1) DELETE filas viejas; (2) INSERT-FROM-WITH con las 7 CTEs `utag_*`.
    /// Devuelve cantidad de filas insertadas.
    pub async fn recalculate_for_user(pool: &PgPool, user_id: i32) -> Result<u64, AppError> {
        let mut tx = pool.begin().await?;
        borrar_pesos_usuario(&mut tx, user_id).await?;
        let filas = insertar_pesos_usuario(&mut tx, user_id).await?;
        tx.commit().await?;
        Ok(filas)
    }

    /// Recalcula para todos los usuarios con actividad reciente (24h).
    /// Worker `algo_planner` (174A-55) lo invoca periódicamente.
    pub async fn recalculate_active(pool: &PgPool, limit: i64) -> Result<u32, AppError> {
        let usuarios = sqlx::query!(
            r#"
            SELECT DISTINCT usuario_id AS "usuario_id!"
            FROM reproducciones
            WHERE created_at > NOW() - INTERVAL '24 hours'
            LIMIT $1
            "#,
            limit,
        )
        .fetch_all(pool)
        .await?;

        let mut count = 0u32;
        for row in usuarios {
            if Self::recalculate_for_user(pool, row.usuario_id)
                .await
                .is_ok()
            {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Lanza recálculo asíncrono usando un lock in-process para evitar
    /// recálculos duplicados concurrentes del mismo usuario.
    ///
    /// `locks` es un `InflightLocks` compartido (típicamente vive en
    /// `AppState`). Devuelve `Some(handle)` si se lanzó, `None` si ya
    /// había uno en vuelo para este usuario.
    pub fn schedule_recalc(pool: PgPool, user_id: i32, locks: &InflightLocks) -> JoinHandle<()> {
        let locks_clone = Arc::clone(locks);
        tokio::spawn(async move {
            {
                let mut guard = locks_clone.lock().await;
                if guard.contains(&user_id) {
                    return;
                }
                guard.insert(user_id);
            }

            if let Err(error) = Self::recalculate_for_user(&pool, user_id).await {
                tracing::warn!(
                    target: "kamples.tag_affinity",
                    %error,
                    user_id,
                    "recalculo async fallo"
                );
            }

            let mut guard = locks_clone.lock().await;
            guard.remove(&user_id);
        })
    }

    /// Helper para construir el `InflightLocks` compartido.
    #[must_use]
    pub fn new_inflight_locks() -> InflightLocks {
        Arc::new(Mutex::new(HashSet::new()))
    }
}

/* Borra los pesos previos del usuario dentro de la misma transacción del recálculo,
 * para que el INSERT posterior reemplace el conjunto completo sin filas huérfanas. */
async fn borrar_pesos_usuario(
    tx: &mut Transaction<'_, Postgres>,
    user_id: i32,
) -> Result<(), AppError> {
    sqlx::query!("DELETE FROM user_tag_scores WHERE user_id = $1", user_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/* Inserta los pesos recalculados con una sola query CTE: agrega todos los pesos por tag
 * desde 7 fuentes y los mergea via GROUP BY. utag_ctx limita a top-8 tags por frecuencia
 * para evitar inflar con tags marginales (paridad con el legado). */
async fn insertar_pesos_usuario(tx: &mut Transaction<'_, Postgres>, user_id: i32) -> Result<u64, AppError> {
    let result = sqlx::query!(
            r#"
            WITH enriched AS (
                SELECT s.id AS sample_id, s.tags_enriquecidos AS etags
                FROM samples s
                WHERE s.estado = 'activo'
            ),
            utag_likes AS (
                SELECT UNNEST(e.etags) AS tag,
                    SUM(CASE WHEN l.reaccion = 'encanta' THEN 2 ELSE 1 END)::float AS peso
                FROM likes l JOIN enriched e ON l.target_id = e.sample_id
                WHERE l.usuario_id = $1 AND l.tipo = 'sample'
                  AND l.reaccion IN ('like', 'encanta')
                GROUP BY tag
            ),
            utag_repro AS (
                SELECT UNNEST(e.etags) AS tag, COUNT(*)::float AS freq
                FROM reproducciones r JOIN enriched e ON r.sample_id = e.sample_id
                WHERE r.usuario_id = $1
                GROUP BY tag
            ),
            utag_tiempo AS (
                SELECT UNNEST(e.etags) AS tag, COUNT(*)::float AS freq
                FROM reproducciones r JOIN enriched e ON r.sample_id = e.sample_id
                WHERE r.usuario_id = $1 AND r.duracion_escuchada > 10
                GROUP BY tag
            ),
            utag_descargas AS (
                SELECT UNNEST(e.etags) AS tag, COUNT(*)::float AS freq
                FROM descargas d JOIN enriched e ON d.sample_id = e.sample_id
                WHERE d.usuario_id = $1
                GROUP BY tag
            ),
            utag_completadas AS (
                SELECT UNNEST(e.etags) AS tag, COUNT(*)::float AS freq
                FROM reproducciones r JOIN enriched e ON r.sample_id = e.sample_id
                WHERE r.usuario_id = $1 AND r.completada = true
                GROUP BY tag
            ),
            utag_dislikes AS (
                SELECT UNNEST(e.etags) AS tag, COUNT(*)::float AS freq
                FROM likes l JOIN enriched e ON l.target_id = e.sample_id
                WHERE l.usuario_id = $1 AND l.tipo = 'sample'
                  AND l.reaccion = 'dislike'
                GROUP BY tag
            ),
            utag_ctx AS (
                SELECT tag, SUM(freq)::float AS freq FROM (
                    SELECT UNNEST(e.etags) AS tag, 1 AS freq
                    FROM likes l JOIN enriched e ON l.target_id = e.sample_id
                    WHERE l.usuario_id = $1 AND l.tipo = 'sample'
                      AND l.reaccion IN ('like', 'encanta')
                ) t GROUP BY tag ORDER BY freq DESC LIMIT 8
            ),
            merged AS (
                SELECT tag,
                    COALESCE(SUM(w_likes), 0)::real        AS w_likes,
                    COALESCE(SUM(w_repro), 0)::real        AS w_repro,
                    COALESCE(SUM(w_tiempo), 0)::real       AS w_tiempo,
                    COALESCE(SUM(w_descargas), 0)::real    AS w_descargas,
                    COALESCE(SUM(w_completadas), 0)::real  AS w_completadas,
                    COALESCE(SUM(w_dislikes), 0)::real     AS w_dislikes,
                    GREATEST(SUM(w_ctx), 0)::real          AS w_ctx
                FROM (
                    SELECT tag, peso AS w_likes, 0::float AS w_repro, 0::float AS w_tiempo,
                           0::float AS w_descargas, 0::float AS w_completadas,
                           0::float AS w_dislikes, 0::float AS w_ctx
                    FROM utag_likes
                    UNION ALL SELECT tag, 0, freq, 0, 0, 0, 0, 0 FROM utag_repro
                    UNION ALL SELECT tag, 0, 0, freq, 0, 0, 0, 0 FROM utag_tiempo
                    UNION ALL SELECT tag, 0, 0, 0, freq, 0, 0, 0 FROM utag_descargas
                    UNION ALL SELECT tag, 0, 0, 0, 0, freq, 0, 0 FROM utag_completadas
                    UNION ALL SELECT tag, 0, 0, 0, 0, 0, freq, 0 FROM utag_dislikes
                    UNION ALL SELECT tag, 0, 0, 0, 0, 0, 0, freq FROM utag_ctx
                ) combined
                GROUP BY tag
            )
            INSERT INTO user_tag_scores (user_id, tag, w_likes, w_repro, w_tiempo,
                w_descargas, w_completadas, w_dislikes, w_ctx, updated_at)
            SELECT $1, tag, w_likes, w_repro, w_tiempo, w_descargas, w_completadas,
                w_dislikes, w_ctx, NOW()
            FROM merged
            ON CONFLICT (user_id, tag) DO UPDATE SET
                w_likes       = EXCLUDED.w_likes,
                w_repro       = EXCLUDED.w_repro,
                w_tiempo      = EXCLUDED.w_tiempo,
                w_descargas   = EXCLUDED.w_descargas,
                w_completadas = EXCLUDED.w_completadas,
                w_dislikes    = EXCLUDED.w_dislikes,
                w_ctx         = EXCLUDED.w_ctx,
                updated_at    = NOW()
            "#,
            user_id,
        )
        .execute(&mut **tx)
        .await?;
    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_weights_default_is_zero() {
        let w = TagWeights::default();
        assert!(w.w_likes.abs() < f32::EPSILON);
        assert!(w.w_repro.abs() < f32::EPSILON);
        assert!(w.w_tiempo.abs() < f32::EPSILON);
        assert!(w.w_descargas.abs() < f32::EPSILON);
        assert!(w.w_completadas.abs() < f32::EPSILON);
        assert!(w.w_dislikes.abs() < f32::EPSILON);
        assert!(w.w_ctx.abs() < f32::EPSILON);
    }

    #[tokio::test]
    async fn inflight_locks_isolate_users() {
        let locks = TagAffinityService::new_inflight_locks();
        {
            let mut guard = locks.lock().await;
            guard.insert(1);
        }
        let guard = locks.lock().await;
        assert!(guard.contains(&1));
        assert!(!guard.contains(&2));
    }
}
