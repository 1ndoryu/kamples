use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::errors::AppError;
#[allow(unused_imports)]
use crate::errors::ErrorResponse;
use crate::middleware::CurrentUser;
use crate::models::{
    AdminActivityQuery, AdminActivityResponse, AdminExtractionQueueQuery,
    AdminExtractionQueueResponse, AdminOkResponse, AdminProcessCookiesRequest,
    AdminProcessStartRequest, AdminProcessesResponse, AdminScrapersQuery, AdminScrapersResponse,
    AdminSummaryStats, AdminUserDeleteRequest, AdminUserSuspendRequest, AdminUserUpdateRequest,
    AdminUsersQuery, AdminUsersResponse, DeleteUserRequest, SuspendUserRequest,
};
use crate::repositories::{AdminPanelRepository, ModerationRepository};
use crate::services::algo_timing::{TimingEntry, ALGO_TIMING};
use crate::services::{AdminProcessService, AdminSamplesService};
use crate::AppState;

/* [174A-25] Endpoints admin: requiere rol admin (validado via require_admin). */

#[utoipa::path(post, path = "/api/admin/users/{id}/suspend",
    params(("id" = i32, Path, description = "User id")),
    request_body = SuspendUserRequest,
    security(("bearer_auth" = [])),
    responses((status = 204, description = "Suspendido"), (status = 401, description = "no auth"), (status = 403, description = "no admin"), (status = 404, description = "no encontrado")))]
pub async fn suspend(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i32>,
    Json(req): Json<SuspendUserRequest>,
) -> Result<StatusCode, AppError> {
    user.require_admin()?;
    req.validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;
    ModerationRepository::suspend(&state.pool, id, &req.razon, req.hasta).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/admin/users/{id}/activate",
    params(("id" = i32, Path, description = "User id")),
    security(("bearer_auth" = [])),
    responses((status = 204, description = "Reactivado"), (status = 401, description = "no auth"), (status = 403, description = "no admin"), (status = 404, description = "no encontrado")))]
pub async fn activate(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i32>,
) -> Result<StatusCode, AppError> {
    user.require_admin()?;
    ModerationRepository::activate(&state.pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(post, path = "/api/admin/users/{id}/delete",
    params(("id" = i32, Path, description = "User id")),
    request_body = DeleteUserRequest,
    security(("bearer_auth" = [])),
    responses((status = 204, description = "Marcado para eliminacion"), (status = 401, description = "no auth"), (status = 403, description = "no admin"), (status = 404, description = "no encontrado")))]
pub async fn mark_delete(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i32>,
    Json(req): Json<DeleteUserRequest>,
) -> Result<StatusCode, AppError> {
    user.require_admin()?;
    let dias = req.dias_gracia.unwrap_or(30).clamp(1, 365);
    ModerationRepository::mark_for_deletion(&state.pool, id, dias).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/admin/resumen",
    tag = "admin",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "KPIs del panel admin", body = AdminSummaryStats),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Requiere admin", body = ErrorResponse)
    )
)]
pub async fn summary(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<AdminSummaryStats>, AppError> {
    user.require_admin()?;
    Ok(Json(AdminPanelRepository::summary(&state.pool).await?))
}

#[utoipa::path(
    get,
    path = "/api/admin/actividad",
    tag = "admin",
    params(AdminActivityQuery),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Actividad diaria del panel admin", body = AdminActivityResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Requiere admin", body = ErrorResponse)
    )
)]
pub async fn activity(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<AdminActivityQuery>,
) -> Result<Json<AdminActivityResponse>, AppError> {
    user.require_admin()?;
    Ok(Json(
        AdminPanelRepository::activity(&state.pool, &query).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/admin/usuarios",
    tag = "admin",
    params(AdminUsersQuery),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Listado admin de usuarios", body = AdminUsersResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Requiere admin", body = ErrorResponse)
    )
)]
pub async fn list_users(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<AdminUsersQuery>,
) -> Result<Json<AdminUsersResponse>, AppError> {
    user.require_admin()?;
    Ok(Json(
        AdminPanelRepository::list_users(&state.pool, &query).await?,
    ))
}

