use super::*;

#[utoipa::path(
    post,
    path = "/api/comentarios/{commentId}/like",
    tag = "social",
    request_body = CommentLikeRequest,
    params(("commentId" = i32, Path, description = "ID del comentario a reaccionar")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Reacción registrada", body = LikeResponse),
        (status = 401, description = "No autenticado"),
        (status = 404, description = "Comentario no encontrado"),
        (status = 422, description = "Reacción inválida"),
    )
)]
pub async fn like_comment(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(comment_id): Path<i32>,
    Json(body): Json<CommentLikeRequest>,
) -> Result<Json<LikeResponse>, AppError> {
    if !LikeRepository::target_exists(&state.pool, LikeKind::Comentario, comment_id).await? {
        return Err(AppError::NotFound(format!(
            "comentario {comment_id} no existe"
        )));
    }

    let reaction = match body.reaccion.as_deref() {
        Some(value) => Reaction::from_str(value)?,
        None => Reaction::Like,
    };
    LikeRepository::upsert_reaction(
        &state.pool,
        user.user_id,
        LikeKind::Comentario,
        comment_id,
        reaction,
    )
    .await?;
    LikeRepository::recount_target(&state.pool, LikeKind::Comentario, comment_id).await?;
    state
        .algo_planner
        .register_interaction(
            &state.pool,
            &state.redis,
            user.user_id,
            InteractionKind::Like,
        )
        .await?;

    if reaction.is_positive() {
        if let Err(error) = maybe_notify_comment_like(&state, user.user_id, comment_id).await {
            warn!(
                actor_id = user.user_id,
                comment_id,
                error = %error,
                "falló fanout de like en comentario"
            );
        }
    }

    Ok(Json(LikeResponse {
        ok: true,
        liked: true,
        reaccion: Some(reaction.as_db_str().to_string()),
    }))
}

#[utoipa::path(
    delete,
    path = "/api/comentarios/{commentId}/like",
    tag = "social",
    params(("commentId" = i32, Path, description = "ID del comentario")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Reacción eliminada", body = LikeResponse),
        (status = 401, description = "No autenticado"),
    )
)]
pub async fn unlike_comment(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(comment_id): Path<i32>,
) -> Result<Json<LikeResponse>, AppError> {
    LikeRepository::delete_reaction(&state.pool, user.user_id, LikeKind::Comentario, comment_id)
        .await?;
    LikeRepository::recount_target(&state.pool, LikeKind::Comentario, comment_id).await?;

    Ok(Json(LikeResponse {
        ok: true,
        liked: false,
        reaccion: None,
    }))
}
