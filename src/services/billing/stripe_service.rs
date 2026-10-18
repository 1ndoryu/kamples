mod service;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use stripe::{ClientBuilder, RequestStrategy};
use stripe_webhook::{Event, Webhook};
use utoipa::ToSchema;

use crate::config::AppConfig;
use crate::domain::{
    calculate_sample_revenue_share, format_price_cents, kamples_plan_catalog, kamples_plan_config,
    KamplesPlanConfig, KamplesPlanId, RevenueShareBreakdown,
};
use crate::errors::{AppError, AppResult};
use crate::repositories::StripeUserProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StripeWebhookSecretKind {
    Payments,
    Connect,
}

#[derive(Debug, Clone, Default)]
pub struct StripePriceCatalog {
    pub free: Option<String>,
    pub pro: Option<String>,
    pub premium: Option<String>,
}

impl StripePriceCatalog {
    pub fn resolve(&self, plan: KamplesPlanId) -> Option<&str> {
        match plan {
            KamplesPlanId::Free => self.free.as_deref(),
            KamplesPlanId::Pro => self.pro.as_deref(),
            KamplesPlanId::Premium => self.premium.as_deref(),
        }
    }
}

#[derive(Clone)]
pub struct StripeRuntime {
    client: stripe::Client,
    secret_key: String,
    publishable_key: Option<String>,
    webhook_secret: Option<String>,
    connect_webhook_secret: Option<String>,
    prices: StripePriceCatalog,
}

