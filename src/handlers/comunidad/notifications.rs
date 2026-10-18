use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::errors::AppError;
#[allow(unused_imports)]
use crate::errors::ErrorResponse;
use crate::handlers::social::OkResponse;
use crate::middleware::CurrentUser;
use crate::repositories::BlockRepository;
use crate::repositories::UserNotification;
use crate::services::NotificationService;
use crate::AppState;

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct NotificationListQuery {
    #[serde(default = "default_page")]
    pub page: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NotificationListResponse {
    pub data: Vec<UserNotification>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct NotificationCountResponse {
    pub total: i64,
}

/* [174A-74] Endpoints base del centro de notificaciones.
 * Este corte porta lectura paginada, badge de no leídas y marcado de lectura.
 * Los productores concretos y el fanout multicanal siguen en tareas posteriores. */

#[utoipa::path(
    get,
    path = "/api/notificaciones",
    tag = "notifications",
    params(NotificationListQuery),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Lista de notificaciones del usuario", body = NotificationListResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse)
    )
)]
pub async fn list_notifications(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<NotificationListQuery>,
) -> Result<Json<NotificationListResponse>, AppError> {
    let hidden_actor_ids = collect_hidden_actor_ids(&state, user.user_id).await?;
    let notifications = NotificationService::list_for_user(
        &state.pool,
        user.user_id,
        &hidden_actor_ids,
        query.page,
    )
    .await?
    .into_iter()
    .map(|notification| normalize_notification(notification, state.public_base_url.as_deref()))
    .collect();

    Ok(Json(NotificationListResponse {
        data: notifications,
    }))
}

#[utoipa::path(
    post,
    path = "/api/notificaciones/{id}/leer",
    tag = "notifications",
    params(("id" = i32, Path, description = "ID de la notificación")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Notificación marcada como leída", body = OkResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse)
    )
)]
pub async fn mark_notification_read(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(notification_id): Path<i32>,
) -> Result<(StatusCode, Json<OkResponse>), AppError> {
    NotificationService::mark_read(&state.pool, user.user_id, notification_id).await?;
    Ok((StatusCode::OK, Json(OkResponse { ok: true })))
}

#[utoipa::path(
    post,
    path = "/api/notificaciones/leer-todas",
    tag = "notifications",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Todas las notificaciones marcadas como leídas", body = OkResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse)
    )
)]
pub async fn mark_all_notifications_read(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<(StatusCode, Json<OkResponse>), AppError> {
    NotificationService::mark_all_read(&state.pool, user.user_id).await?;
    Ok((StatusCode::OK, Json(OkResponse { ok: true })))
}

#[utoipa::path(
    get,
    path = "/api/notificaciones/conteo",
    tag = "notifications",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Conteo de notificaciones no leídas", body = NotificationCountResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse)
    )
)]
pub async fn unread_notifications_count(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<NotificationCountResponse>, AppError> {
    let total = NotificationService::unread_count(&state.pool, user.user_id).await?;
    Ok(Json(NotificationCountResponse { total }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/notificaciones", get(list_notifications))
        .route("/notificaciones/conteo", get(unread_notifications_count))
        .route(
            "/notificaciones/leer-todas",
            post(mark_all_notifications_read),
        )
        .route("/notificaciones/:id/leer", post(mark_notification_read))
}

async fn collect_hidden_actor_ids(state: &AppState, user_id: i32) -> Result<Vec<i32>, AppError> {
    Ok(BlockRepository::list(&state.pool, user_id)
        .await?
        .into_iter()
        .map(|entry| entry.bloqueado_id)
        .collect())
}

fn normalize_notification(
    mut notification: UserNotification,
    public_base_url: Option<&str>,
) -> UserNotification {
    if let Some(actor) = notification.actor.as_mut() {
        actor.avatar_url = asset_to_public_url(public_base_url, actor.avatar_url.take());
    }
    notification
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
    1
}
