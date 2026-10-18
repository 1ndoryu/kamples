use super::*;

#[utoipa::path(
    post, path = "/api/publicaciones", tag = "posts",
    request_body = CreatePostRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 201, body = PostMutationResponse),
        (status = 400, description = "Payload inválido"),
    )
)]
pub async fn create_post(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<CreatePostRequest>,
) -> Result<(StatusCode, Json<PostMutationResponse>), AppError> {
    let payload = normalize_post_payload(&body.contenido, body.imagenes, body.samples_adjuntos)?;
    ensure_samples_exist(&state, &payload.samples_adjuntos).await?;
    let id = PostRepository::create(
        &state.pool,
        user.user_id,
        &payload.contenido,
        &payload.imagenes,
        &payload.samples_adjuntos,
    )
    .await?;
    let hidden = collect_hidden_author_ids(&state, user.user_id).await?;
    let post = PostRepository::get(&state.pool, user.user_id, id, &hidden)
        .await?
        .ok_or_else(|| AppError::Internal(format!("post {id} recién creado no visible")))?;

    /* Moderación asíncrona: texto + imágenes con Groq Vision (capas 1-3 del legacy).
     * El post se crea como 'aprobado'; si la IA lo rechaza, se actualiza a
     * 'rechazado' o 'revision' en background sin bloquear la respuesta 201. */
    if let Some(moderation) = &state.moderation {
        let moderation = Arc::clone(moderation);
        let pool = state.pool.clone();
        let contenido = payload.contenido.clone();
        let imagenes = payload.imagenes.clone();
        tokio::spawn(async move {
            let request = crate::services::ModerationRequest {
                entity_kind: crate::services::ModerationEntityKind::Publication,
                entity_id: Some(i64::from(id)),
                title: String::new(),
                body: contenido,
                tags: Vec::new(),
                primary_folder: None,
                secondary_folder: None,
                image_urls: imagenes,
                extra_context: None,
            };
            let result = moderation.moderate(&request).await;
            let (estado, razon) = match result.decision.verdict {
                crate::services::ModerationVerdict::Approved => return,
                crate::services::ModerationVerdict::Review => {
                    ("revision", result.decision.summary.clone())
                }
                crate::services::ModerationVerdict::Rejected => {
                    ("rechazado", result.decision.summary.clone())
                }
            };
            let detalle = serde_json::json!({
                "category": result.decision.category.as_str(),
                "reason_code": &result.decision.reason_code,
                "ai": result.ai_assessment.as_ref().map(|a| serde_json::json!({
                    "provider": a.provider.as_str(),
                    "model": &a.model,
                    "confidence": a.confidence,
                })),
            });
            if let Err(err) = PostRepository::update_moderation_status(
                &pool, id, estado, detalle, &razon,
            )
            .await
            {
                tracing::error!(post_id = id, error = %err, "Failed to update moderation status");
            } else {
                tracing::info!(post_id = id, estado, "Post moderation complete");
            }
        });
    }

    Ok((
        StatusCode::CREATED,
        Json(PostMutationResponse { ok: true, post }),
    ))
}

