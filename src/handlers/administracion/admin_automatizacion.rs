/* [254A-4] Endpoints admin/automatizacion — paridad con AutomatizacionController.php legacy.
 * GET /admin/automatizacion/estado    — estado general (extraccion + scraping) con ultimo lote.
 * GET /admin/automatizacion/historial — historial paginado de lotes_procesamiento.
 * POST /admin/automatizacion/reactivar — vuelve a activar un proceso auto-detenido.
 *
 * Gotchas:
 *  - PHP usaba options WP; Rust usa app_config (`*_enabled`, intervalos y lote_size)
 *    porque el scraper lee esa tabla como contrato runtime compartido.
 *  - El frontend (apiAutomatizacion.ts) consume `{ ok, estado: { extraccion, scraping } }` y
 *    `{ ok, items, total, pagina }`. Esquema replicado tal cual.
 */

use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::errors::AppError;
use crate::middleware::CurrentUser;
use crate::repositories::AutomationBatchRow;
use crate::services::{admin_automation, AdminAutomationService};
use crate::AppState;

const TIPOS_VALIDOS: [&str; 2] = ["extraccion", "scraping"];
const HISTORIAL_PAGE_SIZE: i64 = 20;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LoteResumen {
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

impl From<AutomationBatchRow> for LoteResumen {
    fn from(row: AutomationBatchRow) -> Self {
        Self {
            id: row.id,
            tipo: row.tipo,
            estado: row.estado,
            iniciado_at: row.iniciado_at,
            completado_at: row.completado_at,
            exitosos: row.exitosos,
            fallidos: row.fallidos,
            recortes: row.recortes,
            samples_publicados: row.samples_publicados,
            canciones_nuevas: row.canciones_nuevas,
            sampleos_nuevos: row.sampleos_nuevos,
            error_mensaje: row.error_mensaje,
            metadata: row.metadata,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct EstadoTipo {
    pub activo: bool,
    pub limite_por_lote: i32,
    pub intervalo_segundos: i32,
    pub fallos_consecutivos: i32,
    pub ultimo_lote: Option<LoteResumen>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct EstadoAutomatizacion {
    pub extraccion: EstadoTipo,
    pub scraping: EstadoTipo,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct EstadoResponse {
    pub ok: bool,
    pub estado: EstadoAutomatizacion,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct HistorialQuery {
    #[serde(default)]
    pub tipo: Option<String>,
    #[serde(default)]
    pub pagina: Option<i64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AdminAutomatizacionHistorialResponse {
    pub ok: bool,
    pub items: Vec<LoteResumen>,
    pub total: i64,
    pub pagina: i64,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ReactivarRequest {
    pub tipo: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReactivarResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mensaje: Option<String>,
}

#[utoipa::path(get, path = "/api/admin/automatizacion/estado", tag = "admin",
    security(("bearer_auth" = [])),
    responses((status = 200, body = EstadoResponse), (status = 403)))]
pub async fn estado(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<EstadoResponse>, AppError> {
    user.require_admin()?;
    let current_status = AdminAutomationService::status(&state.pool).await?;
    Ok(Json(EstadoResponse {
        ok: true,
        estado: map_status(current_status),
    }))
}

#[utoipa::path(get, path = "/api/admin/automatizacion/historial", tag = "admin",
    params(HistorialQuery),
    security(("bearer_auth" = [])),
    responses((status = 200, body = AdminAutomatizacionHistorialResponse), (status = 403)))]
pub async fn historial(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<HistorialQuery>,
) -> Result<Json<AdminAutomatizacionHistorialResponse>, AppError> {
    user.require_admin()?;
    let tipo_filtro = match query.tipo.as_deref() {
        Some(t) if TIPOS_VALIDOS.contains(&t) => Some(t.to_string()),
        Some("") | None => None,
        Some(other) => {
            return Err(AppError::BadRequest(format!(
                "tipo invalido: '{other}'. Validos: extraccion, scraping"
            )))
        }
    };
    let page = query.pagina.unwrap_or(1).max(1);
    let history = AdminAutomationService::history(
        &state.pool,
        tipo_filtro.as_deref(),
        page,
        HISTORIAL_PAGE_SIZE,
    )
    .await?;

    Ok(Json(AdminAutomatizacionHistorialResponse {
        ok: true,
        items: history.items.into_iter().map(LoteResumen::from).collect(),
        total: history.total,
        pagina: history.pagina,
    }))
}

#[utoipa::path(post, path = "/api/admin/automatizacion/reactivar", tag = "admin",
    request_body = ReactivarRequest,
    security(("bearer_auth" = [])),
    responses((status = 200, body = ReactivarResponse), (status = 400), (status = 403)))]
pub async fn reactivar(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(payload): Json<ReactivarRequest>,
) -> Result<Json<ReactivarResponse>, AppError> {
    user.require_admin()?;
    let outcome = AdminAutomationService::reactivate(&state.pool, &payload.tipo).await?;

    Ok(Json(ReactivarResponse {
        ok: true,
        mensaje: Some(outcome.mensaje),
    }))
}

fn map_status(status: admin_automation::AutomationStatus) -> EstadoAutomatizacion {
    EstadoAutomatizacion {
        extraccion: map_type_status(status.extraccion),
        scraping: map_type_status(status.scraping),
    }
}

fn map_type_status(status: admin_automation::AutomationTypeStatus) -> EstadoTipo {
    EstadoTipo {
        activo: status.activo,
        limite_por_lote: status.limite_por_lote,
        intervalo_segundos: status.intervalo_segundos,
        fallos_consecutivos: status.fallos_consecutivos,
        ultimo_lote: status.ultimo_lote.map(LoteResumen::from),
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/automatizacion/estado", get(estado))
        .route("/admin/automatizacion/historial", get(historial))
        .route("/admin/automatizacion/reactivar", post(reactivar))
}
