use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};

use crate::errors::AppError;
#[allow(unused_imports)]
use crate::errors::ErrorResponse;
use crate::models::{
    GlobalSearchQuery, GlobalSearchResponse, LegacyQuickSearchQuery, LegacyQuickSearchResponse,
};
use crate::services::SearchService;
use crate::AppState;

#[utoipa::path(
    get,
    path = "/api/search",
    tag = "search",
    params(
        ("q" = String, Query, description = "Texto a buscar. Con menos de 2 caracteres devuelve listas vacías"),
        ("types" = Option<String>, Query, description = "CSV opcional para filtrar tipos: samples,users,collections,songs")
    ),
    responses(
        (status = 200, description = "Búsqueda global agrupada por tipo", body = GlobalSearchResponse),
        (status = 422, description = "Parámetros inválidos", body = ErrorResponse)
    )
)]
pub async fn global_search(
    State(state): State<AppState>,
    Query(query): Query<GlobalSearchQuery>,
) -> Result<Json<GlobalSearchResponse>, AppError> {
    let response =
        SearchService::global_search(&state.pool, state.public_base_url.as_deref(), query).await?;
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/busqueda/rapida",
    tag = "search",
    params(
        ("q" = String, Query, description = "Texto a buscar. Mantiene el contrato legacy del dropdown rápido")
    ),
    responses(
        (status = 200, description = "Respuesta compatible con el endpoint legacy /busqueda/rapida", body = LegacyQuickSearchResponse),
        (status = 422, description = "Parámetros inválidos", body = ErrorResponse)
    )
)]
pub async fn legacy_quick_search(
    State(state): State<AppState>,
    Query(query): Query<LegacyQuickSearchQuery>,
) -> Result<Json<LegacyQuickSearchResponse>, AppError> {
    let response =
        SearchService::legacy_quick_search(&state.pool, state.public_base_url.as_deref(), query)
            .await?;
    Ok(Json(response))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/search", get(global_search))
        .route("/busqueda/rapida", get(legacy_quick_search))
}
