/* [274A-46] Repositorio de automatizacion: lotes e historial salen del handler
 * para que reactivar pueda compartir el mismo contrato de estado.
 * Consultas con macros sqlx: comprobadas al compilar y cacheadas en .sqlx. */

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};

pub struct AdminAutomationRepository;

#[derive(Debug, Clone, FromRow)]
pub struct AutomationBatchRow {
    pub id: i32,
    pub tipo: String,
    pub estado: String,
    pub iniciado_at: DateTime<Utc>,
    pub completado_at: Option<DateTime<Utc>>,
    pub exitosos: i32,
    pub fallidos: i32,
    pub recortes: i32,
    pub samples_publicados: i32,
    pub canciones_nuevas: i32,
    pub sampleos_nuevos: i32,
    pub error_mensaje: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct AutomationBatchUpdate {
    pub estado: String,
    pub exitosos: i32,
    pub fallidos: i32,
    pub recortes: i32,
    pub samples_publicados: i32,
    pub canciones_nuevas: i32,
    pub sampleos_nuevos: i32,
    pub error_mensaje: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

impl AdminAutomationRepository {
    pub async fn create_batch(
        pool: &PgPool,
        process_type: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<AutomationBatchRow, sqlx::Error> {
        sqlx::query_as!(
            AutomationBatchRow,
            r"INSERT INTO lotes_procesamiento (
                    tipo, estado, metadata
                )
                VALUES ($1, 'ejecutando', COALESCE($2, '{}'::jsonb))
                RETURNING id, tipo, estado, iniciado_at, completado_at,
                          exitosos, fallidos, recortes, samples_publicados,
                          canciones_nuevas, sampleos_nuevos, error_mensaje, metadata",
            process_type,
            metadata
        )
        .fetch_one(pool)
        .await
    }

    pub async fn find_batch(
        pool: &PgPool,
        batch_id: i64,
    ) -> Result<Option<AutomationBatchRow>, sqlx::Error> {
        sqlx::query_as!(
            AutomationBatchRow,
            r"SELECT id, tipo, estado, iniciado_at, completado_at,
                      exitosos, fallidos, recortes, samples_publicados,
                      canciones_nuevas, sampleos_nuevos, error_mensaje, metadata
                 FROM lotes_procesamiento
                WHERE id = $1::bigint",
            batch_id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn complete_batch(
        pool: &PgPool,
        batch_id: i64,
        update: &AutomationBatchUpdate,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r"UPDATE lotes_procesamiento
                   SET estado = $2,
                       completado_at = NOW(),
                       exitosos = $3,
                       fallidos = $4,
                       recortes = $5,
                       samples_publicados = $6,
                       canciones_nuevas = $7,
                       sampleos_nuevos = $8,
                       error_mensaje = $9,
                       metadata = COALESCE($10, metadata, '{}'::jsonb)
                     WHERE id = $1::bigint",
            batch_id,
            update.estado,
            update.exitosos,
            update.fallidos,
            update.recortes,
            update.samples_publicados,
            update.canciones_nuevas,
            update.sampleos_nuevos,
            update.error_mensaje,
            update.metadata
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn latest_batch(
        pool: &PgPool,
        process_type: &str,
    ) -> Result<Option<AutomationBatchRow>, sqlx::Error> {
        sqlx::query_as!(
            AutomationBatchRow,
            r"SELECT id, tipo, estado, iniciado_at, completado_at,
                      exitosos, fallidos, recortes, samples_publicados,
                      canciones_nuevas, sampleos_nuevos, error_mensaje, metadata
                 FROM lotes_procesamiento
                WHERE tipo = $1
                ORDER BY iniciado_at DESC
                LIMIT 1",
            process_type
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn consecutive_failures(
        pool: &PgPool,
        process_type: &str,
    ) -> Result<i32, sqlx::Error> {
        let rows = sqlx::query!(
            r"SELECT estado, error_mensaje
                 FROM lotes_procesamiento
                WHERE tipo = $1
                ORDER BY iniciado_at DESC
                LIMIT 20",
            process_type
        )
        .fetch_all(pool)
        .await?;

        let mut count = 0_i32;
        for fila in rows {
            if fila.estado == "error" {
                if is_missing_report_error(fila.error_mensaje.as_deref()) {
                    continue;
                }
                count += 1;
            } else {
                break;
            }
        }

        Ok(count)
    }

    pub async fn list_history(
        pool: &PgPool,
        process_type: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AutomationBatchRow>, sqlx::Error> {
        sqlx::query_as!(
            AutomationBatchRow,
            r"SELECT id, tipo, estado, iniciado_at, completado_at,
                      exitosos, fallidos, recortes, samples_publicados,
                      canciones_nuevas, sampleos_nuevos, error_mensaje, metadata
                 FROM lotes_procesamiento
                WHERE ($1::text IS NULL OR tipo = $1)
                ORDER BY iniciado_at DESC
                LIMIT $2 OFFSET $3",
            process_type,
            limit,
            offset
        )
        .fetch_all(pool)
        .await
    }

    pub async fn count_history(
        pool: &PgPool,
        process_type: Option<&str>,
    ) -> Result<i64, sqlx::Error> {
        let total = sqlx::query_scalar!(
            r#"SELECT COUNT(*)::bigint AS "total!"
                 FROM lotes_procesamiento
                WHERE ($1::text IS NULL OR tipo = $1)"#,
            process_type
        )
        .fetch_one(pool)
        .await?;

        Ok(total)
    }
}

fn is_missing_report_error(error_message: Option<&str>) -> bool {
    error_message.is_some_and(|message| {
        message.contains("termino sin reportar cierre")
            || message.contains("terminó sin reportar cierre")
    })
}
