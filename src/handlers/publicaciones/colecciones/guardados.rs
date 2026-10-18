use super::*;

/* [174A-66] Saved collections (bookmarks). */

#[derive(Debug, Serialize, ToSchema)]
pub struct SavedListResponse {
    pub items: Vec<SavedColeccion>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SavedListQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}
#[allow(dead_code)]
fn default_limit() -> i64 {
    30
}

#[utoipa::path(
    post, path = "/api/colecciones/{id}/save", tag = "colecciones",
    params(("id" = i64, Path, description = "ID de la coleccion a guardar")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, body = OkResponse),
        (status = 403, description = "No se puede guardar una coleccion privada ajena"),
        (status = 404, description = "Coleccion no encontrada"),
    )
)]
pub async fn save_coleccion(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<Json<OkResponse>, AppError> {
    let col = ColeccionesRepository::fetch(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("coleccion {id} no existe")))?;
    /* Solo se pueden guardar colecciones públicas o propias. */
    if !col.publica && col.usuario_id != user.user_id {
        return Err(AppError::Forbidden("coleccion privada".into()));
    }
    SavedCollectionsRepository::save(&state.pool, user.user_id, id).await?;
    Ok(Json(OkResponse { ok: true }))
}

#[utoipa::path(
    delete, path = "/api/colecciones/{id}/save", tag = "colecciones",
    params(("id" = i64, Path, description = "ID de la coleccion a quitar de guardadas")),
    security(("bearer_auth" = [])),
    responses((status = 200, body = OkResponse))
)]
pub async fn unsave_coleccion(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<Json<OkResponse>, AppError> {
    SavedCollectionsRepository::unsave(&state.pool, user.user_id, id).await?;
    Ok(Json(OkResponse { ok: true }))
}

#[allow(dead_code)]
#[utoipa::path(
    get, path = "/api/me/colecciones-guardadas", tag = "colecciones",
    params(
        ("limit" = Option<i64>, Query, description = "Tamaño de página (default 30, max 100)"),
        ("offset" = Option<i64>, Query, description = "Offset")
    ),
    security(("bearer_auth" = [])),
    responses((status = 200, body = SavedListResponse))
)]
pub async fn list_saved_colecciones(
    State(state): State<AppState>,
    user: CurrentUser,
    axum::extract::Query(q): axum::extract::Query<SavedListQuery>,
) -> Result<Json<SavedListResponse>, AppError> {
    let limit = q.limit.clamp(1, 100);
    let offset = q.offset.max(0);
    let items =
        SavedCollectionsRepository::list_by_user(&state.pool, user.user_id, limit, offset).await?;
    Ok(Json(SavedListResponse { items }))
}