#[utoipa::path(
    put,
    path = "/api/admin/usuarios/{id}",
    tag = "admin",
    params(("id" = i32, Path, description = "User id")),
    request_body = AdminUserUpdateRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Usuario actualizado", body = AdminOkResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Requiere admin", body = ErrorResponse),
        (status = 404, description = "Usuario no encontrado", body = ErrorResponse),
        (status = 422, description = "Payload inválido", body = ErrorResponse)
    )
)]
pub async fn update_user_legacy(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i32>,
    Json(request): Json<AdminUserUpdateRequest>,
) -> Result<Json<AdminOkResponse>, AppError> {
    user.require_admin()?;
    let updated =
        AdminPanelRepository::update_user(&state.pool, user.user_id, id, &request).await?;
    if !updated {
        return Err(AppError::NotFound(format!("usuario {id}")));
    }
    Ok(Json(AdminOkResponse { ok: true }))
}

#[utoipa::path(
    post,
    path = "/api/admin/usuarios/{id}/suspender",
    tag = "admin",
    params(("id" = i32, Path, description = "User id")),
    request_body = AdminUserSuspendRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Usuario suspendido", body = AdminOkResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Requiere admin", body = ErrorResponse),
        (status = 404, description = "Usuario no encontrado", body = ErrorResponse),
        (status = 422, description = "Payload inválido", body = ErrorResponse)
    )
)]
pub async fn suspend_user_legacy(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i32>,
    Json(request): Json<AdminUserSuspendRequest>,
) -> Result<Json<AdminOkResponse>, AppError> {
    user.require_admin()?;
    request
        .validate()
        .map_err(|error| AppError::Validation(error.to_string()))?;
    let updated = AdminPanelRepository::suspend_user(&state.pool, id, &request).await?;
    if !updated {
        return Err(AppError::NotFound(format!("usuario {id}")));
    }
    Ok(Json(AdminOkResponse { ok: true }))
}

#[utoipa::path(
    post,
    path = "/api/admin/usuarios/{id}/desuspender",
    tag = "admin",
    params(("id" = i32, Path, description = "User id")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Usuario desuspendido", body = AdminOkResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Requiere admin", body = ErrorResponse),
        (status = 404, description = "Usuario no encontrado", body = ErrorResponse)
    )
)]
pub async fn unsuspend_user_legacy(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i32>,
) -> Result<Json<AdminOkResponse>, AppError> {
    user.require_admin()?;
    let updated = AdminPanelRepository::unsuspend_user(&state.pool, id).await?;
    if !updated {
        return Err(AppError::NotFound(format!("usuario {id}")));
    }
    Ok(Json(AdminOkResponse { ok: true }))
}

#[utoipa::path(
    post,
    path = "/api/admin/usuarios/{id}/eliminar",
    tag = "admin",
    params(("id" = i32, Path, description = "User id")),
    request_body = AdminUserDeleteRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Usuario marcado para eliminación", body = AdminOkResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Requiere admin", body = ErrorResponse),
        (status = 404, description = "Usuario no encontrado", body = ErrorResponse),
        (status = 422, description = "Payload inválido", body = ErrorResponse)
    )
)]
pub async fn mark_delete_legacy(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i32>,
    Json(request): Json<AdminUserDeleteRequest>,
) -> Result<Json<AdminOkResponse>, AppError> {
    user.require_admin()?;
    request
        .validate()
        .map_err(|error| AppError::Validation(error.to_string()))?;
    let updated = AdminPanelRepository::mark_user_for_deletion(&state.pool, id).await?;
    if !updated {
        return Err(AppError::NotFound(format!("usuario {id}")));
    }
    Ok(Json(AdminOkResponse { ok: true }))
}

#[utoipa::path(
    post,
    path = "/api/admin/usuarios/{id}/cancelar-eliminacion",
    tag = "admin",
    params(("id" = i32, Path, description = "User id")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Eliminación cancelada", body = AdminOkResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Requiere admin", body = ErrorResponse),
        (status = 404, description = "Usuario no encontrado", body = ErrorResponse)
    )
)]
pub async fn cancel_delete_legacy(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i32>,
) -> Result<Json<AdminOkResponse>, AppError> {
    user.require_admin()?;
    let updated = AdminPanelRepository::cancel_user_deletion(&state.pool, id).await?;
    if !updated {
        return Err(AppError::NotFound(format!("usuario {id}")));
    }
    Ok(Json(AdminOkResponse { ok: true }))
}

