use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgPool;

use crate::models::{
    CreatorDashboardIncomePoint, CreatorDashboardIncomeQuery, CreatorDashboardSampleStat,
    CreatorDashboardStats, CreatorDashboardTransaction, CreatorDashboardTransactionType,
    CreatorDashboardTransactionsQuery,
};

const TRANSACTIONS_PER_PAGE: i64 = 20;

#[derive(Debug, Clone)]
struct CreatorDashboardStatsRow {
    ingresos_total: f64,
    ingresos_mes: f64,
    ingresos_anterior: f64,
    descargas_total: i64,
    descargas_mes: i64,
    reproducciones_total: i64,
    reproducciones_mes: i64,
    seguidores_total: i64,
    seguidores_nuevos_mes: i64,
    samples_publicados: i64,
}

#[derive(Debug, Clone)]
struct CreatorDashboardSampleRow {
    id: i32,
    titulo: String,
    slug: String,
    descargas: i64,
    reproducciones: i64,
    likes: i64,
    ingresos: f64,
}

#[derive(Debug, Clone)]
struct CreatorDashboardTransactionRow {
    id: i32,
    fecha: DateTime<Utc>,
    tipo: String,
    sample: String,
    comprador: String,
    monto: f64,
    comision: f64,
    neto: f64,
}

#[derive(Debug, Clone)]
struct CreatorDashboardIncomeRow {
    fecha: NaiveDate,
    monto: f64,
}

pub struct CreatorDashboardRepository;

impl CreatorDashboardRepository {
    pub async fn stats(
        pool: &PgPool,
        user_id: i32,
    ) -> Result<Option<CreatorDashboardStats>, sqlx::Error> {
        sqlx::query_as!(
            CreatorDashboardStatsRow,
            r#"
            SELECT
                COALESCE((
                    SELECT SUM(t.pago_creador)::double precision
                    FROM transacciones t
                    WHERE t.creador_id = $1
                      AND t.estado IN ('completada', 'completed')
                ), 0)::double precision AS "ingresos_total!",
                COALESCE((
                    SELECT SUM(t.pago_creador)::double precision
                    FROM transacciones t
                    WHERE t.creador_id = $1
                      AND t.estado IN ('completada', 'completed')
                      AND t.created_at >= date_trunc('month', NOW())
                ), 0)::double precision AS "ingresos_mes!",
                COALESCE((
                    SELECT SUM(t.pago_creador)::double precision
                    FROM transacciones t
                    WHERE t.creador_id = $1
                      AND t.estado IN ('completada', 'completed')
                      AND t.created_at >= date_trunc('month', NOW()) - INTERVAL '1 month'
                      AND t.created_at < date_trunc('month', NOW())
                ), 0)::double precision AS "ingresos_anterior!",
                COALESCE(u.total_descargas, 0)::bigint AS "descargas_total!",
                (
                    SELECT COUNT(*)
                    FROM descargas d
                    JOIN samples s ON s.id = d.sample_id
                    WHERE s.creador_id = u.id
                      AND d.created_at >= date_trunc('month', NOW())
                ) AS "descargas_mes!",
                COALESCE((
                    SELECT SUM(s.total_reproducciones)::bigint
                    FROM samples s
                    WHERE s.creador_id = u.id
                      AND s.eliminado_en IS NULL
                ), 0)::bigint AS "reproducciones_total!",
                (
                    SELECT COUNT(*)
                    FROM reproducciones r
                    JOIN samples s ON s.id = r.sample_id
                    WHERE s.creador_id = u.id
                      AND r.created_at >= date_trunc('month', NOW())
                ) AS "reproducciones_mes!",
                COALESCE(u.total_seguidores, 0)::bigint AS "seguidores_total!",
                (
                    SELECT COUNT(*)
                    FROM follows f
                    WHERE f.seguido_id = u.id
                      AND f.created_at >= date_trunc('month', NOW())
                ) AS "seguidores_nuevos_mes!",
                COALESCE(u.total_samples, 0)::bigint AS "samples_publicados!"
            FROM usuarios_ext u
            WHERE u.id = $1
            LIMIT 1
            "#,
            user_id
        )
        .fetch_optional(pool)
        .await
        .map(|row| {
            row.map(|row| CreatorDashboardStats {
                ingresos_total: row.ingresos_total,
                ingresos_mes: row.ingresos_mes,
                ingresos_anterior: row.ingresos_anterior,
                descargas_total: row.descargas_total,
                descargas_mes: row.descargas_mes,
                reproducciones_total: row.reproducciones_total,
                reproducciones_mes: row.reproducciones_mes,
                seguidores_total: row.seguidores_total,
                seguidores_nuevos_mes: row.seguidores_nuevos_mes,
                samples_publicados: row.samples_publicados,
            })
        })
    }

