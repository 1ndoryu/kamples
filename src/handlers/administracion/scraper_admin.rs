/* [174A-108b] Endpoints para el scraper Python (clients/kamples-scraper/).
 *
 * Reemplaza a los endpoints WordPress legados:
 *   - POST /wp-json/kamples/v1/dev/extraccion/publicar-auto
 *   - POST /wp-json/kamples/v1/admin/automatizacion/reporte-lote
 *
 * Auth: header `X-Kamples-Secret` comparado constant-time contra
 * `AppState.scraper_secret` (env var `SCRAPER_SECRET`).
 *
 * Si `scraper_secret` no está configurado, los endpoints responden
 * 403 Forbidden — protege contra exposición accidental.
 */

use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

use crate::errors::AppError;
use crate::services::extraccion_publisher::{ExtraccionPublisherService, PublicarResultado};
use crate::services::{AdminAutomationService, AutomationBatchReport};
use crate::AppState;

const HEADER_NAME: &str = "x-kamples-secret";
const LIMITE_POR_LOTE_PUBLICACION: i64 = 50;

/// Resultado de la operación `publicar-auto`.
#[derive(Debug, Serialize, ToSchema)]
pub struct PublicarAutoResponse {
    /// Cantidad de items de `cola_extraccion_samples` que se publicaron como
    /// samples nuevos en esta llamada.
    pub publicados: usize,
    /// Cantidad de items cuyos assets se reemplazaron sobre un sample existente
    /// (modos `extender` / `restaurar`).
    pub reemplazados: usize,
    /// Cantidad de items que fallaron y quedaron disponibles para reintento.
    pub errores: usize,
    /// Detalle por item.
    pub detalle: PublicarResultado,
}

/// Payload del reporte de un lote de scraping.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ReporteLoteRequest {
    pub batch_id: i64,
    pub exitosos: i64,
    pub fallidos: i64,
    #[serde(default)]
    pub recortes: Option<i64>,
    #[serde(default)]
    pub samples_publicados: Option<i64>,
    #[serde(default)]
    pub canciones_nuevas: Option<i64>,
    #[serde(default)]
    pub sampleos_nuevos: Option<i64>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// Resultado del reporte de lote.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReporteLoteResponse {
    pub ok: bool,
    pub batch_id: i64,
}

/* Validación constant-time del header `X-Kamples-Secret`. */
fn require_scraper_secret(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    let Some(expected) = state.scraper_secret.as_deref() else {
        return Err(AppError::Forbidden(
            "scraper endpoints disabled: SCRAPER_SECRET not configured".into(),
        ));
    };
    let provided = headers
        .get(HEADER_NAME)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    /* Comparación constant-time para evitar timing attacks. */
    let provided_bytes = provided.as_bytes();
    let expected_bytes = expected.as_bytes();
    if provided_bytes.len() != expected_bytes.len() {
        return Err(AppError::Unauthorized);
    }
    let mut diff: u8 = 0;
    for (a, b) in provided_bytes.iter().zip(expected_bytes.iter()) {
        diff |= a ^ b;
    }
    if diff == 0 {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}

/// Marca como `completado` los items en estado `extraido` con `sample_id`
/// asignado. Equivale a la "publicación automática" del legado WordPress.
#[utoipa::path(
    post,
    path = "/api/admin/scraper/publicar-auto",
    tag = "scraper",
    responses(
        (status = 200, description = "Items publicados", body = PublicarAutoResponse),
        (status = 401, description = "Secret inválido"),
        (status = 403, description = "SCRAPER_SECRET no configurado")
    )
)]
pub async fn publicar_auto(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PublicarAutoResponse>, AppError> {
    require_scraper_secret(&state, &headers)?;

    /* [264A-3] Antes este endpoint era un no-op: solo marcaba estado='completado'
     * para filas que ya tenian sample_id, pero NADIE asignaba sample_id, asi que
     * jamas publicaba nada. Ahora delega en ExtraccionPublisherService que crea
     * el sample real (o reemplaza assets en modo extender/restaurar). */
    let detalle = ExtraccionPublisherService::publicar_pendientes(
        &state.pool,
        &state.storage,
        LIMITE_POR_LOTE_PUBLICACION,
    )
    .await?;

    tracing::info!(
        publicados = detalle.publicados,
        reemplazados = detalle.reemplazados,
        errores = detalle.errores,
        "scraper publicar-auto ejecutado",
    );

    Ok(Json(PublicarAutoResponse {
        publicados: detalle.publicados,
        reemplazados: detalle.reemplazados,
        errores: detalle.errores,
        detalle,
    }))
}

/// Recibe el reporte resumen de un lote del scraper/extractor y lo persiste
/// en `lotes_procesamiento`, actualizando tambien el auto-stop si aplica.
#[utoipa::path(
    post,
    path = "/api/admin/scraper/reporte-lote",
    tag = "scraper",
    request_body = ReporteLoteRequest,
    responses(
        (status = 200, description = "Reporte aceptado", body = ReporteLoteResponse),
        (status = 401, description = "Secret inválido"),
        (status = 403, description = "SCRAPER_SECRET no configurado")
    )
)]
pub async fn reporte_lote(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ReporteLoteRequest>,
) -> Result<Json<ReporteLoteResponse>, AppError> {
    require_scraper_secret(&state, &headers)?;

    let motivos: HashMap<String, serde_json::Value> = payload
        .metadata
        .as_ref()
        .and_then(|m| m.get("motivos_fallo"))
        .and_then(|m| m.as_object())
        .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();

    tracing::info!(
        batch_id = payload.batch_id,
        exitosos = payload.exitosos,
        fallidos = payload.fallidos,
        recortes = payload.recortes.unwrap_or(payload.exitosos),
        motivos_fallo = ?motivos,
        "scraper reporte-lote recibido",
    );

    AdminAutomationService::record_batch_report(
        &state.pool,
        AutomationBatchReport {
            batch_id: payload.batch_id,
            exitosos: payload.exitosos,
            fallidos: payload.fallidos,
            recortes: payload.recortes,
            samples_publicados: payload.samples_publicados,
            canciones_nuevas: payload.canciones_nuevas,
            sampleos_nuevos: payload.sampleos_nuevos,
            metadata: payload.metadata,
        },
    )
    .await?;

    Ok(Json(ReporteLoteResponse {
        ok: true,
        batch_id: payload.batch_id,
    }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/scraper/publicar-auto", post(publicar_auto))
        .route("/admin/scraper/reporte-lote", post(reporte_lote))
}
