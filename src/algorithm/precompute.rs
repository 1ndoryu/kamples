/* [174A-53] PrecomputeService — alimenta `algorithm::signals` con tendencias
 * pre-calculadas (likes/reproducciones/descargas/follows recientes).
 *
 * Reemplaza al `PrecomputadorFeed.php` legado (18 CTEs SQL inline). En la
 * arquitectura Rust el scoring corre en código (ver `algorithm::recommender`),
 * así que precompute = vista materializada `mv_trending_samples` refrescable
 * por worker o cron (174A-55).
 *
 * Beneficios:
 *   - O(1) por sample en el path caliente (lookup en HashMap).
 *   - El trabajo pesado (subqueries de agregación) corre 1× cada N minutos.
 *   - REFRESH MATERIALIZED VIEW CONCURRENTLY no bloquea lecturas. */

use std::collections::HashMap;

use sqlx::PgPool;

use crate::errors::AppError;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TrendStats {
    pub likes_24h: f64,
    pub reproducciones_24h: f64,
    pub descargas_7d: f64,
    pub follows_creador_7d: f64,
    pub tiempo_escucha_24h: f64,
    pub completadas_24h: f64,
    pub dislikes_7d: f64,
}

pub struct PrecomputeService;

impl PrecomputeService {
    /// Refresca la vista materializada. CONCURRENTLY no bloquea SELECTs en
    /// otras conexiones (PostgreSQL exige UNIQUE INDEX, definido en la
    /// migration). Si la vista nunca se ha poblado (primer arranque) cae a
    /// REFRESH no-concurrente.
    pub async fn refresh(pool: &PgPool) -> Result<(), AppError> {
        /* Literales fijos: sqlx ya envía REFRESH por el protocolo extendido, así
         * que la macro lo valida contra la BD en compilación como cualquier otra. */
        match sqlx::query!("REFRESH MATERIALIZED VIEW CONCURRENTLY mv_trending_samples")
            .execute(pool)
            .await
        {
            Ok(_) => Ok(()),
            Err(error) => {
                let msg = error.to_string();
                /* Postgres devuelve 55000 (object_not_in_prerequisite_state)
                 * cuando la MV no se ha populado nunca. En ese caso lo único
                 * válido es un refresh no-concurrente. */
                if msg.contains("has not been populated") || msg.contains("must be populated") {
                    /* Fallback no-concurrente: primer refresh, la vista aún no tiene datos. */
                    sqlx::query!("REFRESH MATERIALIZED VIEW mv_trending_samples")
                        .execute(pool)
                        .await?;
                    Ok(())
                } else {
                    Err(error.into())
                }
            }
        }
    }

    /// Lookup de tendencias para un set de sample IDs. Devuelve HashMap
    /// vacío si la MV no tiene esos IDs (samples nuevos no refrescados aún
    /// caen al default `TrendStats::default()` en el caller).
    pub async fn fetch_trends(
        pool: &PgPool,
        sample_ids: &[i32],
    ) -> Result<HashMap<i32, TrendStats>, AppError> {
        if sample_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = sqlx::query!(
            r#"
            SELECT
                sample_id              AS "sample_id!",
                likes_24h              AS "likes_24h!",
                reproducciones_24h     AS "reproducciones_24h!",
                descargas_7d           AS "descargas_7d!",
                follows_creador_7d     AS "follows_creador_7d!",
                tiempo_escucha_24h     AS "tiempo_escucha_24h!",
                completadas_24h        AS "completadas_24h!",
                dislikes_7d            AS "dislikes_7d!"
            FROM mv_trending_samples
            WHERE sample_id = ANY($1::int[])
            "#,
            sample_ids,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| {
                (
                    r.sample_id,
                    TrendStats {
                        likes_24h: f64::from(r.likes_24h),
                        reproducciones_24h: f64::from(r.reproducciones_24h),
                        descargas_7d: f64::from(r.descargas_7d),
                        follows_creador_7d: f64::from(r.follows_creador_7d),
                        tiempo_escucha_24h: r.tiempo_escucha_24h,
                        completadas_24h: f64::from(r.completadas_24h),
                        dislikes_7d: f64::from(r.dislikes_7d),
                    },
                )
            })
            .collect())
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn trend_stats_default_is_zero() {
        let stats = TrendStats::default();
        assert_eq!(stats.likes_24h, 0.0);
        assert_eq!(stats.reproducciones_24h, 0.0);
        assert_eq!(stats.descargas_7d, 0.0);
        assert_eq!(stats.follows_creador_7d, 0.0);
        assert_eq!(stats.tiempo_escucha_24h, 0.0);
        assert_eq!(stats.completadas_24h, 0.0);
        assert_eq!(stats.dislikes_7d, 0.0);
    }
}
