/* [254A-4] Endpoints admin/duplicados — paridad con DuplicadosController.php legacy.
 * GET /admin/duplicados?estado=&tipo=&pagina=&porPagina= — listar duplicados con joins a samples
 * GET /admin/duplicados/contar — contador de pendientes (badge nav)
 * POST /admin/duplicados/backfill — backfill manual de audio_hash exacto
 *
 * Devuelve los samples original/duplicado con titulo, slug, creador, ruta_preview, audio_hash.
 * El frontend usa estos campos para mostrar el preview de audio y agrupar por hash.
 */

use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::errors::AppError;
use crate::middleware::CurrentUser;
use crate::repositories::{AdminDuplicatesRepository, DuplicateAdminRow};
use crate::services::AdminDuplicatesService;
use crate::AppState;

pub use crate::services::admin_duplicates::BackfillStats;

const ESTADOS_VALIDOS: [&str; 4] = ["pendiente", "aprobado", "rechazado", "fusionado"];
const TIPOS_VALIDOS: [&str; 3] = ["cross_usuario", "mismo_usuario", "backfill"];

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DuplicadoFila {
    pub id: i32,
    pub tipo: String,
    pub estado: String,
    pub created_at: DateTime<Utc>,

    pub original_id: i32,
    pub original_titulo: String,
    pub original_subido_at: DateTime<Utc>,
    pub original_ruta_preview: Option<String>,
    pub original_ruta_waveform: Option<String>,
    pub original_creador: String,
    pub original_creador_id: i32,
    pub original_slug: Option<String>,
    pub original_hash: Option<String>,

    pub duplicado_id: i32,
    pub duplicado_titulo: String,
    pub duplicado_subido_at: DateTime<Utc>,
    pub duplicado_ruta_preview: Option<String>,
    pub duplicado_ruta_waveform: Option<String>,
    pub duplicado_creador: String,
    pub duplicado_creador_id: i32,
    pub duplicado_slug: Option<String>,
    pub duplicado_hash: Option<String>,
}

impl From<DuplicateAdminRow> for DuplicadoFila {
    fn from(row: DuplicateAdminRow) -> Self {
        Self {
            id: row.id,
            tipo: row.tipo,
            estado: row.estado,
            created_at: row.created_at,
            original_id: row.original_id,
            original_titulo: row.original_titulo,
            original_subido_at: row.original_subido_at,
            original_ruta_preview: row.original_ruta_preview,
            original_ruta_waveform: row.original_ruta_waveform,
            original_creador: row.original_creador,
            original_creador_id: row.original_creador_id,
            original_slug: row.original_slug,
            original_hash: row.original_hash,
            duplicado_id: row.duplicado_id,
            duplicado_titulo: row.duplicado_titulo,
            duplicado_subido_at: row.duplicado_subido_at,
            duplicado_ruta_preview: row.duplicado_ruta_preview,
            duplicado_ruta_waveform: row.duplicado_ruta_waveform,
            duplicado_creador: row.duplicado_creador,
            duplicado_creador_id: row.duplicado_creador_id,
            duplicado_slug: row.duplicado_slug,
            duplicado_hash: row.duplicado_hash,
        }
    }
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ListarQuery {
    #[serde(default)]
    pub estado: Option<String>,
    #[serde(default)]
    pub tipo: Option<String>,
    #[serde(default)]
    pub pagina: Option<i64>,
    #[serde(default, rename = "porPagina")]
    pub por_pagina: Option<i64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AdminDuplicadosListarResponse {
    pub ok: bool,
    pub total: i64,
    pub pagina: i64,
    #[serde(rename = "porPagina")]
    pub por_pagina: i64,
    pub duplicados: Vec<DuplicadoFila>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ContarResponse {
    pub ok: bool,
    pub total: i64,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct BackfillDuplicadosRequest {
    pub batch: Option<i64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BackfillDuplicadosResponse {
    pub ok: bool,
    pub stats: BackfillStats,
}

#[utoipa::path(get, path = "/api/admin/duplicados", tag = "admin",
    params(ListarQuery),
    security(("bearer_auth" = [])),
    responses((status = 200, body = AdminDuplicadosListarResponse), (status = 403)))]
pub async fn listar(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<ListarQuery>,
) -> Result<Json<AdminDuplicadosListarResponse>, AppError> {
    user.require_admin()?;
    let pagina = query.pagina.unwrap_or(1).max(1);
    let por_pagina = query.por_pagina.unwrap_or(20).clamp(1, 50);
    let offset = (pagina - 1) * por_pagina;

    let estado = query
        .estado
        .as_deref()
        .filter(|e| ESTADOS_VALIDOS.contains(e))
        .unwrap_or("pendiente")
        .to_string();
    let tipo = match query.tipo.as_deref() {
        Some(t) if TIPOS_VALIDOS.contains(&t) => Some(t.to_string()),
        Some("") | None => None,
        Some(other) => {
            return Err(AppError::BadRequest(format!(
                "tipo invalido: '{other}'. Validos: cross_usuario, mismo_usuario, backfill"
            )))
        }
    };

    let duplicados =
        AdminDuplicatesRepository::list(&state.pool, &estado, tipo.as_deref(), por_pagina, offset)
            .await?
            .into_iter()
            .map(DuplicadoFila::from)
            .collect();

    let total = AdminDuplicatesRepository::count_pending(&state.pool).await?;

    Ok(Json(AdminDuplicadosListarResponse {
        ok: true,
        total,
        pagina,
        por_pagina,
        duplicados,
    }))
}

#[utoipa::path(get, path = "/api/admin/duplicados/contar", tag = "admin",
    security(("bearer_auth" = [])),
    responses((status = 200, body = ContarResponse), (status = 403)))]
pub async fn contar(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<ContarResponse>, AppError> {
    user.require_admin()?;
    let total = AdminDuplicatesRepository::count_pending(&state.pool).await?;
    Ok(Json(ContarResponse { ok: true, total }))
}

#[utoipa::path(post, path = "/api/admin/duplicados/backfill", tag = "admin",
    request_body = BackfillDuplicadosRequest,
    security(("bearer_auth" = [])),
    responses((status = 200, body = BackfillDuplicadosResponse), (status = 403)))]
pub async fn backfill(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(payload): Json<BackfillDuplicadosRequest>,
) -> Result<Json<BackfillDuplicadosResponse>, AppError> {
    user.require_admin()?;
    let requested_batch = payload.batch.unwrap_or(100).clamp(10, 500);
    let backfill_stats =
        AdminDuplicatesService::run_hash_backfill(&state.pool, requested_batch).await?;

    Ok(Json(BackfillDuplicadosResponse {
        ok: true,
        stats: backfill_stats,
    }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/duplicados", get(listar))
        .route("/admin/duplicados/contar", get(contar))
        .route("/admin/duplicados/backfill", post(backfill))
}