#[utoipa::path(
    get,
    path = "/api/admin/scrapers",
    tag = "admin",
    params(AdminScrapersQuery),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Listado admin de scraping_log", body = AdminScrapersResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Requiere admin", body = ErrorResponse)
    )
)]
pub async fn list_scrapers(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<AdminScrapersQuery>,
) -> Result<Json<AdminScrapersResponse>, AppError> {
    user.require_admin()?;
    Ok(Json(
        AdminPanelRepository::list_scrapers(&state.pool, &query).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/admin/cola-extraccion",
    tag = "admin",
    params(AdminExtractionQueueQuery),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Listado admin de cola_extraccion_samples", body = AdminExtractionQueueResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Requiere admin", body = ErrorResponse)
    )
)]
pub async fn list_extraction_queue(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<AdminExtractionQueueQuery>,
) -> Result<Json<AdminExtractionQueueResponse>, AppError> {
    user.require_admin()?;
    Ok(Json(
        AdminPanelRepository::list_extraction_queue(&state.pool, &query).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/admin/procesos",
    tag = "admin",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Estado de procesos de fondo", body = AdminProcessesResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Requiere admin", body = ErrorResponse)
    )
)]
pub async fn list_processes(user: CurrentUser) -> Result<Json<AdminProcessesResponse>, AppError> {
    user.require_admin()?;
    Ok(Json(AdminProcessService::list()?))
}

#[utoipa::path(
    get,
    path = "/api/admin/procesos/{nombre}",
    tag = "admin",
    params(("nombre" = String, Path, description = "Nombre del proceso")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Estado de un proceso", body = AdminProcessState),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Requiere admin", body = ErrorResponse),
        (status = 422, description = "Proceso invalido", body = ErrorResponse)
    )
)]
pub async fn process_state(
    user: CurrentUser,
    Path(nombre): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    user.require_admin()?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "proceso": AdminProcessService::state(&nombre)?,
    })))
}

#[utoipa::path(
    post,
    path = "/api/admin/procesos/{nombre}/start",
    tag = "admin",
    params(("nombre" = String, Path, description = "Nombre del proceso")),
    request_body = AdminProcessStartRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Proceso iniciado"),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Requiere admin", body = ErrorResponse),
        (status = 409, description = "Proceso ya activo", body = ErrorResponse)
    )
)]
pub async fn start_process(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(nombre): Path<String>,
    Json(request): Json<AdminProcessStartRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    user.require_admin()?;
    Ok(Json(
        AdminProcessService::start(&nombre, request.limit, &state.pool).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/admin/procesos/{nombre}/stop",
    tag = "admin",
    params(("nombre" = String, Path, description = "Nombre del proceso")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Proceso detenido"),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Requiere admin", body = ErrorResponse)
    )
)]
pub async fn stop_process(
    user: CurrentUser,
    Path(nombre): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    user.require_admin()?;
    Ok(Json(AdminProcessService::stop(&nombre)?))
}

#[utoipa::path(
    get,
    path = "/api/admin/procesos/cookies",
    tag = "admin",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Info de cookies yt-dlp"),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Requiere admin", body = ErrorResponse)
    )
)]
pub async fn process_cookies(user: CurrentUser) -> Result<Json<serde_json::Value>, AppError> {
    user.require_admin()?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "cookies": AdminProcessService::all_cookie_info(),
    })))
}

#[utoipa::path(
    post,
    path = "/api/admin/procesos/cookies",
    tag = "admin",
    request_body = AdminProcessCookiesRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Cookies guardadas"),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Requiere admin", body = ErrorResponse),
        (status = 422, description = "Payload invalido", body = ErrorResponse)
    )
)]
pub async fn save_process_cookies(
    user: CurrentUser,
    Json(request): Json<AdminProcessCookiesRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    user.require_admin()?;
    let cookie_type = request.tipo.as_deref().unwrap_or("youtube");
    Ok(Json(AdminProcessService::save_cookies(
        cookie_type,
        &request.contenido,
    )?))
}

