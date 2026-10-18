use super::*;

#[utoipa::path(
    post,
    path = "/api/comentarios/{tipo}/{targetId}",
    tag = "social",
    request_body(
        content = CreateCommentMultipartRequestDoc,
        content_type = "multipart/form-data",
        description = "Acepta multipart con media opcional o JSON equivalente con { contenido, parent_id }"
    ),
    params(
        ("tipo" = String, Path, description = "Tipo de target: sample, publicacion, cancion, relacion, articulo"),
        ("targetId" = i32, Path, description = "ID del target comentado")
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 201, description = "Comentario creado", body = CommentMutationResponse),
        (status = 400, description = "Payload inválido"),
        (status = 401, description = "No autenticado"),
        (status = 403, description = "Cuenta no activa"),
        (status = 404, description = "Target o comentario padre no encontrado"),
        (status = 413, description = "Media demasiado grande"),
        (status = 415, description = "Tipo de media no soportado"),
        (status = 422, description = "Validación de comentario"),
    )
)]
pub async fn create_comment(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((tipo, target_id)): Path<(String, i32)>,
    request: Request,
) -> Result<(StatusCode, Json<CommentMutationResponse>), AppError> {
    let target_kind = CommentTargetKind::from_str(&tipo)?;
    if !CommentRepository::target_exists(&state.pool, target_kind, target_id).await? {
        return Err(AppError::NotFound(format!(
            "{} {} no existe",
            target_kind.as_db_str(),
            target_id
        )));
    }

    ensure_active_profile(&state, user.user_id).await?;

    let parsed = parse_create_comment_request(request, &state).await?;
    if let Some(parent_id) = parsed.parent_id {
        if !CommentRepository::validate_parent_context(
            &state.pool,
            parent_id,
            target_kind,
            target_id,
        )
        .await?
        {
            return Err(AppError::NotFound(format!(
                "comentario padre {parent_id} no existe en este contexto"
            )));
        }
    }

    let mut stored_media_key = None;
    let mut media_metadata = None;
    let mut media_url = None;
    let mut content_kind = CommentContentKind::Texto;

    if let Some(media) = parsed.media {
        let storage_key = build_comment_storage_key(user.user_id, &media.extension);
        state.storage.put_bytes(&storage_key, &media.bytes).await?;
        media_metadata = Some(serde_json::json!({
            "content_type": media.content_type,
            "size_bytes": media.bytes.len(),
            "original_filename": media.original_filename,
            "extension": media.extension,
            "media_kind": media.kind.as_db_str(),
        }));
        content_kind = media.kind;
        media_url = Some(storage_key.clone());
        stored_media_key = Some(storage_key);
    }

    let created_id = match CommentRepository::create(
        &state.pool,
        CreateCommentParams {
            autor_id: user.user_id,
            target_kind,
            target_id,
            contenido: &parsed.contenido,
            content_kind,
            media_url: media_url.as_deref(),
            media_metadata,
            parent_id: parsed.parent_id,
        },
    )
    .await
    {
        Ok(id) => id,
        Err(error) => {
            if let Some(key) = stored_media_key.as_deref() {
                state.storage.delete(key).await?;
            }
            return Err(error);
        }
    };

    let respuesta = finalizar_comentario(
        &state,
        user.user_id,
        created_id,
        (target_kind, target_id),
        parsed.parent_id,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(respuesta)))
}

/* Tras persistir el comentario: actualiza contadores, registra la interacción,
 * devuelve el comentario ya listo y avisa del fanout (si falla, solo se registra el aviso). */
