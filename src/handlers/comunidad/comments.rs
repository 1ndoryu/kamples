mod notifications;
mod payload;

use axum::extract::{DefaultBodyLimit, Path, Query, Request, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use utoipa::{IntoParams, ToSchema};

use crate::algorithm::InteractionKind;
use crate::errors::AppError;
use crate::handlers::likes::LikeResponse;
use crate::handlers::social::OkResponse;
use crate::middleware::{CurrentUser, OptionalUser};
use crate::repositories::{
    CommentContentKind, CommentDetail, CommentRepository, CommentTargetKind, CreateCommentParams,
    LikeKind, LikeRepository, ModerationRepository, ProfileRepository, Reaction,
};
use crate::AppState;
use tracing::warn;

use self::{
    notifications::{maybe_notify_comment_creation, maybe_notify_comment_like},
    payload::{
        build_comment_storage_key, extract_storage_key, normalize_content,
        parse_create_comment_request,
    },
};

const MAX_COMMENT_CHARS: usize = 2_000;
const MAX_JSON_BODY_BYTES: usize = 64 * 1024;
const MAX_IMAGE_UPLOAD_BYTES: usize = 8 * 1024 * 1024;
const MAX_AUDIO_UPLOAD_BYTES: usize = 24 * 1024 * 1024;
const MAX_COMMENT_MULTIPART_BODY_BYTES: usize = MAX_AUDIO_UPLOAD_BYTES + (4 * 1024 * 1024);
const DEFAULT_PAGE: i64 = 1;
const DEFAULT_PER_PAGE: i64 = 20;
const MAX_PER_PAGE: i64 = 100;
const DEFAULT_REPLIES_LIMIT: i64 = 100;

/* [174A-68] Comentarios polimórficos con replies, likes y media sobre el mismo schema social.
 * Se soporta creación JSON o multipart en un único endpoint para mantener paridad con desktop/legacy.
 * Gotcha: el cleanup de media contempla todo el subárbol porque delete hace cascade en respuestas. */

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct CommentListQuery {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct CommentRepliesQuery {
    #[serde(default = "default_replies_limit")]
    pub limit: i64,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateCommentJsonRequest {
    #[serde(default)]
    pub contenido: String,
    pub parent_id: Option<i32>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateCommentMultipartRequestDoc {
    #[schema(value_type = String, format = Binary)]
    pub media: Option<Vec<u8>>,
    pub contenido: Option<String>,
    pub parent_id: Option<i32>,
    pub tipo_contenido: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateCommentRequest {
    #[serde(default)]
    pub contenido: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CommentLikeRequest {
    #[serde(default)]
    pub reaccion: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CommentListResponse {
    pub items: Vec<CommentDetail>,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CommentRepliesResponse {
    pub items: Vec<CommentDetail>,
    pub limit: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CommentMutationResponse {
    pub ok: bool,
    pub comment: CommentDetail,
}

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

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/comentarios/:tipo/:target_id",
            get(list_comments)
                .post(create_comment)
                .layer(DefaultBodyLimit::max(MAX_COMMENT_MULTIPART_BODY_BYTES)),
        )
        .route(
            "/comentarios/:comment_id",
            axum::routing::put(update_comment).delete(delete_comment),
        )
        .route("/comentarios/:comment_id/respuestas", get(list_replies))
        .route(
            "/comentarios/:comment_id/like",
            post(like_comment).delete(unlike_comment),
        )
}

async fn collect_hidden_author_ids(
    state: &AppState,
    viewer_id: Option<i32>,
) -> Result<Vec<i32>, AppError> {
    let Some(user_id) = viewer_id else {
        return Ok(Vec::new());
    };

    let mut ids = ModerationRepository::list_blocked(&state.pool, user_id).await?;
    ids.extend(ModerationRepository::list_blockers(&state.pool, user_id).await?);
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
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

fn normalize_comment_detail(
    mut detail: CommentDetail,
    public_base_url: Option<&str>,
) -> CommentDetail {
    detail.media_url = asset_to_public_url(public_base_url, detail.media_url);
    detail.autor.avatar_url = asset_to_public_url(public_base_url, detail.autor.avatar_url);
    detail
}

fn asset_to_public_url(public_base_url: Option<&str>, raw: Option<String>) -> Option<String> {
    let raw = raw?.trim().replace('\\', "/");
    if raw.is_empty() {
        return None;
    }
    if raw.starts_with("http://") || raw.starts_with("https://") {
        return Some(raw);
    }

    let path = if raw.starts_with('/') {
        raw
    } else {
        format!("/uploads/{raw}")
    };

    Some(match public_base_url {
        Some(base) => format!("{}{}", base.trim_end_matches('/'), path),
        None => path,
    })
}

const fn default_page() -> i64 {
    DEFAULT_PAGE
}

const fn default_per_page() -> i64 {
    DEFAULT_PER_PAGE
}

const fn default_replies_limit() -> i64 {
    DEFAULT_REPLIES_LIMIT
}
