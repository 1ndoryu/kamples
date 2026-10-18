use stripe::{AccountId, StripeRequest};
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

use crate::domain::KamplesPlanId;
use crate::errors::{AppError, AppResult};
use crate::repositories::{BillingRepository, StripeUserProfile};

use super::{
    append_checkout_session_id, connect_metadata, customer_metadata, display_name_for_stripe,
    sample_checkout_metadata, sample_title_for_stripe, subscription_metadata,
    StripeCheckoutSessionSummary, StripeConnectAccountSummary, StripeConnectBalanceSummary,
    StripeConnectLinkSummary, StripeConnectPayoutSummary, StripePayoutApiResponse,
    StripePortalSessionSummary, StripeRuntime, StripeSampleCheckoutRequest, StripeService,
};

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
