use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::algorithm::InteractionKind;
use crate::errors::AppError;
#[allow(unused_imports)]
use crate::errors::ErrorResponse;
use crate::middleware::{CurrentUser, OptionalUser};
use crate::repositories::{
    BlockRepository, BlockedUser, FollowRepository, ProfileRepository, UserRepository,
};
use crate::services::NotificationFanoutService;
use crate::AppState;
use tracing::warn;

/* [174A-60] Follows + Blocks. Port de:
 * - SocialController::seguir/dejarDeSeguir → POST/DELETE /api/follow/:userId.
 * - ModeracionController::bloquearUsuario/desbloquearUsuario/misBloqueados →
 *   POST/DELETE /api/block/:userId, GET /api/me/bloqueados.
 *
 * Reglas portadas:
 * - Self-follow / self-block prohibido (400).
 * - Verificar que target existe (404).
 * - Bloquear → unfollow mutuo + recount.
 * - Trigger AlgoPlanner Follow al seguir.
 *
 * NO portado:
 * - Rate limit 20 follows/min, 10 bloqueos/min (sin RateLimiter global).
 * - Notificación al target (Fase 11).
 * - Verificación de ban activo (depende de QQ71).
 */

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OkResponse {
    pub ok: bool,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct BlockRequest {
    #[serde(default)]
    pub razon: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BlockedListResponse {
    pub data: Vec<BlockedUser>,
}

#[utoipa::path(
    post,
    path = "/api/follow/{userId}",
    tag = "social",
    params(("userId" = i32, Path, description = "ID del usuario a seguir")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Follow registrado", body = OkResponse),
        (status = 400, description = "Auto-follow prohibido"),
        (status = 401, description = "No autenticado"),
        (status = 404, description = "Usuario no encontrado"),
    )
)]
pub async fn follow_user(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(target_id): Path<i32>,
) -> Result<Json<OkResponse>, AppError> {
    if user.user_id == target_id {
        return Err(AppError::Validation("No puedes seguirte a ti mismo".into()));
    }
    if !FollowRepository::user_exists(&state.pool, target_id).await? {
        return Err(AppError::NotFound(format!("usuario {target_id} no existe")));
    }

    FollowRepository::follow(&state.pool, user.user_id, target_id).await?;
    FollowRepository::recount(&state.pool, user.user_id, target_id).await?;

    state
        .algo_planner
        .register_interaction(
            &state.pool,
            &state.redis,
            user.user_id,
            InteractionKind::Follow,
        )
        .await?;

    if let Err(error) =
        NotificationFanoutService::dispatch_follow(&state, target_id, user.user_id).await
    {
        warn!(
            actor_id = user.user_id,
            recipient_id = target_id,
            error = %error,
            "falló fanout de follow"
        );
    }

    Ok(Json(OkResponse { ok: true }))
}

#[utoipa::path(
    delete,
    path = "/api/follow/{userId}",
    tag = "social",
    params(("userId" = i32, Path, description = "ID del usuario a dejar de seguir")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Unfollow registrado", body = OkResponse),
        (status = 401, description = "No autenticado"),
    )
)]
pub async fn unfollow_user(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(target_id): Path<i32>,
) -> Result<Json<OkResponse>, AppError> {
    FollowRepository::unfollow(&state.pool, user.user_id, target_id).await?;
    FollowRepository::recount(&state.pool, user.user_id, target_id).await?;
    Ok(Json(OkResponse { ok: true }))
}

#[utoipa::path(
    post,
    path = "/api/block/{userId}",
    tag = "social",
    params(("userId" = i32, Path, description = "ID del usuario a bloquear")),
    request_body = BlockRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Bloqueo registrado", body = OkResponse),
        (status = 400, description = "Auto-bloqueo prohibido"),
        (status = 401, description = "No autenticado"),
        (status = 404, description = "Usuario no encontrado"),
    )
)]
pub async fn block_user(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(target_id): Path<i32>,
    Json(body): Json<BlockRequest>,
) -> Result<Json<OkResponse>, AppError> {
    if user.user_id == target_id {
        return Err(AppError::Validation(
            "No puedes bloquearte a ti mismo".into(),
        ));
    }
    if !FollowRepository::user_exists(&state.pool, target_id).await? {
        return Err(AppError::NotFound(format!("usuario {target_id} no existe")));
    }

    let razon = body.razon.unwrap_or_default();
    BlockRepository::block(&state.pool, user.user_id, target_id, &razon).await?;

    /* Bloquear implica unfollow mutuo. */
    FollowRepository::unfollow(&state.pool, user.user_id, target_id).await?;
    FollowRepository::unfollow(&state.pool, target_id, user.user_id).await?;
    FollowRepository::recount(&state.pool, user.user_id, target_id).await?;
    FollowRepository::recount(&state.pool, target_id, user.user_id).await?;

    Ok(Json(OkResponse { ok: true }))
}

