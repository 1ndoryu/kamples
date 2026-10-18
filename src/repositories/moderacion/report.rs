use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::errors::AppError;

pub const AUTO_HIDE_SAMPLE_REPORT_THRESHOLD: i64 = 3;
pub const AUTO_HIDE_POST_REPORT_THRESHOLD: i64 = 3;

const LEGAL_REPORT_SAMPLE: &str = "legal_sample";
const LEGAL_REPORT_RELATION: &str = "legal_relacion";

#[derive(Debug, Clone)]
pub struct CreateReportRecord<'a> {
    pub tipo: &'a str,
    pub target_id: i32,
    pub reportador_id: Option<i32>,
    pub reportado_id: Option<i32>,
    pub razon: &'a str,
    pub detalles: Option<&'a str>,
    pub ip_origen: Option<&'a str>,
}

pub struct LegalReportRow {
    pub id: i32,
    pub tipo: String,
    pub target_id: i32,
    pub razon: String,
    pub detalles: Option<String>,
    pub ip_origen: Option<String>,
    pub estado: String,
    pub created_at: DateTime<Utc>,
}

pub struct ReportRepository;

impl ReportRepository {
    pub async fn create_report(
        pool: &PgPool,
        record: &CreateReportRecord<'_>,
    ) -> Result<i32, AppError> {
        let id = sqlx::query_scalar::<_, i32>(
            "INSERT INTO reportes (
                tipo,
                target_id,
                reportador_id,
                reportado_id,
                razon,
                detalles,
                ip_origen,
                estado
             )
             VALUES ($1, $2, $3, $4, $5, $6, $7, 'pendiente')
             RETURNING id",
        )
        .bind(record.tipo)
        .bind(record.target_id)
        .bind(record.reportador_id)
        .bind(record.reportado_id)
        .bind(record.razon)
        .bind(record.detalles)
        .bind(record.ip_origen)
        .fetch_one(pool)
        .await?;

