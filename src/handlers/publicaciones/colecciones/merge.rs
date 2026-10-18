use super::*;

#[derive(Debug, Deserialize, ToSchema)]
pub struct MergeColeccionRequest {
    pub source_id: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MergeColeccionResponse {
    pub ok: bool,
    pub moved: i64,
}

/* [174A-65] Merge: combina source en target, soft-deletea source.
 * Ambas deben pertenecer al usuario actual. */
#[utoipa::path(
    post, path = "/api/colecciones/{id}/merge", tag = "colecciones",
    params(("id" = i64, Path, description = "ID de la coleccion target")),
    request_body = MergeColeccionRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, body = MergeColeccionResponse),
        (status = 400, description = "source == target"),
        (status = 403, description = "No autorizado en alguna de las dos colecciones"),
        (status = 404, description = "Alguna coleccion no existe"),
    )
)]
pub async fn merge_coleccion(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(target_id): Path<i64>,
    Json(body): Json<MergeColeccionRequest>,
) -> Result<Json<MergeColeccionResponse>, AppError> {
    if !ColeccionesRepository::is_owner(&state.pool, target_id, user.user_id).await? {
        return Err(AppError::Forbidden(
            "no eres dueño de la coleccion target".into(),
        ));
    }
    if !ColeccionesRepository::is_owner(&state.pool, body.source_id, user.user_id).await? {
        return Err(AppError::Forbidden(
            "no eres dueño de la coleccion source".into(),
        ));
    }
    let moved = ColeccionesRepository::merge(&state.pool, target_id, body.source_id).await?;
    Ok(Json(MergeColeccionResponse { ok: true, moved }))
}