#[utoipa::path(
    get, path = "/api/publicaciones", tag = "posts",
    params(PostListQuery),
    responses((status = 200, body = PostListResponse))
)]
pub async fn list_posts(
    State(state): State<AppState>,
    user: OptionalUser,
    Query(query): Query<PostListQuery>,
) -> Result<Json<PostListResponse>, AppError> {
    /* [204A-1] Feed público: guests ven posts sin reacciones personalizadas.
     * viewer_id=0 garantiza que los subqueries SQL (siguiendo, mi_reaccion, etc.)
     * devuelvan false/null para guests — ningún user tiene id=0 en la BD. */
    let viewer_id = user.0.as_ref().map_or(0, |u| u.user_id);
    let (only_following, sort_popular) = parse_filter(&query.filtro)?;
    /* only_following requiere auth — si no hay usuario, ignorar el filtro */
    let only_following = only_following && user.0.is_some();
    let page = query.page.max(1);
    let per_page = query.per_page.clamp(1, 100);
    let offset = (page - 1) * per_page;
    let hidden = if let Some(ref u) = user.0 {
        collect_hidden_author_ids(&state, u.user_id).await?
    } else {
        vec![]
    };
    /* [254A-6] author_id directo tiene precedencia; si solo viene `autor=username`,
     * lo traducimos. Username inexistente => lista vacia (no error) para no romper
     * la UX del perfil cuando alguien navega a un usuario borrado. */
    let author_id = match query.author_id {
        Some(id) => Some(id),
        None => match query
            .autor
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(username) => {
                match crate::repositories::UserRepository::find_by_username(&state.pool, username)
                    .await?
                {
                    Some(u) => Some(u.id),
                    None => {
                        return Ok(Json(PostListResponse {
                            items: vec![],
                            page,
                            per_page,
                        }));
                    }
                }
            }
            None => None,
        },
    };
    let items = PostRepository::list(
        &state.pool,
        crate::repositories::PostListParams {
            viewer_id,
            only_following,
            sort_popular,
            author_id,
            blocked_ids: &hidden,
            limit: per_page,
            offset,
        },
    )
    .await?;
    Ok(Json(PostListResponse {
        items,
        page,
        per_page,
    }))
}

#[utoipa::path(
    get, path = "/api/publicaciones/{id}", tag = "posts",
    params(("id" = i32, Path, description = "ID de la publicación")),
    security(("bearer_auth" = [])),
    responses((status = 200, body = PostMutationResponse), (status = 404, description = "No encontrada"))
)]
pub async fn get_post(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i32>,
) -> Result<Json<PostMutationResponse>, AppError> {
    let hidden = collect_hidden_author_ids(&state, user.user_id).await?;
    let post = PostRepository::get(&state.pool, user.user_id, id, &hidden)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("publicacion {id} no existe")))?;
    Ok(Json(PostMutationResponse { ok: true, post }))
}

#[utoipa::path(
    put, path = "/api/publicaciones/{id}", tag = "posts",
    params(("id" = i32, Path, description = "ID de la publicación a editar")),
    request_body = UpdatePostRequest,
    security(("bearer_auth" = [])),
    responses((status = 200, body = PostMutationResponse), (status = 403, description = "No autorizado"))
)]
pub async fn update_post(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i32>,
    Json(body): Json<UpdatePostRequest>,
) -> Result<Json<PostMutationResponse>, AppError> {
    ensure_owner_original_post(&state, id, user.user_id).await?;
    let payload = normalize_post_payload(&body.contenido, body.imagenes, body.samples_adjuntos)?;
    ensure_samples_exist(&state, &payload.samples_adjuntos).await?;
    let updated = PostRepository::update(
        &state.pool,
        id,
        user.user_id,
        &payload.contenido,
        &payload.imagenes,
        &payload.samples_adjuntos,
    )
    .await?;
    if !updated {
        return Err(AppError::Conflict(format!(
            "no se pudo actualizar la publicacion {id}"
        )));
    }
    let hidden = collect_hidden_author_ids(&state, user.user_id).await?;
    let post = PostRepository::get(&state.pool, user.user_id, id, &hidden)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("publicacion {id} no existe")))?;
    Ok(Json(PostMutationResponse { ok: true, post }))
}

#[utoipa::path(
    delete, path = "/api/publicaciones/{id}", tag = "posts",
    params(("id" = i32, Path, description = "ID de la publicación a eliminar")),
    security(("bearer_auth" = [])),
    responses((status = 200, body = OkResponse), (status = 403, description = "No autorizado"))
)]
pub async fn delete_post(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i32>,
) -> Result<Json<OkResponse>, AppError> {
    let owner_id = ensure_can_delete_original_post(&state, id, &user).await?;
    let deleted = PostRepository::soft_delete(&state.pool, id, owner_id).await?;
    if !deleted {
        return Err(AppError::Conflict(format!(
            "no se pudo eliminar la publicacion {id}"
        )));
    }
    Ok(Json(OkResponse { ok: true }))
}
