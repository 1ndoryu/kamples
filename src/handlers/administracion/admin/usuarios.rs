use super::*;

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