/* [174A-57] Endpoint admin para inspeccionar las últimas mediciones del
 * algoritmo. [296A-1] Ahora registra para todos los usuarios; por defecto
 * filtra por el admin que consulta. `?all=true` para ver todos. */

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct AlgoTimingQuery {
    /// Máximo de entradas a devolver. Default 50, máximo 100.
    pub limit: Option<usize>,
    /// Si true, retorna mediciones de todos los usuarios. Por defecto solo las del admin actual.
    pub all: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/api/admin/algo-timing",
    params(AlgoTimingQuery),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Historial de mediciones del feed (más reciente primero)", body = Vec<TimingEntry>),
        (status = 401, description = "no auth"),
        (status = 403, description = "no admin")
    )
)]
pub async fn algo_timing_history(
    user: CurrentUser,
    Query(query): Query<AlgoTimingQuery>,
) -> Result<Json<Vec<TimingEntry>>, AppError> {
    user.require_admin()?;
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let filter_user = if query.all.unwrap_or(false) {
        None
    } else {
        Some(user.user_id)
    };
    Ok(Json(ALGO_TIMING.history(limit, filter_user)))
}

/* [186A-1] DELETE para limpiar el historial de mediciones. */
#[utoipa::path(
    delete,
    path = "/api/admin/algo-timing",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Historial limpiado"),
        (status = 401, description = "no auth"),
        (status = 403, description = "no admin")
    )
)]
pub async fn algo_timing_clear(user: CurrentUser) -> Result<Json<serde_json::Value>, AppError> {
    user.require_admin()?;
    ALGO_TIMING.clear();
    Ok(Json(serde_json::json!({"ok": true})))
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AdminSamplesDeleteAllResponse {
    pub ok: bool,
    pub eliminados: usize,
    pub errores: usize,
}

#[utoipa::path(
    delete,
    path = "/api/admin/samples/todos",
    tag = "admin",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Borrado masivo de samples", body = AdminSamplesDeleteAllResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Requiere admin", body = ErrorResponse)
    )
)]
pub async fn delete_all_samples(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<AdminSamplesDeleteAllResponse>, AppError> {
    user.require_admin()?;
    let outcome = AdminSamplesService::delete_all(&state.pool, state.storage.as_ref()).await?;

    Ok(Json(AdminSamplesDeleteAllResponse {
        ok: true,
        eliminados: outcome.eliminados,
        errores: outcome.errores,
    }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/resumen", get(summary))
        .route("/admin/actividad", get(activity))
        .route("/admin/usuarios", get(list_users))
        .route("/admin/usuarios/:id", put(update_user_legacy))
        .route("/admin/usuarios/:id/suspender", post(suspend_user_legacy))
        .route(
            "/admin/usuarios/:id/desuspender",
            post(unsuspend_user_legacy),
        )
        .route("/admin/usuarios/:id/eliminar", post(mark_delete_legacy))
        .route(
            "/admin/usuarios/:id/cancelar-eliminacion",
            post(cancel_delete_legacy),
        )
        .route("/admin/scrapers", get(list_scrapers))
        .route("/admin/cola-extraccion", get(list_extraction_queue))
        .route("/admin/procesos", get(list_processes))
        .route(
            "/admin/procesos/cookies",
            get(process_cookies).post(save_process_cookies),
        )
        .route("/admin/procesos/:nombre", get(process_state))
        .route("/admin/procesos/:nombre/start", post(start_process))
        .route("/admin/procesos/:nombre/stop", post(stop_process))
        .route("/admin/samples/todos", delete(delete_all_samples))
        .route("/admin/users/:id/suspend", post(suspend))
        .route("/admin/users/:id/activate", post(activate))
        .route("/admin/users/:id/delete", post(mark_delete))
        .route(
            "/admin/algo-timing",
            get(algo_timing_history).delete(algo_timing_clear),
        )
}
