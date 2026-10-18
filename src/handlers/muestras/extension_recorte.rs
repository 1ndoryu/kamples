/* [254A-8c-refactor] Handlers admin para extensión de recortes.
 *
 * Las 3 operaciones encolan trabajo para el scraper Python (kamples-scraper)
 * vía cola_extraccion_samples; devuelven 202 Accepted con el id de cola
 * programado y el modo. NO realizan descarga ni recorte aquí.
 *
 * - POST /api/samples/:id/extender-recorte
 * - POST /api/samples/:id/generar-siguiente
 * - POST /api/samples/:id/restaurar-recorte
 *
 * Solo admin. */

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;

use crate::errors::AppError;
#[allow(unused_imports)]
use crate::errors::ErrorResponse;
use crate::middleware::CurrentUser;
use crate::services::extension_recorte::{self, EncoladoResult};
use crate::AppState;

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ExtenderRecorteRequest {
    /// Segundos a añadir ANTES del inicio actual (>= 0).
    pub segundos_antes: f64,
    /// Segundos a añadir DESPUÉS del fin actual (>= 0).
    pub segundos_despues: f64,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct GenerarSiguienteRequest {
    /// Duración en segundos del nuevo segmento (> 0).
    pub duracion: f64,
}

fn ok202(result: EncoladoResult) -> impl IntoResponse {
    (StatusCode::ACCEPTED, Json(result))
}

#[utoipa::path(
    post,
    path = "/api/samples/{id}/extender-recorte",
    params(("id" = i32, Path, description = "ID del sample")),
    request_body = ExtenderRecorteRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 202, description = "Encolado para el scraper", body = EncoladoResult),
        (status = 400, description = "Sin cola_extraccion o rango inválido", body = ErrorResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Sin permisos de admin", body = ErrorResponse)
    ),
    tag = "admin-samples"
)]
pub async fn extender_recorte(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i32>,
    Json(req): Json<ExtenderRecorteRequest>,
) -> Result<axum::response::Response, AppError> {
    user.require_admin()?;
    let result =
        extension_recorte::extender(&state.pool, id, req.segundos_antes, req.segundos_despues)
            .await?;
    Ok(ok202(result).into_response())
}

#[utoipa::path(
    post,
    path = "/api/samples/{id}/generar-siguiente",
    params(("id" = i32, Path, description = "ID del sample origen")),
    request_body = GenerarSiguienteRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 202, description = "Encolado para el scraper", body = EncoladoResult),
        (status = 400, description = "Sin cola_extraccion o duración inválida", body = ErrorResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Sin permisos de admin", body = ErrorResponse)
    ),
    tag = "admin-samples"
)]
pub async fn generar_siguiente(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i32>,
    Json(req): Json<GenerarSiguienteRequest>,
) -> Result<axum::response::Response, AppError> {
    user.require_admin()?;
    let result = extension_recorte::generar_siguiente(&state.pool, id, req.duracion).await?;
    Ok(ok202(result).into_response())
}

#[utoipa::path(
    post,
    path = "/api/samples/{id}/restaurar-recorte",
    params(("id" = i32, Path, description = "ID del sample")),
    security(("bearer_auth" = [])),
    responses(
        (status = 202, description = "Encolado para el scraper", body = EncoladoResult),
        (status = 400, description = "metadata sin timing_original", body = ErrorResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Sin permisos de admin", body = ErrorResponse)
    ),
    tag = "admin-samples"
)]
pub async fn restaurar_recorte(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i32>,
) -> Result<axum::response::Response, AppError> {
    user.require_admin()?;
    let result = extension_recorte::restaurar(&state.pool, id).await?;
    Ok(ok202(result).into_response())
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/samples/:id/extender-recorte", post(extender_recorte))
        .route(
            "/api/samples/:id/generar-siguiente",
            post(generar_siguiente),
        )
        .route(
            "/api/samples/:id/restaurar-recorte",
            post(restaurar_recorte),
        )
}
