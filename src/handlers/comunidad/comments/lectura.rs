use super::*;

#[utoipa::path(
    get,
    path = "/api/comentarios/{tipo}/{targetId}",
    tag = "social",
    params(
        ("tipo" = String, Path, description = "Tipo de target: sample, publicacion, cancion, relacion, articulo"),
        ("targetId" = i32, Path, description = "ID del target comentado"),
        CommentListQuery
    ),
    responses(
        (status = 200, description = "Lista de comentarios raíz", body = CommentListResponse),
        (status = 404, description = "Target no encontrado"),
        (status = 422, description = "Tipo inválido"),
    )
)]
pub async fn list_comments(
    State(state): State<AppState>,
    viewer: OptionalUser,
    Path((tipo, target_id)): Path<(String, i32)>,
    Query(query): Query<CommentListQuery>,
) -> Result<Json<CommentListResponse>, AppError> {
    let target_kind = CommentTargetKind::from_str(&tipo)?;
    if !CommentRepository::target_exists(&state.pool, target_kind, target_id).await? {
        return Err(AppError::NotFound(format!(
            "{} {} no existe",
            target_kind.as_db_str(),
            target_id
        )));
    }

    let page = query.page.max(1);
    let per_page = query.per_page.clamp(1, MAX_PER_PAGE);
    let viewer_id = viewer.0.as_ref().map(|user| user.user_id);
    let hidden = collect_hidden_author_ids(&state, viewer_id).await?;
    let items = CommentRepository::list_roots(
        &state.pool,
        viewer_id,
        target_kind,
        target_id,
        &hidden,
        per_page,
        (page - 1) * per_page,
    )
    .await?
    .into_iter()
    .map(|comment| normalize_comment_detail(comment, state.public_base_url.as_deref()))
    .collect();

    Ok(Json(CommentListResponse {
        items,
        page,
        per_page,
    }))
}

#[utoipa::path(
    get,
    path = "/api/comentarios/{commentId}/respuestas",
    tag = "social",
    params(
        ("commentId" = i32, Path, description = "ID del comentario padre"),
        CommentRepliesQuery
    ),
    responses(
        (status = 200, description = "Lista de respuestas", body = CommentRepliesResponse),
        (status = 404, description = "Comentario no encontrado"),
    )
)]
pub async fn list_replies(
    State(state): State<AppState>,
    viewer: OptionalUser,
    Path(comment_id): Path<i32>,
    Query(query): Query<CommentRepliesQuery>,
) -> Result<Json<CommentRepliesResponse>, AppError> {
    let viewer_id = viewer.0.as_ref().map(|user| user.user_id);
    let hidden = collect_hidden_author_ids(&state, viewer_id).await?;
    let parent = CommentRepository::get(&state.pool, viewer_id, comment_id, &hidden)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("comentario {comment_id} no existe")))?;

    let limit = query.limit.clamp(1, DEFAULT_REPLIES_LIMIT);
    let items = CommentRepository::list_replies(&state.pool, viewer_id, parent.id, &hidden, limit)
        .await?
        .into_iter()
        .map(|comment| normalize_comment_detail(comment, state.public_base_url.as_deref()))
        .collect();

    Ok(Json(CommentRepliesResponse { items, limit }))
}
