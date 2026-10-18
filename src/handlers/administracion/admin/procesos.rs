use super::*;

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