        Ok(id)
    }

    pub async fn has_reported_target(
        pool: &PgPool,
        tipo: &str,
        target_id: i32,
        reportador_id: i32,
    ) -> Result<bool, AppError> {
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(
                SELECT 1
                FROM reportes
                WHERE tipo = $1
                  AND target_id = $2
                  AND reportador_id = $3
            )",
        )
        .bind(tipo)
        .bind(target_id)
        .bind(reportador_id)
        .fetch_one(pool)
        .await?;

        Ok(exists)
    }

    pub async fn count_by_reporter_and_type_since(
        pool: &PgPool,
        reportador_id: i32,
        tipo: &str,
        since: DateTime<Utc>,
    ) -> Result<i64, AppError> {
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)
             FROM reportes
             WHERE reportador_id = $1
               AND tipo = $2
               AND created_at >= $3",
        )
        .bind(reportador_id)
        .bind(tipo)
        .bind(since)
        .fetch_one(pool)
        .await?;

        Ok(total)
    }

    pub async fn count_by_ip_and_types_since(
        pool: &PgPool,
        ip_origen: &str,
        tipos: &[&str],
        since: DateTime<Utc>,
    ) -> Result<i64, AppError> {
        if tipos.is_empty() {
            return Ok(0);
        }

        let tipos = tipos
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>();
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)
             FROM reportes
             WHERE ip_origen = $1
               AND tipo = ANY($2)
               AND created_at >= $3",
        )
        .bind(ip_origen)
        .bind(tipos)
        .bind(since)
        .fetch_one(pool)
        .await?;

        Ok(total)
    }

    pub async fn count_recent_user_reports_about_user(
        pool: &PgPool,
        target_id: i32,
        since: DateTime<Utc>,
    ) -> Result<i64, AppError> {
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)
             FROM reportes
             WHERE tipo = 'usuario'
               AND target_id = $1
               AND created_at >= $2",
        )
        .bind(target_id)
        .bind(since)
        .fetch_one(pool)
        .await?;

        Ok(total)
    }

    pub async fn target_exists(
        pool: &PgPool,
        tipo: &str,
        target_id: i32,
    ) -> Result<bool, AppError> {
        let exists = match tipo {
            "usuario" => {
                sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(SELECT 1 FROM usuarios_ext WHERE id = $1)",
                )
                .bind(target_id)
                .fetch_one(pool)
                .await?
            }
            "publicacion" => sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM publicaciones WHERE id = $1 AND eliminado_en IS NULL)",
            )
            .bind(target_id)
            .fetch_one(pool)
            .await?,
            "comentario" => {
                sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(SELECT 1 FROM comentarios WHERE id = $1)",
                )
                .bind(target_id)
                .fetch_one(pool)
                .await?
            }
            "sample" | LEGAL_REPORT_SAMPLE => {
                sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(SELECT 1 FROM samples WHERE id = $1 AND eliminado_en IS NULL)",
                )
                .bind(target_id)
                .fetch_one(pool)
                .await?
            }
            LEGAL_REPORT_RELATION => {
                sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(SELECT 1 FROM relaciones_sample WHERE id = $1)",
                )
                .bind(target_id)
                .fetch_one(pool)
                .await?
            }
            _ => false,
        };

        Ok(exists)
    }

    pub async fn pending_counts_for_targets(
        pool: &PgPool,
        tipo: &str,
        target_ids: &[i32],
    ) -> Result<HashMap<i32, i64>, AppError> {
        if target_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let rows = sqlx::query!(
            "SELECT
                                target_id AS \"target_id!\",
                                COUNT(*)::bigint AS \"pending_count!\"
             FROM reportes
             WHERE tipo = $1
               AND COALESCE(estado, 'pendiente') = 'pendiente'
               AND target_id = ANY($2)
                         GROUP BY target_id",
            tipo,
            target_ids,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| (row.target_id, row.pending_count))
            .collect())
    }

    pub async fn is_auto_hidden_for_viewer(
        pool: &PgPool,
        tipo: &str,
        target_id: i32,
        creator_id: i32,
        viewer_id: Option<i32>,
        threshold: i64,
    ) -> Result<bool, AppError> {
        if threshold <= 0 || viewer_id == Some(creator_id) {
            return Ok(false);
        }

        let pending = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)
             FROM reportes
             WHERE tipo = $1
               AND target_id = $2
               AND COALESCE(estado, 'pendiente') = 'pendiente'",
        )
        .bind(tipo)
        .bind(target_id)
        .fetch_one(pool)
        .await?;

        Ok(pending >= threshold)
    }

    pub async fn list_pending_legal_reports(
        pool: &PgPool,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<LegalReportRow>, AppError> {
        let rows = sqlx::query!(
            "SELECT
                id,
                tipo,
                target_id,
                razon,
                detalles,
                ip_origen,
                                COALESCE(estado, 'pendiente') AS \"estado!\",
                                created_at AS \"created_at!\"
             FROM reportes
             WHERE tipo IN ('legal_sample', 'legal_relacion')
               AND COALESCE(estado, 'pendiente') = 'pendiente'
             ORDER BY created_at DESC
                         LIMIT $1 OFFSET $2",
            limit,
            offset,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| LegalReportRow {
                id: row.id,
                tipo: row.tipo,
                target_id: row.target_id,
                razon: row.razon,
                detalles: row.detalles,
                ip_origen: row.ip_origen,
                estado: row.estado,
                created_at: row.created_at,
            })
            .collect())
    }

    pub async fn count_pending_legal_reports(pool: &PgPool) -> Result<i64, AppError> {
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)
             FROM reportes
             WHERE tipo IN ('legal_sample', 'legal_relacion')
               AND COALESCE(estado, 'pendiente') = 'pendiente'",
        )
        .fetch_one(pool)
        .await?;

        Ok(total)
    }
}