#[derive(Debug, thiserror::Error)]
pub enum StripeRuntimeError {
    #[error("Configuración Stripe incompleta: falta la secret key ({0})")]
    PartialConfig(&'static str),
    #[error("No se pudo inicializar el cliente Stripe: {0}")]
    ClientInitialization(String),
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StripeCheckoutSessionSummary {
    pub id: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StripePortalSessionSummary {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StripeConnectAccountSummary {
    pub id: String,
    pub details_submitted: Option<bool>,
    pub charges_enabled: Option<bool>,
    pub payouts_enabled: Option<bool>,
    pub currently_due: Vec<String>,
    pub disabled_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct StripeConnectLinkSummary {
    pub url: String,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct StripeConnectBalanceSummary {
    pub available_cents: i64,
    pub pending_cents: i64,
    pub currency: String,
}

#[derive(Debug, Clone)]
pub struct StripeConnectPayoutSummary {
    pub id: String,
    pub amount_cents: i64,
    pub currency: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
struct StripePayoutApiResponse {
    id: String,
    amount: i64,
    currency: String,
    status: String,
}

#[derive(Debug, Clone)]
pub struct StripeSampleCheckoutRequest {
    pub sample_id: i32,
    pub creator_id: i32,
    pub sample_title: String,
    pub price_cents: i64,
    pub success_url: String,
    pub cancel_url: String,
}

pub struct StripeService;

/* [174A-79] Wrapper Stripe específico de Kamples.
 * Centraliza planes, customer lookup, checkout, portal, Connect y webhook para
 * que las siguientes fases sólo conecten handlers/repositorio sin duplicar HTTP.
 * Gotcha: el catálogo de precios sigue leyendo GLORY_STRIPE_/STRIPE_ porque el
 * legado mezcla ambos prefijos. Pendiente: 174A-82 mover webhooks/suscripciones
 * a un flujo totalmente idempotente con persistencia transaccional. */

impl StripeRuntime {
    pub fn from_config(config: &AppConfig) -> Result<Option<Self>, StripeRuntimeError> {
        let stripe = &config.stripe;

        let Some(secret_key) = stripe.secret_key.clone() else {
            if has_any_stripe_env() {
                return Err(StripeRuntimeError::PartialConfig(
                    "GLORY_STRIPE_SECRET_KEY / STRIPE_SECRET_KEY",
                ));
            }
            return Ok(None);
        };

        let client = ClientBuilder::new(secret_key.clone())
            .request_strategy(RequestStrategy::Retry(3))
            .build()
            .map_err(|error| StripeRuntimeError::ClientInitialization(error.to_string()))?;

        Ok(Some(Self {
            client,
            secret_key,
            publishable_key: stripe.publishable_key.clone(),
            webhook_secret: stripe.webhook_secret.clone(),
            connect_webhook_secret: stripe.connect_webhook_secret.clone(),
            prices: StripePriceCatalog {
                free: stripe.prices.free.clone(),
                pro: stripe.prices.pro.clone(),
                premium: stripe.prices.premium.clone(),
            },
        }))
    }

    pub fn publishable_key(&self) -> Option<&str> {
        self.publishable_key.as_deref()
    }

    pub fn plan_catalog(&self) -> &'static [KamplesPlanConfig; 3] {
        kamples_plan_catalog()
    }

    pub fn price_id_for_plan(&self, plan: KamplesPlanId) -> Option<&str> {
        self.prices.resolve(plan)
    }

    pub fn plan_from_price_id(&self, price_id: &str) -> Option<KamplesPlanId> {
        [KamplesPlanId::Pro, KamplesPlanId::Premium]
            .into_iter()
            .find(|plan| self.price_id_for_plan(*plan) == Some(price_id))
    }

    pub fn calculate_sample_revenue_share(
        &self,
        price_cents: i64,
        plan: KamplesPlanId,
    ) -> RevenueShareBreakdown {
        let _ = self;
        calculate_sample_revenue_share(price_cents, plan)
    }

    pub fn verify_webhook(
        &self,
        payload: &str,
        signature: &str,
        kind: StripeWebhookSecretKind,
    ) -> AppResult<Event> {
        let secret = self
            .resolve_webhook_secret(kind)
            .ok_or_else(|| AppError::Conflict("Stripe webhook no está configurado".to_string()))?;

        Webhook::construct_event(payload, signature, secret)
            .map_err(|error| AppError::BadRequest(format!("Firma Stripe inválida: {error}")))
    }

    fn resolve_webhook_secret(&self, kind: StripeWebhookSecretKind) -> Option<&str> {
        match kind {
            StripeWebhookSecretKind::Payments => self.webhook_secret.as_deref(),
            StripeWebhookSecretKind::Connect => self
                .connect_webhook_secret
                .as_deref()
                .or(self.webhook_secret.as_deref()),
        }
    }

    fn stripe_error(service: &str, error: impl std::fmt::Display) -> AppError {
        AppError::ExternalService {
            service: service.to_string(),
            message: error.to_string(),
        }
    }
}

fn has_any_stripe_env() -> bool {
    [
        "GLORY_STRIPE_SECRET_KEY",
        "STRIPE_SECRET_KEY",
        "GLORY_STRIPE_PUBLISHABLE_KEY",
        "STRIPE_PUBLISHABLE_KEY",
        "GLORY_STRIPE_WEBHOOK_SECRET",
        "STRIPE_WEBHOOK_SECRET",
        "GLORY_STRIPE_PRICE_PRO",
        "STRIPE_PRICE_PRO",
        "GLORY_STRIPE_PRICE_PREMIUM",
        "STRIPE_PRICE_PREMIUM",
    ]
    .iter()
    .any(|name| std::env::var_os(name).is_some())
}

fn display_name_for_stripe(profile: &StripeUserProfile) -> String {
    let display_name = profile.display_name.trim();
    if display_name.is_empty() {
        profile.username.clone()
    } else {
        display_name.to_string()
    }
}

fn sample_title_for_stripe(sample_title: &str, sample_id: i32) -> String {
    let title = sample_title.trim();
    if title.is_empty() {
        format!("Sample {sample_id}")
    } else {
        title.to_string()
    }
}

fn customer_metadata(user_id: i32, username: &str) -> HashMap<String, String> {
    HashMap::from([
        ("user_id".to_string(), user_id.to_string()),
        ("username".to_string(), username.to_string()),
    ])
}

fn subscription_metadata(user_id: i32, plan: KamplesPlanId) -> HashMap<String, String> {
    HashMap::from([
        ("user_id".to_string(), user_id.to_string()),
        ("plan".to_string(), plan.as_str().to_string()),
        (
            "precio_mensual".to_string(),
            format_price_cents(kamples_plan_config(plan).monthly_price_cents),
        ),
    ])
}

fn connect_metadata(user_id: i32, username: &str) -> HashMap<String, String> {
    HashMap::from([
        ("user_id".to_string(), user_id.to_string()),
        ("username".to_string(), username.to_string()),
    ])
}

fn sample_checkout_metadata(
    user_id: i32,
    request: &StripeSampleCheckoutRequest,
) -> HashMap<String, String> {
    HashMap::from([
        ("tipo".to_string(), "compra_sample".to_string()),
        ("user_id".to_string(), user_id.to_string()),
        ("sample_id".to_string(), request.sample_id.to_string()),
        ("creador_id".to_string(), request.creator_id.to_string()),
        (
            "precio".to_string(),
            format_price_cents(request.price_cents),
        ),
    ])
}

fn append_checkout_session_id(url: &str) -> String {
    let separator = if url.contains('?') { '&' } else { '?' };
    format!("{url}{separator}session_id={{CHECKOUT_SESSION_ID}}")
}

#[cfg(test)]
mod tests {
    use super::append_checkout_session_id;

    #[test]
    fn append_checkout_session_id_uses_ampersand_when_query_exists() {
        assert_eq!(
            append_checkout_session_id("https://kamples.com/planes/?checkout=exito"),
            "https://kamples.com/planes/?checkout=exito&session_id={CHECKOUT_SESSION_ID}"
        );
    }

    #[test]
    fn append_checkout_session_id_uses_question_mark_when_query_is_missing() {
        assert_eq!(
            append_checkout_session_id("https://kamples.com/planes/"),
            "https://kamples.com/planes/?session_id={CHECKOUT_SESSION_ID}"
        );
    }
}
