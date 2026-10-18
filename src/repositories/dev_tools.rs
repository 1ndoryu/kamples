/* [274A-54..58] Repositorio de operaciones dev Sample Discovery.
 * Extrae SQL fuera de handlers para mantener DIP y reducir superficie del controlador.
 * Consultas con macros sqlx: comprobadas al compilar y cacheadas en .sqlx. */

use serde_json::Value;
use sqlx::{FromRow, PgPool};

use crate::errors::AppError;

pub struct DevToolsRepository;

#[derive(Debug, Clone, FromRow)]
pub struct ScrapingPendiente {
    pub url: String,
    pub tipo_pagina: String,
}

#[derive(Debug, FromRow)]
struct RelacionRecorteRow {
    id: i32,
    fuente_youtube_id: Option<String>,
    destino_youtube_id: Option<String>,
    sample_fuente_id: Option<i32>,
    sample_destino_id: Option<i32>,
    timings_fuente: Value,
    timings_destino: Value,
}

impl DevToolsRepository {
    pub async fn purgar_canciones(pool: &PgPool) -> Result<(), AppError> {
        sqlx::query!(
            "TRUNCATE TABLE
                cola_extraccion_samples,
                scraping_log,
                relaciones_sample,
                canciones_artistas,
                canciones,
                artistas_musicales
             RESTART IDENTITY CASCADE",
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn tomar_pendiente_scraping(
        pool: &PgPool,
    ) -> Result<Option<ScrapingPendiente>, AppError> {
        let row = sqlx::query_as!(
            ScrapingPendiente,
            "SELECT url, tipo_pagina
             FROM scraping_log
             WHERE estado = 'pendiente'
             ORDER BY created_at ASC, id ASC
             LIMIT 1",
        )
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn encolar_recorte_bilateral(
        pool: &PgPool,
        relacion_id: i32,
    ) -> Result<Vec<i32>, AppError> {
        let relacion = sqlx::query_as!(
            RelacionRecorteRow,
            r#"SELECT
                r.id,
                cf.youtube_id AS fuente_youtube_id,
                cd.youtube_id AS destino_youtube_id,
                r.sample_fuente_id,
                r.sample_destino_id,
                COALESCE(r.timings_fuente, '[]'::jsonb) AS "timings_fuente!",
                COALESCE(r.timings_destino, '[]'::jsonb) AS "timings_destino!"
             FROM relaciones_sample r
             JOIN canciones cf ON cf.id = r.cancion_fuente_id
             JOIN canciones cd ON cd.id = r.cancion_destino_id
             WHERE r.id = $1"#,
            relacion_id
        )
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Relación no encontrada".into()))?;

        let mut ids = Vec::new();
        if relacion.sample_fuente_id.is_none() {
            if let Some(youtube_id) = relacion.fuente_youtube_id.as_deref() {
                let (inicio, fin) = timing_range(&relacion.timings_fuente);
                ids.push(
                    Self::encolar_lado(pool, &relacion, "fuente", youtube_id, inicio, fin).await?,
                );
            }
        }
        if relacion.sample_destino_id.is_none() {
            if let Some(youtube_id) = relacion.destino_youtube_id.as_deref() {
                let (inicio, fin) = timing_range(&relacion.timings_destino);
                ids.push(
                    Self::encolar_lado(pool, &relacion, "destino", youtube_id, inicio, fin).await?,
                );
            }
        }
        Ok(ids)
    }

    async fn encolar_lado(
        pool: &PgPool,
        relacion: &RelacionRecorteRow,
        lado: &str,
        youtube_id: &str,
        inicio: f64,
        fin: f64,
    ) -> Result<i32, AppError> {
        let timing_inicio = f64_to_i16(inicio)?;
        let duracion = (fin - inicio).max(1.0);
        // Metadata se liga a una variable para que la macro reciba una expresion simple.
        let metadata = serde_json::json!({
            "origen": "dev_recorte_generar",
            "lado": lado,
            "relacion_id": relacion.id,
            "requested_at": chrono::Utc::now().to_rfc3339(),
        });
        let id = sqlx::query_scalar!(
            "INSERT INTO cola_extraccion_samples (
                     relacion_id, youtube_id, timing_inicio_seg, compas_inicio_seg,
                     compas_fin_seg, duracion_compas_seg, lado, estado, metadata_extraccion
                 ) VALUES ($1, $2, $3, $4::float8, $5::float8, $6::float8, $7, 'pendiente', $8)
                 ON CONFLICT (relacion_id, lado) DO UPDATE SET
                     youtube_id = EXCLUDED.youtube_id,
                     timing_inicio_seg = EXCLUDED.timing_inicio_seg,
                     compas_inicio_seg = EXCLUDED.compas_inicio_seg,
                     compas_fin_seg = EXCLUDED.compas_fin_seg,
                     duracion_compas_seg = EXCLUDED.duracion_compas_seg,
                     estado = 'pendiente',
                     intentos = 0,
                     error_mensaje = NULL,
                     proximo_intento_at = NULL,
                     metadata_extraccion = EXCLUDED.metadata_extraccion
                 RETURNING id",
            relacion.id,
            youtube_id,
            timing_inicio,
            inicio,
            fin,
            duracion,
            lado,
            metadata
        )
        .fetch_one(pool)
        .await?;
        Ok(id)
    }
}

fn timing_range(timings: &Value) -> (f64, f64) {
    let first = timings.as_array().and_then(|items| items.first());
    let inicio = first
        .and_then(|item| item.get("inicio").or_else(|| item.get("start")))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let fin = first
        .and_then(|item| item.get("fin").or_else(|| item.get("end")))
        .and_then(Value::as_f64)
        .filter(|fin| *fin > inicio)
        .unwrap_or(inicio + 30.0);
    (inicio.max(0.0), fin)
}

fn f64_to_i16(value: f64) -> Result<i16, AppError> {
    if !(0.0..=f64::from(i16::MAX)).contains(&value) {
        return Err(AppError::BadRequest("timing fuera de rango".into()));
    }
    format!("{:.0}", value.round())
        .parse::<i16>()
        .map_err(|_| AppError::BadRequest("timing fuera de rango".into()))
}