#[utoipa::path(
    delete,
    path = "/api/block/{userId}",
    tag = "social",
    params(("userId" = i32, Path, description = "ID del usuario a desbloquear")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Desbloqueo registrado", body = OkResponse),
        (status = 401, description = "No autenticado"),
    )
)]
pub async fn unblock_user(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(target_id): Path<i32>,
) -> Result<Json<OkResponse>, AppError> {
    BlockRepository::unblock(&state.pool, user.user_id, target_id).await?;
    Ok(Json(OkResponse { ok: true }))
}

#[utoipa::path(
    get,
    path = "/api/me/bloqueados",
    tag = "social",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Lista de usuarios bloqueados", body = BlockedListResponse),
        (status = 401, description = "No autenticado"),
    )
)]
pub async fn my_blocks(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<(StatusCode, Json<BlockedListResponse>), AppError> {
    let data = BlockRepository::list(&state.pool, user.user_id).await?;
    Ok((StatusCode::OK, Json(BlockedListResponse { data })))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/follow/:userId", post(follow_user))
        .route("/follow/:userId", delete(unfollow_user))
        .route("/block/:userId", post(block_user))
        .route("/block/:userId", delete(unblock_user))
        .route("/me/bloqueados", get(my_blocks))
        .route("/users/me/seguidos", get(my_following))
        .route("/me/seguidos", get(my_following))
        .route("/usuarios/:username/seguidores", get(seguidores))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FollowedIdItem {
    pub id: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FollowedListResponse {
    pub data: Vec<FollowedIdItem>,
}

/* [274A-16] GET /api/users/me/seguidos (alias /me/seguidos)
 * Lista de IDs de usuarios que el usuario actual sigue.
 * Migrado desde SocialController::misSeguidos (PHP). */
#[utoipa::path(get, path = "/api/users/me/seguidos",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Lista de IDs seguidos", body = FollowedListResponse),
        (status = 401, body = ErrorResponse)
    ))]
pub async fn my_following(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<FollowedListResponse>, AppError> {
    let ids = FollowRepository::ids_seguidos(&state.pool, user.user_id).await?;
    let data = ids.into_iter().map(|id| FollowedIdItem { id }).collect();
    Ok(Json(FollowedListResponse { data }))
}

/* [296A-3] GET /api/usuarios/{username}/seguidores
 * Lista paginada de seguidores de un usuario. Port de SocialController::seguidores (PHP). */

#[derive(Debug, Deserialize, IntoParams)]
pub struct SeguidoresQuery {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}
fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    20
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SeguidorResumen {
    pub id: i32,
    pub username: String,
    pub nombre_visible: String,
    pub avatar_url: Option<String>,
    pub siguiendo: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SeguidoresListResponse {
    pub data: Vec<SeguidorResumen>,
    pub page: i64,
    pub per_page: i64,
}

#[utoipa::path(
    get,
    path = "/api/usuarios/{username}/seguidores",
    tag = "social",
    params(("username" = String, Path, description = "Username"), SeguidoresQuery),
    responses(
        (status = 200, description = "Lista paginada de seguidores", body = SeguidoresListResponse),
        (status = 404, body = ErrorResponse),
    )
)]
pub async fn seguidores(
    State(state): State<AppState>,
    Path(username): Path<String>,
    Query(q): Query<SeguidoresQuery>,
    OptionalUser(viewer): OptionalUser,
) -> Result<Json<SeguidoresListResponse>, AppError> {
    let target = UserRepository::find_by_username(&state.pool, &username)
        .await?
        .ok_or(AppError::NotFound(format!("usuario {username}")))?;

    let limit = q.per_page.clamp(1, 50);
    let offset = (q.page.max(1) - 1) * limit;
    let follower_ids =
        FollowRepository::ids_seguidores(&state.pool, target.id, limit, offset).await?;

    if follower_ids.is_empty() {
        return Ok(Json(SeguidoresListResponse {
            data: vec![],
            page: q.page,
            per_page: q.per_page,
        }));
    }

    /* Batch lookup de perfiles basicos. */
    let rows = ProfileRepository::perfiles_basicos(&state.pool, &follower_ids).await?;

    /* Map id → profile para mantener el orden de follows.created_at. */
    let profile_map: std::collections::HashMap<i32, (String, String, Option<String>)> =
        rows.into_iter().map(|(id, u, n, a)| (id, (u, n, a))).collect();

    /* Determinar a quién sigue el viewer entre estos seguidores. */
    let viewer_following_set: std::collections::HashSet<i32> = if let Some(ref v) = viewer {
        FollowRepository::ids_seguidos(&state.pool, v.user_id)
            .await?
            .into_iter()
            .collect()
    } else {
        std::collections::HashSet::new()
    };

    let data: Vec<SeguidorResumen> = follower_ids
        .iter()
        .filter_map(|&fid| {
            profile_map.get(&fid).map(|(username, nombre_visible, avatar_url)| {
                SeguidorResumen {
                    id: fid,
                    username: username.clone(),
                    nombre_visible: nombre_visible.clone(),
                    avatar_url: avatar_url.clone(),
                    siguiendo: viewer_following_set.contains(&fid),
                }
            })
        })
        .collect();

    Ok(Json(SeguidoresListResponse {
        data,
        page: q.page,
        per_page: q.per_page,
    }))
}
