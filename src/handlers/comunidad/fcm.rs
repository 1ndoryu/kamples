use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::errors::AppError;
#[allow(unused_imports)]
use crate::errors::ErrorResponse;
use crate::handlers::social::OkResponse;
use crate::middleware::CurrentUser;
use crate::services::{FcmNotificationService, FcmTokenPlatform};
use crate::AppState;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct RegisterFcmTokenRequest {
    pub token: String,
    #[serde(default)]
    pub plataforma: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct DeleteFcmTokenRequest {
    pub token: String,
}

/* [174A-76] API FCM Android.
 * Porta los endpoints legacy `/fcm/registrar` y `/fcm/eliminar` para que el
 * bridge mobile pueda registrar tokens de dispositivo y darlos de baja en logout. */

#[utoipa::path(
    post,
    path = "/api/fcm/registrar",
    tag = "fcm",
    request_body = RegisterFcmTokenRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Token FCM registrado", body = OkResponse),
        (status = 400, description = "Payload inválido", body = ErrorResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse)
    )
)]
pub async fn register_fcm_token(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<RegisterFcmTokenRequest>,
) -> Result<(StatusCode, Json<OkResponse>), AppError> {
    let platform = body
        .plataforma
        .as_deref()
        .map(FcmTokenPlatform::from_request_str)
        .unwrap_or_default();

    FcmNotificationService::register(&state.pool, user.user_id, &body.token, platform).await?;

    tracing::info!(
        user_id = user.user_id,
        plataforma = platform.as_db_str(),
        token = %truncate_token(&body.token),
        "token FCM registrado"
    );

    Ok((StatusCode::OK, Json(OkResponse { ok: true })))
}

#[utoipa::path(
    post,
    path = "/api/fcm/eliminar",
    tag = "fcm",
    request_body = DeleteFcmTokenRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Token FCM eliminado", body = OkResponse),
        (status = 400, description = "Payload inválido", body = ErrorResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse)
    )
)]
pub async fn delete_fcm_token(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<DeleteFcmTokenRequest>,
) -> Result<(StatusCode, Json<OkResponse>), AppError> {
    let deleted = FcmNotificationService::delete(&state.pool, user.user_id, &body.token).await?;

    tracing::info!(
        user_id = user.user_id,
        deleted,
        token = %truncate_token(&body.token),
        "token FCM eliminado"
    );

    Ok((StatusCode::OK, Json(OkResponse { ok: true })))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/fcm/registrar", post(register_fcm_token))
        .route("/fcm/eliminar", post(delete_fcm_token))
}

fn truncate_token(token: &str) -> String {
    token.chars().take(24).collect()
}
