use sqlx::{PgPool, Postgres, QueryBuilder};

use super::AdminPanelRepository;
use crate::errors::AppError;
use crate::models::{
    AdminActivityPoint, AdminActivityQuery, AdminActivityResponse, AdminSummaryStats,
};

/* Conteos del endpoint /metrics (Prometheus). */
pub struct ConteosMetricas {
    pub ia_pendientes: i64,
    pub scraping_pendientes: i64,
    pub suscripciones_activas: i64,
}

impl AdminPanelRepository {
    /// Conteos para /metrics, consultados en paralelo. Si una consulta falla,
    /// devuelve -1 en ese contador: es el contrato existente del endpoint.
    pub async fn conteos_metricas(pool: &PgPool) -> ConteosMetricas {
        /* `ia_queue` usa la columna `status`: la antigua `estado` no existe y el
         * gauge quedaba siempre en -1 sin avisar. */
        let (ia, scraping, subs) = tokio::join!(
            sqlx::query_scalar!(
                r#"SELECT COUNT(*)::bigint AS "n!" FROM ia_queue WHERE status = 'pending'"#
            )
            .fetch_one(pool),
            sqlx::query_scalar!(
                r#"SELECT COUNT(*)::bigint AS "n!" FROM scraping_log WHERE estado = 'pendiente'"#
            )
            .fetch_one(pool),
            sqlx::query_scalar!(
                r#"SELECT COUNT(*)::bigint AS "n!" FROM suscripciones WHERE estado = 'activa'"#
            )
            .fetch_one(pool),
        );
        ConteosMetricas {
            ia_pendientes: ia.unwrap_or(-1),
            scraping_pendientes: scraping.unwrap_or(-1),
            suscripciones_activas: subs.unwrap_or(-1),
        }
    }

    pub async fn summary(pool: &PgPool) -> Result<AdminSummaryStats, AppError> {
        let mut builder = QueryBuilder::<Postgres>::new(
            r"SELECT
                    (SELECT COUNT(*)::bigint FROM usuarios_ext) AS total_usuarios,
                    (SELECT COUNT(*)::bigint FROM samples WHERE estado = 'activo') AS total_samples,
                    (SELECT COUNT(*)::bigint FROM descargas) AS total_descargas,
                    (SELECT COUNT(*)::bigint FROM publicaciones) AS total_publicaciones,
                    (SELECT COUNT(*)::bigint FROM publicaciones WHERE moderacion_estado = 'pendiente') AS pendientes_moderacion,
                    (SELECT COUNT(*)::bigint FROM reportes WHERE estado = 'pendiente') AS reportes_pendientes,
                    (SELECT COUNT(*)::bigint FROM usuarios_ext WHERE plan = 'pro') AS usuarios_pro,
                    (SELECT COUNT(*)::bigint FROM usuarios_ext WHERE plan = 'premium') AS usuarios_premium,
                    (SELECT COUNT(*)::bigint FROM samples WHERE created_at > NOW() - INTERVAL '7 days') AS samples_semana,
                    (SELECT COUNT(*)::bigint FROM usuarios_ext WHERE created_at > NOW() - INTERVAL '7 days') AS registros_semana",
        );

        let row = builder
            .build_query_as::<AdminSummaryStats>()
            .fetch_one(pool)
            .await?;
        Ok(row)
    }

    pub async fn activity(
        pool: &PgPool,
        query: &AdminActivityQuery,
    ) -> Result<AdminActivityResponse, AppError> {
        let days = query.dias.unwrap_or(7).clamp(7, 90) as i32;

        Ok(AdminActivityResponse {
            registros: activity_series(pool, "usuarios_ext", days).await?,
            uploads: activity_series(pool, "samples", days).await?,
            descargas: activity_series(pool, "descargas", days).await?,
        })
    }
}

async fn activity_series(
    pool: &PgPool,
    table: &str,
    days: i32,
) -> Result<Vec<AdminActivityPoint>, AppError> {
    let sql = format!(
        "SELECT TO_CHAR(DATE(created_at), 'YYYY-MM-DD') AS fecha, COUNT(*)::bigint AS total \
         FROM {table} \
         WHERE created_at > NOW() - ("
    );

    let mut builder = QueryBuilder::<Postgres>::new(sql);
    builder.push_bind(days);
    builder.push("::int * INTERVAL '1 day') GROUP BY DATE(created_at) ORDER BY fecha");

    let rows = builder
        .build_query_as::<AdminActivityPoint>()
        .fetch_all(pool)
        .await?;
    Ok(rows)
}