    pub async fn top_samples(
        pool: &PgPool,
        user_id: i32,
    ) -> Result<Vec<CreatorDashboardSampleStat>, sqlx::Error> {
        sqlx::query_as!(
            CreatorDashboardSampleRow,
            r#"
            SELECT
                s.id,
                s.titulo AS "titulo!",
                s.slug AS "slug!",
                COALESCE(s.total_descargas, 0)::bigint AS "descargas!",
                COALESCE(s.total_reproducciones, 0)::bigint AS "reproducciones!",
                COALESCE(s.total_likes, 0)::bigint AS "likes!",
                COALESCE((
                    SELECT SUM(t.pago_creador)::double precision
                    FROM transacciones t
                    WHERE t.sample_id = s.id
                      AND t.estado IN ('completada', 'completed')
                ), 0)::double precision AS "ingresos!"
            FROM samples s
            WHERE s.creador_id = $1
              AND s.estado = 'activo'
              AND s.eliminado_en IS NULL
            ORDER BY s.total_descargas DESC, s.id DESC
            LIMIT 10
            "#,
            user_id
        )
        .fetch_all(pool)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|row| CreatorDashboardSampleStat {
                    id: row.id,
                    titulo: row.titulo,
                    slug: row.slug,
                    descargas: row.descargas,
                    reproducciones: row.reproducciones,
                    likes: row.likes,
                    ingresos: row.ingresos,
                })
                .collect()
        })
    }

    pub async fn transactions(
        pool: &PgPool,
        user_id: i32,
        query: &CreatorDashboardTransactionsQuery,
    ) -> Result<Vec<CreatorDashboardTransaction>, sqlx::Error> {
        let page = query.page();
        let offset = (page - 1) * TRANSACTIONS_PER_PAGE;

        sqlx::query_as!(
            CreatorDashboardTransactionRow,
            r#"
            SELECT
                t.id,
                t.created_at AS "fecha!",
                CASE
                    WHEN t.tipo = 'compra_sample' THEN 'venta'
                    WHEN t.tipo = 'descarga' THEN 'descarga'
                    WHEN t.tipo = 'suscripcion' THEN 'suscripcion'
                    ELSE 'venta'
                END AS "tipo!",
                COALESCE(s.titulo, '') AS "sample!",
                COALESCE(u.username, '') AS "comprador!",
                CAST(t.monto AS double precision) AS "monto!",
                CAST(t.comision_plataforma AS double precision) AS "comision!",
                CAST(t.pago_creador AS double precision) AS "neto!"
            FROM transacciones t
            LEFT JOIN samples s ON s.id = t.sample_id
            LEFT JOIN usuarios_ext u ON u.id = t.comprador_id
            WHERE t.creador_id = $1
              AND t.tipo IN ('compra_sample', 'descarga', 'suscripcion')
            ORDER BY t.created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            user_id,
            TRANSACTIONS_PER_PAGE,
            offset
        )
        .fetch_all(pool)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|row| CreatorDashboardTransaction {
                    id: row.id,
                    fecha: row.fecha,
                    tipo: parse_transaction_type(&row.tipo),
                    sample: row.sample,
                    comprador: row.comprador,
                    monto: row.monto,
                    comision: row.comision,
                    neto: row.neto,
                })
                .collect()
        })
    }

    pub async fn income_series(
        pool: &PgPool,
        user_id: i32,
        query: &CreatorDashboardIncomeQuery,
    ) -> Result<Vec<CreatorDashboardIncomePoint>, sqlx::Error> {
        let days = query.period().days();

        sqlx::query_as!(
            CreatorDashboardIncomeRow,
            r#"
            SELECT
                DATE(t.created_at) AS "fecha!",
                COALESCE(SUM(t.pago_creador), 0)::double precision AS "monto!"
            FROM transacciones t
            WHERE t.creador_id = $1
              AND t.estado IN ('completada', 'completed')
              AND t.created_at >= NOW() - make_interval(days => $2)
            GROUP BY DATE(t.created_at)
                        ORDER BY DATE(t.created_at) ASC
            "#,
            user_id,
            days
        )
        .fetch_all(pool)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|row| CreatorDashboardIncomePoint {
                    fecha: row.fecha,
                    monto: row.monto,
                })
                .collect()
        })
    }
}

fn parse_transaction_type(value: &str) -> CreatorDashboardTransactionType {
    match value {
        "descarga" => CreatorDashboardTransactionType::Descarga,
        "suscripcion" => CreatorDashboardTransactionType::Suscripcion,
        _ => CreatorDashboardTransactionType::Venta,
    }
}
