use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::domain::{format_price_cents, kamples_plan_config, KamplesPlanConfig, KamplesPlanId};
use crate::errors::AppError;
#[allow(unused_imports)]
use crate::errors::ErrorResponse;
use crate::middleware::CurrentUser;
use crate::models::{
    CreateSampleCheckoutRequest, CreateSubscriptionCheckoutRequest, PaymentPlanPeriod,
    PaymentPlanPublic, PaymentPlansResponse, PaymentRedirectResponse,
};
use crate::repositories::BillingRepository;
use crate::services::{StripeRuntime, StripeSampleCheckoutRequest, StripeService};
use crate::AppState;

pub mod webhook;

/* [174A-80] Endpoint público de catálogo de planes.
 * Reusa el dominio de pagos como fuente única para que precios, límites y
 * revenue share no vuelvan a duplicarse entre backend, checkout y descargas. */

#[utoipa::path(
    get,
    path = "/api/pagos/planes",
    tag = "payments",
    responses(
        (status = 200, description = "Catálogo público de planes Kamples", body = PaymentPlansResponse)
    )
)]
pub async fn list_plans(State(state): State<AppState>) -> Json<PaymentPlansResponse> {
    Json(build_payment_plans_response(
        state.stripe_runtime.as_deref(),
    ))
}

#[utoipa::path(
    post,
    path = "/api/pagos/checkout",
    tag = "payments",
    request_body = CreateSubscriptionCheckoutRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "URL de Stripe Checkout para suscripción", body = PaymentRedirectResponse),
        (status = 400, description = "Plan o periodo inválido", body = ErrorResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 409, description = "Stripe no configurado", body = ErrorResponse)
    )
)]
pub async fn create_subscription_checkout(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<CreateSubscriptionCheckoutRequest>,
) -> Result<Json<PaymentRedirectResponse>, AppError> {
    let runtime = require_stripe_runtime(&state)?;
    let period = body.periodo.unwrap_or(PaymentPlanPeriod::Mensual);

    if !matches!(body.plan, KamplesPlanId::Pro | KamplesPlanId::Premium) {
        return Err(AppError::Validation(
            "Plan debe ser pro o premium".to_string(),
        ));
    }
    if period != PaymentPlanPeriod::Mensual {
        return Err(AppError::Validation(
            "El checkout anual todavia no esta configurado".to_string(),
        ));
    }

    let (success_url, cancel_url) = subscription_checkout_urls(state.public_base_url.as_deref());
    let session = StripeService::create_subscription_checkout_session(
        runtime,
        &state.pool,
        user.user_id,
        body.plan,
        &success_url,
        &cancel_url,
    )
    .await?;

    Ok(Json(PaymentRedirectResponse {
        ok: true,
        url: session.url,
    }))
}

