/* [254A-8c-refactor] Servicio de extensión de recortes — versión "encolar al scraper".
 *
 * Cambio arquitectónico vs. versión inicial:
 *   - ANTES: el backend Rust descargaba YouTube con yt-dlp y recortaba con ffmpeg.
 *   - PROBLEMA: yt-dlp no está disponible en el contenedor Rust de producción.
 *     Ya existe un scraper Python externo (clients/kamples-scraper/) que es
 *     quien hace la descarga + recorte y luego POSTea a /api/admin/scraper/...
 *   - AHORA: las 3 operaciones solo manipulan `cola_extraccion_samples` para
 *     instruir al scraper qué procesar; no descargan nada.
 *
 * Las 3 operaciones devuelven 202 Accepted con el id de cola programado.
 * El scraper Python lo procesará en su próximo ciclo (controlado por
 * app_config.extraccion_intervalo_seg + extraccion_lote_size — ver tarea 264A-1).
 *
 * Modos guardados en `cola.metadata_extraccion.extension_modo`:
 *   - "extender":      reemplazar audio del sample existente con nuevo rango.
 *   - "generar_siguiente": crear sample nuevo a continuación del actual.
 *   - "restaurar":     reemplazar audio del sample existente con timing original.
 *
 * Pendiente (no bloqueante para esta tarea):
 *   - El pipeline.py debe leer `extension_modo` y comportarse distinto:
 *     * "generar_siguiente" → INSERT samples nuevo + crea fila cola adicional
 *     * "extender" / "restaurar" → reemplaza assets del sample con sample_id
 *   - El "publicador" Rust (equivalente a DevController::publicarExtracciones)
 *     que crea samples desde filas en estado='extraido' aún no existe en este
 *     repo. Es bloqueante para que el flujo end-to-end funcione, pero se trata
 *     como tarea aparte porque también afecta al scraping normal. */

use crate::errors::AppError;
use crate::repositories::{ColaExtraccionRepository, ColaExtraccionRow, EncolarParams};
use sqlx::PgPool;

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct EncoladoResult {
    pub cola_id: i32,
    pub estado: String,
    pub modo: String,
    pub nuevo_inicio: f64,
    pub nuevo_fin: f64,
}

/// Programa una extensión del recorte: el scraper Python ampliará el rango
/// `[inicio - seg_antes, fin + seg_despues]` y reemplazará los assets del sample.
pub async fn extender(
    pool: &PgPool,
    sample_id: i32,
    seg_antes: f64,
    seg_despues: f64,
) -> Result<EncoladoResult, AppError> {
    if seg_antes < 0.0 || seg_despues < 0.0 {
        return Err(AppError::BadRequest(
            "segundos_antes/despues no pueden ser negativos".into(),
        ));
    }

    let cola = require_cola(pool, sample_id).await?;
    let inicio_actual = cola
        .compas_inicio_seg
        .ok_or_else(|| AppError::BadRequest("cola sin compas_inicio_seg".into()))?;
    let fin_actual = cola
        .compas_fin_seg
        .ok_or_else(|| AppError::BadRequest("cola sin compas_fin_seg".into()))?;

    let nuevo_inicio = (inicio_actual - seg_antes).max(0.0);
    let nuevo_fin = fin_actual + seg_despues;
    if nuevo_fin <= nuevo_inicio {
        return Err(AppError::BadRequest(format!(
            "rango inválido: inicio={nuevo_inicio} fin={nuevo_fin}"
        )));
    }

    /* Persistir timing_original en metadata para poder restaurar después. */
    guardar_timing_original_si_falta(pool, sample_id, inicio_actual, fin_actual).await?;

    ColaExtraccionRepository::encolar_para_scraper(
        pool,
        EncolarParams {
            cola_id: cola.id,
            nuevo_inicio,
            nuevo_fin,
            modo: "extender",
        },
    )
    .await?;

    Ok(EncoladoResult {
        cola_id: cola.id,
        estado: "pendiente".into(),
        modo: "extender".into(),
        nuevo_inicio,
        nuevo_fin,
    })
}

