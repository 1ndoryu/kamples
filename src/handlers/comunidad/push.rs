use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::errors::AppError;
#[allow(unused_imports)]
use crate::errors::ErrorResponse;
use crate::handlers::social::OkResponse;
use crate::middleware::CurrentUser;
use crate::services::{PushNotificationService, PushSubscriptionPlatform};
use crate::AppState;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct PushSubscriptionKeysRequest {
    pub p256dh: String,
    pub auth: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SubscribePushRequest {
    pub endpoint: String,
    pub keys: PushSubscriptionKeysRequest,
    #[serde(default)]
    pub plataforma: PushSubscriptionPlatform,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UnsubscribePushRequest {
    pub endpoint: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PushVapidKeyResponse {
    pub habilitado: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vapid_key: Option<String>,
}

/* [174A-75] API Web Push VAPID.
 * Porta los endpoints legacy `/push/vapid-key`, `/push/subscribe` y
 * `/push/unsubscribe` para que el frontend pueda registrar Service Worker,
 * almacenar endpoint+p256dh+auth y desuscribirse sin depender de Firebase. */

#[utoipa::path(
    get,
    path = "/api/push/vapid-key",
    tag = "push",
    responses(
        (status = 200, description = "Clave pública VAPID o estado deshabilitado", body = PushVapidKeyResponse)
    )
)]
pub async fn get_vapid_key(State(state): State<AppState>) -> Json<PushVapidKeyResponse> {
    let response = match state.push_runtime.as_ref() {
        Some(runtime) => PushVapidKeyResponse {
            habilitado: true,
            vapid_key: Some(runtime.public_key().to_string()),
        },
        None => PushVapidKeyResponse {
            habilitado: false,
            vapid_key: None,
        },
    };

    Json(response)
}

#[utoipa::path(
    post,
    path = "/api/push/subscribe",
    tag = "push",
    request_body = SubscribePushRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Suscripción push registrada", body = OkResponse),
        (status = 400, description = "Payload inválido", body = ErrorResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse)
    )
)]
pub async fn subscribe_push(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<SubscribePushRequest>,
) -> Result<(StatusCode, Json<OkResponse>), AppError> {
    PushNotificationService::subscribe(
        &state.pool,
        user.user_id,
        &body.endpoint,
        &body.keys.p256dh,
        &body.keys.auth,
        body.plataforma,
    )
    .await?;

    tracing::info!(
        user_id = user.user_id,
        plataforma = body.plataforma.as_db_str(),
        endpoint = %truncate_endpoint(&body.endpoint),
        "suscripción web push registrada"
    );

    Ok((StatusCode::OK, Json(OkResponse { ok: true })))
}

#[utoipa::path(
    post,
    path = "/api/push/unsubscribe",
    tag = "push",
    request_body = UnsubscribePushRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Suscripción push eliminada", body = OkResponse),
        (status = 400, description = "Payload inválido", body = ErrorResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse)
    )
)]
pub async fn unsubscribe_push(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<UnsubscribePushRequest>,
) -> Result<(StatusCode, Json<OkResponse>), AppError> {
    let deleted =
        PushNotificationService::unsubscribe(&state.pool, user.user_id, &body.endpoint).await?;

    tracing::info!(
        user_id = user.user_id,
        endpoint = %truncate_endpoint(&body.endpoint),
        deleted,
        "suscripción web push eliminada"
    );

    Ok((StatusCode::OK, Json(OkResponse { ok: true })))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/push/vapid-key", get(get_vapid_key))
        .route("/push/subscribe", post(subscribe_push))
        .route("/push/unsubscribe", post(unsubscribe_push))
}

fn truncate_endpoint(endpoint: &str) -> String {
    endpoint.chars().take(80).collect()
}
