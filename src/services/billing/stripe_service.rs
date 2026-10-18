use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use stripe::{AccountId, ClientBuilder, RequestStrategy, StripeRequest};
use stripe_billing::billing_portal_session::CreateBillingPortalSession;
use stripe_checkout::checkout_session::{CreateCheckoutSession, CreateCheckoutSessionLineItems};
use stripe_checkout::CheckoutSessionMode;
use stripe_connect::account::{
    CapabilitiesParam, CapabilityParam, CreateAccount, CreateAccountType, RetrieveAccount,
};
use stripe_connect::account_link::{CreateAccountLink, CreateAccountLinkType};
use stripe_connect::login_link::CreateAccountLoginLink;
use stripe_core::balance::RetrieveForMyAccountBalance;
use stripe_core::customer::CreateCustomer;
use stripe_product::price::{CreatePrice, CreatePriceProductData};
use stripe_types::Currency;
use stripe_webhook::{Event, Webhook};
use utoipa::ToSchema;

use crate::config::AppConfig;
use crate::domain::{
    calculate_sample_revenue_share, format_price_cents, kamples_plan_catalog, kamples_plan_config,
    KamplesPlanConfig, KamplesPlanId, RevenueShareBreakdown,
};
use crate::errors::{AppError, AppResult};
use crate::repositories::{BillingRepository, StripeUserProfile};

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

impl StripeService {
    pub async fn get_or_create_customer_id(
        runtime: &StripeRuntime,
        pool: &sqlx::PgPool,
        user_id: i32,
    ) -> AppResult<String> {
        let profile = Self::load_user_profile(pool, user_id).await?;
        if let Some(customer_id) = profile.stripe_customer_id.clone() {
            return Ok(customer_id);
        }

        let email = profile.email.clone().ok_or_else(|| {
            AppError::Validation("El usuario no tiene email para crear customer de Stripe".into())
        })?;
        let display_name = display_name_for_stripe(&profile);

        let customer = CreateCustomer::new()
            .name(display_name)
            .email(email)
            .metadata(customer_metadata(profile.user_id, &profile.username))
            .send(&runtime.client)
            .await
            .map_err(|error| StripeRuntime::stripe_error("stripe.customer.create", error))?;

        let customer_id = customer.id.to_string();
        BillingRepository::save_stripe_customer_id(pool, user_id, &customer_id).await?;
        Ok(customer_id)
    }

    pub async fn create_subscription_checkout_session(
        runtime: &StripeRuntime,
        pool: &sqlx::PgPool,
        user_id: i32,
        plan: KamplesPlanId,
        success_url: &str,
        cancel_url: &str,
    ) -> AppResult<StripeCheckoutSessionSummary> {
        if plan == KamplesPlanId::Free {
            return Err(AppError::Validation(
                "El plan free no requiere checkout de Stripe".into(),
            ));
        }

        let price_id = runtime.price_id_for_plan(plan).ok_or_else(|| {
            AppError::Conflict(format!(
                "El plan {} no tiene price_id configurado en Stripe",
                plan.as_str()
            ))
        })?;
        let customer_id = Self::get_or_create_customer_id(runtime, pool, user_id).await?;

        let session = CreateCheckoutSession::new()
            .cancel_url(cancel_url)
            .customer(customer_id.as_str())
            .line_items(vec![CreateCheckoutSessionLineItems {
                quantity: Some(1),
                price: Some(price_id.to_string()),
                ..Default::default()
            }])
            .metadata(subscription_metadata(user_id, plan))
            .mode(CheckoutSessionMode::Subscription)
            .success_url(append_checkout_session_id(success_url))
            .send(&runtime.client)
            .await
            .map_err(|error| StripeRuntime::stripe_error("stripe.checkout.subscription", error))?;

        let url = session.url.ok_or_else(|| AppError::ExternalService {
            service: "stripe.checkout.subscription".to_string(),
            message: "Stripe no devolvió la URL de checkout".to_string(),
        })?;

        Ok(StripeCheckoutSessionSummary {
            id: session.id.to_string(),
            url,
        })
    }