/// Programa la generación de un sample nuevo a continuación del actual.
/// El scraper recortará el rango `[fin_actual, fin_actual + duracion_seg]`
/// y creará un sample nuevo vinculado a la misma relación.
pub async fn generar_siguiente(
    pool: &PgPool,
    sample_id: i32,
    duracion_seg: f64,
) -> Result<EncoladoResult, AppError> {
    if duracion_seg <= 0.0 {
        return Err(AppError::BadRequest("duracion debe ser > 0".into()));
    }

    let cola = require_cola(pool, sample_id).await?;
    let fin_actual = cola
        .compas_fin_seg
        .ok_or_else(|| AppError::BadRequest("cola sin compas_fin_seg".into()))?;

    let nuevo_inicio = fin_actual;
    let nuevo_fin = fin_actual + duracion_seg;

    ColaExtraccionRepository::encolar_para_scraper(
        pool,
        EncolarParams {
            cola_id: cola.id,
            nuevo_inicio,
            nuevo_fin,
            modo: "generar_siguiente",
        },
    )
    .await?;

    Ok(EncoladoResult {
        cola_id: cola.id,
        estado: "pendiente".into(),
        modo: "generar_siguiente".into(),
        nuevo_inicio,
        nuevo_fin,
    })
}

/// Programa la restauración del recorte al timing guardado en
/// `samples.metadata.timing_original`.
pub async fn restaurar(pool: &PgPool, sample_id: i32) -> Result<EncoladoResult, AppError> {
    let cola = require_cola(pool, sample_id).await?;

    /* Leer timing_original directamente desde samples.metadata. */
    // "metadata!" fuerza no nulo: COALESCE nunca devuelve NULL.
    let metadata = sqlx::query_scalar!(
        r#"SELECT COALESCE(metadata, '{}'::jsonb) AS "metadata!" FROM samples WHERE id = $1"#,
        sample_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("sample {sample_id} no encontrado")))?;

    let timing = metadata
        .get("timing_original")
        .ok_or_else(|| AppError::BadRequest("metadata sin timing_original".into()))?;
    let inicio = timing
        .get("inicio")
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| AppError::BadRequest("timing_original.inicio inválido".into()))?;
    let fin = timing
        .get("fin")
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| AppError::BadRequest("timing_original.fin inválido".into()))?;
    if fin <= inicio {
        return Err(AppError::BadRequest(
            "timing_original con rango inválido".into(),
        ));
    }

    ColaExtraccionRepository::encolar_para_scraper(
        pool,
        EncolarParams {
            cola_id: cola.id,
            nuevo_inicio: inicio,
            nuevo_fin: fin,
            modo: "restaurar",
        },
    )
    .await?;

    Ok(EncoladoResult {
        cola_id: cola.id,
        estado: "pendiente".into(),
        modo: "restaurar".into(),
        nuevo_inicio: inicio,
        nuevo_fin: fin,
    })
}

/* ---------- helpers ---------- */

async fn require_cola(pool: &PgPool, sample_id: i32) -> Result<ColaExtraccionRow, AppError> {
    ColaExtraccionRepository::find_by_sample_id(pool, sample_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(format!(
                "sample {sample_id} no tiene fila en cola_extraccion_samples"
            ))
        })
}

async fn guardar_timing_original_si_falta(
    pool: &PgPool,
    sample_id: i32,
    inicio: f64,
    fin: f64,
) -> Result<(), AppError> {
    /* Solo escribe si timing_original aún no existe. */
    let patch = serde_json::json!({
        "timing_original": { "inicio": inicio, "fin": fin }
    });

    // El cast fija $2 a jsonb: sin él Postgres podría inferir text para el operador ||.
    sqlx::query!(
        "UPDATE samples
         SET metadata = COALESCE(metadata, '{}'::jsonb) || $2::jsonb
         WHERE id = $1
           AND NOT (COALESCE(metadata, '{}'::jsonb) ? 'timing_original')",
        sample_id,
        patch
    )
    .execute(pool)
    .await?;

    Ok(())
}
