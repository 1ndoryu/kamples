use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::domain::format_price_cents;
use crate::errors::AppError;
#[allow(unused_imports)]
use crate::errors::ErrorResponse;
use crate::middleware::CurrentUser;
use crate::models::{
    CreatorConnectBalance, CreatorConnectState, CreatorConnectStatus, PaymentRedirectResponse,
};
use crate::repositories::BillingRepository;
use crate::services::{StripeConnectAccountSummary, StripeRuntime, StripeService};
use crate::AppState;

#[utoipa::path(
    post,
    path = "/api/connect/onboarding",
    tag = "payments",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "URL de onboarding de Stripe Connect", body = PaymentRedirectResponse),
        (status = 400, description = "Falta email o la cuenta no es apta", body = ErrorResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 409, description = "Stripe no configurado", body = ErrorResponse)
    )
)]
pub async fn create_connect_onboarding(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<PaymentRedirectResponse>, AppError> {
    let runtime = require_stripe_runtime(&state)?;
    let account = StripeService::create_connect_account(runtime, &state.pool, user.user_id).await?;
    let (return_url, refresh_url) = connect_onboarding_urls(state.public_base_url.as_deref());
    let link = StripeService::create_connect_onboarding_link(
        runtime,
        &account.id,
        &return_url,
        &refresh_url,
    )
    .await?;

    Ok(Json(PaymentRedirectResponse {
        ok: true,
        url: link.url,
    }))
}

#[utoipa::path(
    get,
    path = "/api/connect/estado",
    tag = "payments",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Estado semántico de la cuenta Connect", body = CreatorConnectStatus),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 404, description = "Usuario no encontrado", body = ErrorResponse)
    )
)]
pub async fn get_connect_status(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<CreatorConnectStatus>, AppError> {
    let profile = BillingRepository::find_stripe_user_profile(&state.pool, user.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Usuario {} no encontrado", user.user_id)))?;

    let Some(connect_id) = profile.stripe_connect_id else {
        return Ok(Json(CreatorConnectStatus {
            estado: CreatorConnectState::NoConfigurado,
            connect_id: None,
            cargos_activos: false,
            payouts_activos: false,
            detalle: None,
            requerimientos_pendientes: None,
        }));
    };

    let Some(runtime) = state.stripe_runtime.as_deref() else {
        return Ok(Json(CreatorConnectStatus {
            estado: CreatorConnectState::Error,
            connect_id: Some(connect_id),
            cargos_activos: false,
            payouts_activos: false,
            detalle: Some("Stripe no esta configurado en este entorno".to_string()),
            requerimientos_pendientes: Some(0),
        }));
    };

    match StripeService::retrieve_connect_account(runtime, &connect_id).await {
        Ok(account) => Ok(Json(connect_status_from_summary(account))),
        Err(error) => Ok(Json(CreatorConnectStatus {
            estado: CreatorConnectState::Error,
            connect_id: Some(connect_id),
            cargos_activos: false,
            payouts_activos: false,
            detalle: Some(error.to_string()),
            requerimientos_pendientes: Some(0),
        })),
    }
}

#[utoipa::path(
    post,
    path = "/api/connect/dashboard",
    tag = "payments",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "URL del dashboard Express de Stripe", body = PaymentRedirectResponse),
        (status = 400, description = "Cuenta Connect no configurada", body = ErrorResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 409, description = "Stripe no configurado", body = ErrorResponse)
    )
)]
pub async fn create_connect_dashboard_link(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<PaymentRedirectResponse>, AppError> {
    let runtime = require_stripe_runtime(&state)?;
    let profile = BillingRepository::find_stripe_user_profile(&state.pool, user.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Usuario {} no encontrado", user.user_id)))?;
    let connect_id = profile.stripe_connect_id.ok_or_else(|| {
        AppError::BadRequest(
            "No tienes cuenta de pagos configurada. Haz el onboarding primero.".to_string(),
        )
    })?;

    let link = StripeService::create_connect_login_link(runtime, &connect_id).await?;
    Ok(Json(PaymentRedirectResponse {
        ok: true,
        url: link.url,
    }))
}

#[utoipa::path(
    get,
    path = "/api/connect/balance",
    tag = "payments",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Balance disponible y pendiente del creador", body = CreatorConnectBalance),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 404, description = "Usuario no encontrado", body = ErrorResponse),
        (status = 409, description = "Stripe no configurado", body = ErrorResponse)
    )
)]
pub async fn get_connect_balance(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<CreatorConnectBalance>, AppError> {
    let profile = BillingRepository::find_stripe_user_profile(&state.pool, user.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Usuario {} no encontrado", user.user_id)))?;

    let Some(connect_id) = profile.stripe_connect_id else {
        return Ok(Json(CreatorConnectBalance {
            disponible: 0.0,
            pendiente: 0.0,
            moneda: "usd".to_string(),
        }));
    };

    let runtime = require_stripe_runtime(&state)?;
    let balance = StripeService::retrieve_connect_balance(runtime, &connect_id).await?;

    Ok(Json(CreatorConnectBalance {
        disponible: cents_to_major_units(balance.available_cents),
        pendiente: cents_to_major_units(balance.pending_cents),
        moneda: balance.currency,
    }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/connect/onboarding", post(create_connect_onboarding))
        .route("/connect/estado", get(get_connect_status))
        .route("/connect/dashboard", post(create_connect_dashboard_link))
        .route("/connect/balance", get(get_connect_balance))
}

fn connect_status_from_summary(summary: StripeConnectAccountSummary) -> CreatorConnectStatus {
    let cargos_activos = summary.charges_enabled.unwrap_or(false);
    let payouts_activos = summary.payouts_enabled.unwrap_or(false);
    let requerimientos_pendientes =
        Some(i32::try_from(summary.currently_due.len()).unwrap_or(i32::MAX));

    let (estado, detalle) = if cargos_activos && payouts_activos {
        (CreatorConnectState::Activo, None)
    } else if !summary.currently_due.is_empty() || summary.details_submitted == Some(false) {
        (
            CreatorConnectState::Pendiente,
            Some("Hay informacion pendiente por completar en Stripe".to_string()),
        )
    } else {
        (
            CreatorConnectState::Restringido,
            summary.disabled_reason.clone().map(|reason| {
                format!("Stripe requiere informacion adicional para activar pagos ({reason})")
            }),
        )
    };

    CreatorConnectStatus {
        estado,
        connect_id: Some(summary.id),
        cargos_activos,
        payouts_activos,
        detalle,
        requerimientos_pendientes,
    }
}

fn require_stripe_runtime(state: &AppState) -> Result<&StripeRuntime, AppError> {
    state
        .stripe_runtime
        .as_deref()
        .ok_or_else(|| AppError::Conflict("Stripe no esta configurado en este entorno".to_string()))
}

/* [13AA-1] Fallback = sitio público (frontend Vite en 5183), no el backend (3001). */
fn site_base_url(public_base_url: Option<&str>) -> String {
    public_base_url
        .map(|value| value.trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:5183".to_string())
}

fn connect_onboarding_urls(public_base_url: Option<&str>) -> (String, String) {
    let base_url = site_base_url(public_base_url);
    (
        format!("{base_url}/admin/dashboard/?connect=completado"),
        format!("{base_url}/admin/dashboard/?connect=refresh"),
    )
}

fn cents_to_major_units(cents: i64) -> f64 {
    format_price_cents(cents).parse::<f64>().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::{connect_onboarding_urls, connect_status_from_summary};
    use crate::models::CreatorConnectState;
    use crate::services::StripeConnectAccountSummary;

    #[test]
    fn onboarding_urls_match_legacy_shape() {
        let (return_url, refresh_url) = connect_onboarding_urls(Some("https://kamples.com/"));

        assert_eq!(
            return_url,
            "https://kamples.com/admin/dashboard/?connect=completado"
        );
        assert_eq!(
            refresh_url,
            "https://kamples.com/admin/dashboard/?connect=refresh"
        );
    }

    #[test]
    fn connect_status_marks_pending_when_requirements_exist() {
        let status = connect_status_from_summary(StripeConnectAccountSummary {
            id: "acct_123".to_string(),
            details_submitted: Some(false),
            charges_enabled: Some(false),
            payouts_enabled: Some(false),
            currently_due: vec!["individual.first_name".to_string()],
            disabled_reason: None,
        });

        assert_eq!(status.estado, CreatorConnectState::Pendiente);
        assert_eq!(status.requerimientos_pendientes, Some(1));
    }
}