    pub async fn create_sample_checkout_session(
        runtime: &StripeRuntime,
        pool: &sqlx::PgPool,
        user_id: i32,
        request: &StripeSampleCheckoutRequest,
    ) -> AppResult<StripeCheckoutSessionSummary> {
        if request.price_cents < 50 {
            return Err(AppError::Validation(
                "El precio mínimo para una compra individual es $0.50 USD".into(),
            ));
        }

        let customer_id = Self::get_or_create_customer_id(runtime, pool, user_id).await?;
        let price = CreatePrice::new(Currency::USD)
            .metadata(sample_checkout_metadata(user_id, request))
            .product_data(CreatePriceProductData::new(sample_title_for_stripe(
                &request.sample_title,
                request.sample_id,
            )))
            .unit_amount(request.price_cents)
            .send(&runtime.client)
            .await
            .map_err(|error| StripeRuntime::stripe_error("stripe.price.sample", error))?;

        let session = CreateCheckoutSession::new()
            .cancel_url(&request.cancel_url)
            .customer(customer_id.as_str())
            .line_items(vec![CreateCheckoutSessionLineItems {
                quantity: Some(1),
                price: Some(price.id.to_string()),
                ..Default::default()
            }])
            .metadata(sample_checkout_metadata(user_id, request))
            .mode(CheckoutSessionMode::Payment)
            .success_url(append_checkout_session_id(&request.success_url))
            .send(&runtime.client)
            .await
            .map_err(|error| StripeRuntime::stripe_error("stripe.checkout.sample", error))?;

        let url = session.url.ok_or_else(|| AppError::ExternalService {
            service: "stripe.checkout.sample".to_string(),
            message: "Stripe no devolvió la URL de checkout".to_string(),
        })?;

        Ok(StripeCheckoutSessionSummary {
            id: session.id.to_string(),
            url,
        })
    }

    pub async fn create_billing_portal_session(
        runtime: &StripeRuntime,
        pool: &sqlx::PgPool,
        user_id: i32,
        return_url: &str,
    ) -> AppResult<StripePortalSessionSummary> {
        let profile = Self::load_user_profile(pool, user_id).await?;
        let customer_id = profile.stripe_customer_id.ok_or_else(|| {
            AppError::Conflict("El usuario no tiene customer de Stripe asociado".into())
        })?;

        let session = CreateBillingPortalSession::new()
            .customer(customer_id)
            .return_url(return_url)
            .send(&runtime.client)
            .await
            .map_err(|error| StripeRuntime::stripe_error("stripe.portal", error))?;

        Ok(StripePortalSessionSummary { url: session.url })
    }

    pub async fn create_connect_account(
        runtime: &StripeRuntime,
        pool: &sqlx::PgPool,
        user_id: i32,
    ) -> AppResult<StripeConnectAccountSummary> {
        let profile = Self::load_user_profile(pool, user_id).await?;
        if let Some(connect_id) = profile.stripe_connect_id.as_deref() {
            BillingRepository::promote_user_to_creator(pool, user_id).await?;
            return Self::retrieve_connect_account(runtime, connect_id).await;
        }

        let email = profile.email.clone().ok_or_else(|| {
            AppError::Validation("El creador no tiene email para Stripe Connect".into())
        })?;

        let account = CreateAccount::new()
            .capabilities(CapabilitiesParam {
                card_payments: Some(CapabilityParam {
                    requested: Some(true),
                }),
                transfers: Some(CapabilityParam {
                    requested: Some(true),
                }),
                ..Default::default()
            })
            .email(email)
            .metadata(connect_metadata(profile.user_id, &profile.username))
            .type_(CreateAccountType::Express)
            .send(&runtime.client)
            .await
            .map_err(|error| StripeRuntime::stripe_error("stripe.connect.account", error))?;

        let account_id = account.id.to_string();
        BillingRepository::save_stripe_connect_id(pool, user_id, &account_id).await?;
        BillingRepository::promote_user_to_creator(pool, user_id).await?;

        Ok(StripeConnectAccountSummary {
            id: account_id,
            details_submitted: account.details_submitted,
            charges_enabled: account.charges_enabled,
            payouts_enabled: account.payouts_enabled,
            currently_due: account
                .requirements
                .as_ref()
                .and_then(|requirements| requirements.currently_due.clone())
                .unwrap_or_default(),
            disabled_reason: account
                .requirements
                .as_ref()
                .and_then(|requirements| requirements.disabled_reason.as_ref())
                .map(|reason| reason.as_str().to_string()),
        })
    }

    pub async fn retrieve_connect_account(
        runtime: &StripeRuntime,
        account_id: &str,
    ) -> AppResult<StripeConnectAccountSummary> {
        let account = RetrieveAccount::new(account_id)
            .send(&runtime.client)
            .await
            .map_err(|error| StripeRuntime::stripe_error("stripe.connect.retrieve", error))?;

        Ok(StripeConnectAccountSummary {
            id: account.id.to_string(),
            details_submitted: account.details_submitted,
            charges_enabled: account.charges_enabled,
            payouts_enabled: account.payouts_enabled,
            currently_due: account
                .requirements
                .as_ref()
                .and_then(|requirements| requirements.currently_due.clone())
                .unwrap_or_default(),
            disabled_reason: account
                .requirements
                .as_ref()
                .and_then(|requirements| requirements.disabled_reason.as_ref())
                .map(|reason| reason.as_str().to_string()),
        })
    }

