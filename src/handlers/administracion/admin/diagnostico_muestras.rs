use super::*;

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
