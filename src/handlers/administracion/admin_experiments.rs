use std::collections::BTreeMap;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

use crate::errors::AppError;
#[allow(unused_imports)]
use crate::errors::ErrorResponse;
use crate::middleware::CurrentUser;
use crate::services::AdminExperimentsService;
use crate::AppState;

#[derive(Debug, Deserialize, ToSchema)]
pub struct GenerarExperimentoRequest {
    pub acciones: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BenchmarkRequest {
    #[serde(rename = "userId")]
    pub user_id: Option<i32>,
    #[serde(rename = "perPage")]
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExperimentoResponse {
    pub ok: bool,
    #[schema(value_type = Object)]
    pub data: BTreeMap<String, Value>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EmbeddingsResponse {
    pub ok: bool,
    pub actualizados: i64,
    #[serde(rename = "tiempoMs")]
    pub tiempo_ms: i64,
    pub mensaje: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BenchmarkResponse {
    pub ok: bool,
    pub output: String,
    pub stderr: String,
    #[serde(rename = "exitCode")]
    pub exit_code: i32,
    pub error: Option<String>,
}

#[utoipa::path(
    post,
    path = "/api/admin/experimentos/generar",
    tag = "admin",
    request_body = GenerarExperimentoRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Experimento social generado", body = ExperimentoResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Requiere admin", body = ErrorResponse)
    )
)]
pub async fn generar_experimento(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(req): Json<GenerarExperimentoRequest>,
) -> Result<Json<ExperimentoResponse>, AppError> {
    user.require_admin()?;
    let data =
        AdminExperimentsService::generate_experiment(&state.pool, user.user_id, req.acciones)
            .await?;
    Ok(Json(ExperimentoResponse { ok: true, data }))
}

#[utoipa::path(
    post,
    path = "/api/admin/embeddings/generar",
    tag = "admin",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Embeddings faltantes generados", body = EmbeddingsResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Requiere admin", body = ErrorResponse)
    )
)]
pub async fn generar_embeddings(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<EmbeddingsResponse>, AppError> {
    user.require_admin()?;
    let result = AdminExperimentsService::generate_embeddings(&state.pool, false).await?;
    Ok(Json(EmbeddingsResponse {
        ok: true,
        actualizados: result.actualizados,
        tiempo_ms: result.tiempo_ms,
        mensaje: result.mensaje,
    }))
}

#[utoipa::path(
    post,
    path = "/api/admin/embeddings/regenerar",
    tag = "admin",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Todos los embeddings regenerados", body = EmbeddingsResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Requiere admin", body = ErrorResponse)
    )
)]
pub async fn regenerar_embeddings(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<EmbeddingsResponse>, AppError> {
    user.require_admin()?;
    let result = AdminExperimentsService::generate_embeddings(&state.pool, true).await?;
    Ok(Json(EmbeddingsResponse {
        ok: true,
        actualizados: result.actualizados,
        tiempo_ms: result.tiempo_ms,
        mensaje: result.mensaje,
    }))
}

#[utoipa::path(
    post,
    path = "/api/admin/procesos/benchmark",
    tag = "admin",
    request_body = BenchmarkRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Benchmark del algoritmo", body = BenchmarkResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Requiere admin", body = ErrorResponse)
    )
)]
pub async fn benchmark(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(req): Json<BenchmarkRequest>,
) -> Result<Json<BenchmarkResponse>, AppError> {
    user.require_admin()?;
    let result = AdminExperimentsService::run_benchmark(
        &state.pool,
        req.user_id.unwrap_or(user.user_id),
        req.per_page.unwrap_or(30),
    )
    .await?;
    Ok(Json(BenchmarkResponse {
        ok: result.exit_code == 0,
        output: result.output,
        stderr: result.stderr,
        exit_code: result.exit_code,
        error: None,
    }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/experimentos/generar", post(generar_experimento))
        .route("/admin/embeddings/generar", post(generar_embeddings))
        .route("/admin/embeddings/regenerar", post(regenerar_embeddings))
        .route("/admin/procesos/benchmark", post(benchmark))
}