async fn finalizar_comentario(
    state: &AppState,
    user_id: i32,
    created_id: i32,
    destino: (CommentTargetKind, i32),
    parent_id: Option<i32>,
) -> Result<CommentMutationResponse, AppError> {
    let (target_kind, target_id) = destino;
    if let Some(parent) = parent_id {
        CommentRepository::increment_replies(&state.pool, parent).await?;
    }
    CommentRepository::recount_target(&state.pool, target_kind, target_id).await?;
    state
        .algo_planner
        .register_interaction(&state.pool, &state.redis, user_id, InteractionKind::Comentario)
        .await?;

    let hidden = collect_hidden_author_ids(state, Some(user_id)).await?;
    let comment = CommentRepository::get(&state.pool, Some(user_id), created_id, &hidden)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("comentario {created_id} no existe")))?;

    if let Err(error) =
        maybe_notify_comment_creation(state, user_id, target_kind, target_id, parent_id).await
    {
        warn!(
            actor_id = user_id,
            comment_id = created_id,
            target_kind = target_kind.as_db_str(),
            target_id,
            error = %error,
            "falló fanout de comentario"
        );
    }

    Ok(CommentMutationResponse {
        ok: true,
        comment: normalize_comment_detail(comment, state.public_base_url.as_deref()),
    })
}

#[utoipa::path(
    put,
    path = "/api/comentarios/{commentId}",
    tag = "social",
    request_body = UpdateCommentRequest,
    params(("commentId" = i32, Path, description = "ID del comentario a editar")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Comentario actualizado", body = CommentMutationResponse),
        (status = 401, description = "No autenticado"),
        (status = 403, description = "No eres autor"),
        (status = 404, description = "Comentario no encontrado"),
        (status = 422, description = "Contenido inválido"),
    )
)]
pub async fn update_comment(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(comment_id): Path<i32>,
    Json(body): Json<UpdateCommentRequest>,
) -> Result<Json<CommentMutationResponse>, AppError> {
    let context = CommentRepository::find_context(&state.pool, comment_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("comentario {comment_id} no existe")))?;
    if context.autor_id != user.user_id {
        return Err(AppError::Forbidden("no eres autor del comentario".into()));
    }

    let contenido = normalize_content(&body.contenido, context.media_url.is_some())?;
    if !CommentRepository::update_content(&state.pool, comment_id, user.user_id, &contenido).await?
    {
        return Err(AppError::NotFound(format!(
            "comentario {comment_id} no existe"
        )));
    }

    let hidden = collect_hidden_author_ids(&state, Some(user.user_id)).await?;
    let comment = CommentRepository::get(&state.pool, Some(user.user_id), comment_id, &hidden)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("comentario {comment_id} no existe")))?;

    Ok(Json(CommentMutationResponse {
        ok: true,
        comment: normalize_comment_detail(comment, state.public_base_url.as_deref()),
    }))
}

#[utoipa::path(
    delete,
    path = "/api/comentarios/{commentId}",
    tag = "social",
    params(("commentId" = i32, Path, description = "ID del comentario a borrar")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Comentario eliminado", body = OkResponse),
        (status = 401, description = "No autenticado"),
        (status = 403, description = "No eres autor"),
        (status = 404, description = "Comentario no encontrado"),
    )
)]
pub async fn delete_comment(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(comment_id): Path<i32>,
) -> Result<Json<OkResponse>, AppError> {
    let context = CommentRepository::find_context(&state.pool, comment_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("comentario {comment_id} no existe")))?;
    if context.autor_id != user.user_id {
        return Err(AppError::Forbidden("no eres autor del comentario".into()));
    }

    let media_urls = CommentRepository::list_media_urls_for_thread(&state.pool, comment_id).await?;
    if !CommentRepository::delete(&state.pool, comment_id).await? {
        return Err(AppError::NotFound(format!(
            "comentario {comment_id} no existe"
        )));
    }
    if let Some(parent_id) = context.parent_id {
        CommentRepository::decrement_replies(&state.pool, parent_id).await?;
    }
    CommentRepository::recount_target(&state.pool, context.target_kind, context.target_id).await?;

    for media_url in media_urls {
        if let Some(storage_key) = extract_storage_key(&media_url) {
            state.storage.delete(&storage_key).await?;
        }
    }

    Ok(Json(OkResponse { ok: true }))
}

async fn ensure_active_profile(state: &AppState, user_id: i32) -> Result<(), AppError> {
    let profile = ProfileRepository::find_by_id(&state.pool, user_id)
        .await?
        .ok_or(AppError::NotFound(format!("usuario {user_id} no existe")))?;
    if profile.estado != "activo" {
        return Err(AppError::Forbidden(
            "La cuenta no está activa para comentar".into(),
        ));
    }
    Ok(())
}