    pub async fn create_connect_onboarding_link(
        runtime: &StripeRuntime,
        account_id: &str,
        return_url: &str,
        refresh_url: &str,
    ) -> AppResult<StripeConnectLinkSummary> {
        let link = CreateAccountLink::new(account_id, CreateAccountLinkType::AccountOnboarding)
            .refresh_url(refresh_url)
            .return_url(return_url)
            .send(&runtime.client)
            .await
            .map_err(|error| StripeRuntime::stripe_error("stripe.connect.onboarding", error))?;

        Ok(StripeConnectLinkSummary {
            url: link.url,
            expires_at: Some(link.expires_at),
        })
    }

    pub async fn create_connect_login_link(
        runtime: &StripeRuntime,
        account_id: &str,
    ) -> AppResult<StripeConnectLinkSummary> {
        let link = CreateAccountLoginLink::new(account_id)
            .send(&runtime.client)
            .await
            .map_err(|error| StripeRuntime::stripe_error("stripe.connect.login", error))?;

        Ok(StripeConnectLinkSummary {
            url: link.url,
            expires_at: None,
        })
    }

    pub async fn retrieve_connect_balance(
        runtime: &StripeRuntime,
        account_id: &str,
    ) -> AppResult<StripeConnectBalanceSummary> {
        let balance = RetrieveForMyAccountBalance::new()
            .customize()
            .account_id(AccountId::from(account_id))
            .send(&runtime.client)
            .await
            .map_err(|error| StripeRuntime::stripe_error("stripe.connect.balance", error))?;

        let available_cents = balance
            .available
            .iter()
            .map(|entry| entry.amount)
            .sum::<i64>();
        let pending_cents = balance
            .pending
            .iter()
            .map(|entry| entry.amount)
            .sum::<i64>();
        let currency = balance
            .available
            .first()
            .map(|entry| entry.currency.to_string())
            .or_else(|| {
                balance
                    .pending
                    .first()
                    .map(|entry| entry.currency.to_string())
            })
            .unwrap_or_else(|| "usd".to_string())
            .to_ascii_lowercase();

        Ok(StripeConnectBalanceSummary {
            available_cents,
            pending_cents,
            currency,
        })
    }

    pub async fn create_connect_payout(
        runtime: &StripeRuntime,
        account_id: &str,
        amount_cents: i64,
        currency: &str,
        creator_id: i32,
    ) -> AppResult<StripeConnectPayoutSummary> {
        let currency = currency.to_ascii_lowercase();
        let amount = amount_cents.to_string();
        let description = format!("Kamples payout creator {creator_id}");
        let creator_id = creator_id.to_string();
        let params = [
            ("amount", amount.as_str()),
            ("currency", currency.as_str()),
            ("description", description.as_str()),
            ("metadata[creator_id]", creator_id.as_str()),
            ("metadata[source]", "kamples_dashboard_payout"),
        ];

        let response = reqwest::Client::new()
            .post("https://api.stripe.com/v1/payouts")
            .bearer_auth(&runtime.secret_key)
            .header("Stripe-Account", account_id)
            .form(&params)
            .send()
            .await
            .map_err(|error| AppError::ExternalService {
                service: "stripe.connect.payout".to_string(),
                message: error.to_string(),
            })?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| AppError::ExternalService {
                service: "stripe.connect.payout".to_string(),
                message: error.to_string(),
            })?;
        if !status.is_success() {
            return Err(AppError::ExternalService {
                service: "stripe.connect.payout".to_string(),
                message: body,
            });
        }
        let payout: StripePayoutApiResponse =
            serde_json::from_str(&body).map_err(|error| AppError::ExternalService {
                service: "stripe.connect.payout".to_string(),
                message: error.to_string(),
            })?;

        Ok(StripeConnectPayoutSummary {
            id: payout.id,
            amount_cents: payout.amount,
            currency: payout.currency,
            status: payout.status,
        })
    }

    async fn load_user_profile(pool: &sqlx::PgPool, user_id: i32) -> AppResult<StripeUserProfile> {
        BillingRepository::find_stripe_user_profile(pool, user_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Usuario {user_id} no encontrado")))
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
