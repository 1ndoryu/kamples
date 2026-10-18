use axum::extract::{ws::WebSocketUpgrade, Query, State};
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use url::Url;
use utoipa::{IntoParams, ToSchema};

use crate::errors::AppError;
#[allow(unused_imports)]
use crate::errors::ErrorResponse;
use crate::middleware::CurrentUser;
use crate::AppState;

const DEFAULT_WS_TICKET_TTL_SECS: i64 = 60;
const MAX_WS_TICKET_TTL_SECS: i64 = 300;

/* [174A-70] Handshake websocket del backend principal.
 * - `GET /api/ws/ticket` emite ticket HMAC corto para el usuario autenticado.
 * - `GET /api/ws?ticket=...` valida el ticket y hace upgrade Axum.
 * No emite eventos de dominio todavía; eso queda para 174A-72. */

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct WebSocketTicketQuery {
    /// TTL pedido para el ticket, en segundos. Default: config, máximo: 300.
    pub ttl_secs: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct WebSocketConnectQuery {
    /// Ticket websocket HMAC emitido por `GET /api/ws/ticket`.
    pub ticket: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct WebSocketTicketResponse {
    pub ok: bool,
    pub ticket: String,
    pub ttl_secs: i64,
    pub ws_url: String,
}

#[utoipa::path(
    get,
    path = "/api/ws/ticket",
    tag = "ws",
    params(WebSocketTicketQuery),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Ticket websocket emitido", body = WebSocketTicketResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse)
    )
)]
pub async fn issue_ticket(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<WebSocketTicketQuery>,
) -> Result<Json<WebSocketTicketResponse>, AppError> {
    let ttl_secs = normalize_ticket_ttl(query.ttl_secs.unwrap_or(state.ws_ticket_ttl_secs));
    let ticket = glory_rs::websocket::generate_ticket(user.user_id, ttl_secs, &state.ws_secret);

    Ok(Json(WebSocketTicketResponse {
        ok: true,
        ticket,
        ttl_secs,
        ws_url: resolve_ws_url(&state),
    }))
}

#[utoipa::path(
    get,
    path = "/api/ws",
    tag = "ws",
    params(WebSocketConnectQuery),
    responses(
        (status = 101, description = "Upgrade websocket aceptado"),
        (status = 401, description = "Ticket websocket inválido", body = ErrorResponse),
        (status = 403, description = "Ticket websocket expirado", body = ErrorResponse),
        (status = 429, description = "Demasiadas conexiones websocket para el usuario", body = ErrorResponse)
    )
)]
pub async fn upgrade_connection(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
    Query(query): Query<WebSocketConnectQuery>,
) -> Result<Response, AppError> {
    let claims = glory_rs::websocket::verify_ticket(&query.ticket, &state.ws_secret)
        .map_err(map_ws_error)?;
    let hub = state.ws_hub.clone();

    Ok(ws.on_upgrade(move |socket| crate::ws::serve_socket(socket, hub, claims.user_id)))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/ws/ticket", get(issue_ticket))
        .route("/ws", get(upgrade_connection))
}

fn normalize_ticket_ttl(ttl_secs: i64) -> i64 {
    if ttl_secs <= 0 {
        DEFAULT_WS_TICKET_TTL_SECS
    } else {
        ttl_secs.min(MAX_WS_TICKET_TTL_SECS)
    }
}

fn resolve_ws_url(state: &AppState) -> String {
    resolve_ws_url_from_values(
        state.ws_public_url.as_deref(),
        state.public_base_url.as_deref(),
    )
}

fn resolve_ws_url_from_values(
    ws_public_url: Option<&str>,
    public_base_url: Option<&str>,
) -> String {
    if let Some(ws_public_url) = ws_public_url {
        return ws_public_url.to_string();
    }

    if let Some(public_base_url) = public_base_url {
        if let Ok(mut url) = Url::parse(public_base_url) {
            let next_scheme = match url.scheme() {
                "https" => "wss",
                "http" => "ws",
                _ => return "/api/ws".into(),
            };
            let _ = url.set_scheme(next_scheme);
            url.set_path("/api/ws");
            url.set_query(None);
            url.set_fragment(None);
            return url.to_string();
        }
    }

    "/api/ws".into()
}

fn map_ws_error(error: glory_rs::errors::AppError) -> AppError {
    match error {
        glory_rs::errors::AppError::NotFound(message) => AppError::NotFound(message),
        glory_rs::errors::AppError::BadRequest(message) => AppError::BadRequest(message),
        glory_rs::errors::AppError::Unauthorized => AppError::Unauthorized,
        glory_rs::errors::AppError::Forbidden(message) => AppError::Forbidden(message),
        glory_rs::errors::AppError::Conflict(message) => AppError::Conflict(message),
        glory_rs::errors::AppError::Internal(message) => AppError::Internal(message),
        glory_rs::errors::AppError::Database(error) => AppError::Database(error),
        glory_rs::errors::AppError::Validation(message) => AppError::Validation(message),
        glory_rs::errors::AppError::TooManyRequests(message) => AppError::TooManyRequests(message),
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_ticket_ttl, resolve_ws_url_from_values};

    #[test]
    fn normalize_ticket_ttl_uses_default_and_cap() {
        assert_eq!(normalize_ticket_ttl(0), 60);
        assert_eq!(normalize_ticket_ttl(-5), 60);
        assert_eq!(normalize_ticket_ttl(90), 90);
        assert_eq!(normalize_ticket_ttl(999), 300);
    }

    #[test]
    fn resolve_ws_url_prefers_explicit_url() {
        let resolved = resolve_ws_url_from_values(
            Some("wss://socket.kamples.com/api/ws"),
            Some("https://api.kamples.com"),
        );
        assert_eq!(resolved, "wss://socket.kamples.com/api/ws");
    }

    #[test]
    fn resolve_ws_url_derives_from_public_base_url() {
        let resolved = resolve_ws_url_from_values(None, Some("https://api.kamples.com/base"));
        assert_eq!(resolved, "wss://api.kamples.com/api/ws");
    }

    #[test]
    fn resolve_ws_url_falls_back_to_relative_path() {
        let resolved = resolve_ws_url_from_values(None, Some("nota-url-valida"));
        assert_eq!(resolved, "/api/ws");
    }
}