#[utoipa::path(
    post,
    path = "/api/pagos/checkout-sample",
    tag = "payments",
    request_body = CreateSampleCheckoutRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "URL de Stripe Checkout para compra individual", body = PaymentRedirectResponse),
        (status = 400, description = "Sample inválido o no comprable", body = ErrorResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 404, description = "Sample no encontrado", body = ErrorResponse),
        (status = 409, description = "Stripe no configurado", body = ErrorResponse)
    )
)]
pub async fn create_sample_checkout(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<CreateSampleCheckoutRequest>,
) -> Result<Json<PaymentRedirectResponse>, AppError> {
    let runtime = require_stripe_runtime(&state)?;
    if body.sample_id <= 0 {
        return Err(AppError::Validation("ID de sample requerido".to_string()));
    }

    let sample = BillingRepository::find_sample_checkout_candidate(&state.pool, body.sample_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Sample {} no encontrado", body.sample_id)))?;

    let price = sample.price.unwrap_or(0.0);
    if !sample.is_premium || price <= 0.0 {
        return Err(AppError::Validation(
            "Este sample no tiene precio de venta".to_string(),
        ));
    }
    if sample.creator_id == user.user_id {
        return Err(AppError::BadRequest(
            "No puedes comprar tu propio sample".to_string(),
        ));
    }
    if BillingRepository::has_completed_sample_purchase(&state.pool, user.user_id, body.sample_id)
        .await?
    {
        return Err(AppError::BadRequest("Ya compraste este sample".to_string()));
    }

    let price_cents = price_to_cents(price)?;
    let (success_url, cancel_url) = sample_checkout_urls(
        state.public_base_url.as_deref(),
        sample.sample_id,
        &sample.slug,
    );
    let session = StripeService::create_sample_checkout_session(
        runtime,
        &state.pool,
        user.user_id,
        &StripeSampleCheckoutRequest {
            sample_id: sample.sample_id,
            creator_id: sample.creator_id,
            sample_title: sample.sample_title,
            price_cents,
            success_url,
            cancel_url,
        },
    )
    .await?;

    Ok(Json(PaymentRedirectResponse {
        ok: true,
        url: session.url,
    }))
}

#[utoipa::path(
    post,
    path = "/api/pagos/portal",
    tag = "payments",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "URL del Customer Portal", body = PaymentRedirectResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 409, description = "Stripe no configurado o customer ausente", body = ErrorResponse)
    )
)]
pub async fn create_billing_portal(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<PaymentRedirectResponse>, AppError> {
    let runtime = require_stripe_runtime(&state)?;
    let return_url = billing_portal_return_url(state.public_base_url.as_deref());
    let session = StripeService::create_billing_portal_session(
        runtime,
        &state.pool,
        user.user_id,
        &return_url,
    )
    .await?;

    Ok(Json(PaymentRedirectResponse {
        ok: true,
        url: session.url,
    }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/pagos/planes", get(list_plans))
        .route("/pagos/checkout", post(create_subscription_checkout))
        .route("/pagos/checkout-sample", post(create_sample_checkout))
        .route("/pagos/webhook", post(webhook::payment_webhook))
        .route("/pagos/portal", post(create_billing_portal))
}

fn build_payment_plans_response(runtime: Option<&StripeRuntime>) -> PaymentPlansResponse {
    let publishable_key = runtime.and_then(|stripe| stripe.publishable_key().map(str::to_owned));

    PaymentPlansResponse {
        stripe_habilitado: runtime.is_some(),
        publishable_key,
        moneda: "usd".to_string(),
        planes: [
            KamplesPlanId::Free,
            KamplesPlanId::Pro,
            KamplesPlanId::Premium,
        ]
        .into_iter()
        .map(|plan_id| {
            let config = kamples_plan_config(plan_id);
            let price_id_configurado = runtime
                .and_then(|stripe| stripe.price_id_for_plan(plan_id))
                .is_some();

            serialize_plan(config, price_id_configurado)
        })
        .collect(),
    }
}

fn serialize_plan(config: &KamplesPlanConfig, price_id_configurado: bool) -> PaymentPlanPublic {
    let precio_anual_cents = annual_price_cents(config.monthly_price_cents);
    let ahorro_anual_cents = annual_savings_cents(config.monthly_price_cents);

    PaymentPlanPublic {
        id: config.id,
        nombre: public_plan_name(config.id).to_string(),
        precio_mensual_cents: config.monthly_price_cents,
        precio_mensual: format_price_cents(config.monthly_price_cents),
        precio_anual_cents,
        precio_anual: format_price_cents(precio_anual_cents),
        ahorro_anual_cents,
        ahorro_anual: format_price_cents(ahorro_anual_cents),
        descargas_dia: config.downloads_per_day,
        subidas_mes: config.uploads_per_month,
        max_samples: config.max_samples,
        transferencia_gb: config.transfer_gb,
        revenue_share_bps: config.revenue_share_bps,
        revenue_share_label: revenue_share_label(config.revenue_share_bps),
        price_id_configurado,
        prueba_gratuita_dias: config.free_trial_days,
        descargas_prueba: config.trial_downloads,
    }
}

fn public_plan_name(plan: KamplesPlanId) -> &'static str {
    match plan {
        KamplesPlanId::Free => "Free",
        KamplesPlanId::Pro => "Pro",
        KamplesPlanId::Premium => "Premium",
    }
}

fn revenue_share_label(revenue_share_bps: u16) -> String {
    let creator_percent = revenue_share_bps / 100;
    let platform_percent = 100_u16.saturating_sub(creator_percent);
    format!("{creator_percent}/{platform_percent}")
}

fn annual_price_cents(monthly_price_cents: i64) -> i64 {
    monthly_price_cents * 10
}

fn annual_savings_cents(monthly_price_cents: i64) -> i64 {
    monthly_price_cents * 2
}

fn require_stripe_runtime(state: &AppState) -> Result<&StripeRuntime, AppError> {
    state
        .stripe_runtime
        .as_deref()
        .ok_or_else(|| AppError::Conflict("Stripe no esta configurado en este entorno".to_string()))
}

/* [13AA-1] Fallback = sitio público (frontend Vite en 5183), no el backend: en dev el backend
 * (3001) no sirve /planes ni /descargas. */
fn site_base_url(public_base_url: Option<&str>) -> String {
    public_base_url
        .map(|value| value.trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:5183".to_string())
}

fn subscription_checkout_urls(public_base_url: Option<&str>) -> (String, String) {
    let base_url = site_base_url(public_base_url);
    (
        format!("{base_url}/planes/?checkout=exito"),
        format!("{base_url}/planes/?checkout=cancelado"),
    )
}

fn sample_checkout_urls(
    public_base_url: Option<&str>,
    sample_id: i32,
    slug: &str,
) -> (String, String) {
    let base_url = site_base_url(public_base_url);
    let slug = slug.trim();
    let sample_path = if slug.is_empty() {
        sample_id.to_string()
    } else {
        slug.to_string()
    };

    (
        format!("{base_url}/descargas/?compra=exito&sample={sample_id}"),
        format!("{base_url}/sample/{sample_path}/?compra=cancelado"),
    )
}

fn billing_portal_return_url(public_base_url: Option<&str>) -> String {
    let base_url = site_base_url(public_base_url);
    format!("{base_url}/planes/")
}

fn price_to_cents(price: f64) -> Result<i64, AppError> {
    format!("{price:.2}")
        .replace('.', "")
        .parse::<i64>()
        .map_err(|_| AppError::BadRequest("Precio invalido para checkout".to_string()))
}

#[cfg(test)]
mod tests {
    use super::{
        annual_price_cents, annual_savings_cents, billing_portal_return_url,
        build_payment_plans_response, price_to_cents, sample_checkout_urls, serialize_plan,
        subscription_checkout_urls,
    };
    use crate::domain::{kamples_plan_config, KamplesPlanId};

    #[test]
    fn free_plan_exposes_trial_fields() {
        let response = serialize_plan(kamples_plan_config(KamplesPlanId::Free), false);

        assert_eq!(response.precio_mensual_cents, 0);
        assert_eq!(response.prueba_gratuita_dias, Some(30));
        assert_eq!(response.descargas_prueba, Some(20));
        assert_eq!(response.revenue_share_label, "80/20");
    }

    #[test]
    fn paid_plan_uses_legacy_annual_discount() {
        let response = serialize_plan(kamples_plan_config(KamplesPlanId::Pro), true);

        assert_eq!(annual_price_cents(500), 5_000);
        assert_eq!(annual_savings_cents(500), 1_000);
        assert_eq!(response.precio_anual_cents, 5_000);
        assert_eq!(response.ahorro_anual_cents, 1_000);
        assert!(response.price_id_configurado);
    }

    #[test]
    fn response_marks_stripe_disabled_when_runtime_is_missing() {
        let response = build_payment_plans_response(None);

        assert!(!response.stripe_habilitado);
        assert!(response.publishable_key.is_none());
        assert_eq!(response.planes.len(), 3);
        assert_eq!(response.planes[0].id, KamplesPlanId::Free);
    }

    #[test]
    fn subscription_urls_match_legacy_shape() {
        let (success_url, cancel_url) = subscription_checkout_urls(Some("https://kamples.com/"));

        assert_eq!(success_url, "https://kamples.com/planes/?checkout=exito");
        assert_eq!(cancel_url, "https://kamples.com/planes/?checkout=cancelado");
        assert_eq!(
            billing_portal_return_url(Some("https://kamples.com/")),
            "https://kamples.com/planes/"
        );
    }

    #[test]
    fn sample_urls_fall_back_to_sample_id_when_slug_is_missing() {
        let (success_url, cancel_url) = sample_checkout_urls(None, 42, "");

        assert_eq!(
            success_url,
            "http://127.0.0.1:5183/descargas/?compra=exito&sample=42"
        );
        assert_eq!(
            cancel_url,
            "http://127.0.0.1:5183/sample/42/?compra=cancelado"
        );
    }

    #[test]
    fn price_to_cents_preserves_two_decimal_checkout_amounts() {
        assert_eq!(price_to_cents(19.99).expect("cents"), 1_999);
        assert_eq!(price_to_cents(5.0).expect("cents"), 500);
    }
}
