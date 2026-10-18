use axum::body::Bytes;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

use super::{price_to_cents, require_stripe_runtime};
use crate::domain::KamplesPlanId;
use crate::errors::AppError;
#[allow(unused_imports)]
use crate::errors::ErrorResponse;
use crate::models::PaymentWebhookResponse;
use crate::repositories::{
    BillingRepository, CompletedSamplePurchaseInsert, UpsertStripeSubscriptionRecord,
};
use crate::services::{IdempotencyStore, StripeRuntime, StripeWebhookSecretKind};
use crate::AppState;

const WEBHOOK_EVENT_TTL_SECS: u64 = 60 * 60 * 24 * 7;

#[derive(Debug, Deserialize)]
struct StripeWebhookEnvelope {
    id: String,
    #[serde(rename = "type")]
    event_type: String,
    data: StripeWebhookData,
}

#[derive(Debug, Deserialize)]
struct StripeWebhookData {
    object: Value,
}

#[utoipa::path(
    post,
    path = "/api/pagos/webhook",
    tag = "payments",
    params(("stripe-signature" = String, Header, description = "Firma HMAC Stripe")),
    responses(
        (status = 200, description = "Webhook recibido", body = PaymentWebhookResponse),
        (status = 400, description = "Firma o payload inválido", body = ErrorResponse),
        (status = 409, description = "Stripe no configurado", body = ErrorResponse)
    )
)]
pub async fn payment_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload: Bytes,
) -> Result<Json<PaymentWebhookResponse>, AppError> {
    let runtime = require_stripe_runtime(&state)?;
    let signature = headers
        .get("stripe-signature")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::BadRequest("Falta cabecera stripe-signature".to_string()))?;
    let payload_str = std::str::from_utf8(&payload)
        .map_err(|_| AppError::BadRequest("Payload Stripe inválido".to_string()))?;

    let _ = runtime.verify_webhook(payload_str, signature, StripeWebhookSecretKind::Payments)?;

    let envelope: StripeWebhookEnvelope = serde_json::from_slice(&payload)
        .map_err(|error| AppError::BadRequest(format!("Payload Stripe inválido: {error}")))?;

    if let Some(cached) = IdempotencyStore::get_json::<PaymentWebhookResponse>(
        &state.redis,
        "stripe-webhook",
        &envelope.id,
    )
    .await?
    {
        return Ok(Json(cached));
    }

    let response = process_payment_webhook(&state, runtime, &envelope).await?;
    IdempotencyStore::set_json(
        &state.redis,
        "stripe-webhook",
        &envelope.id,
        WEBHOOK_EVENT_TTL_SECS,
        &response,
    )
    .await?;

    Ok(Json(response))
}

async fn process_payment_webhook(
    state: &AppState,
    runtime: &StripeRuntime,
    envelope: &StripeWebhookEnvelope,
) -> Result<PaymentWebhookResponse, AppError> {
    let procesado = match envelope.event_type.as_str() {
        "checkout.session.completed" => {
            process_checkout_session_completed(state, runtime, &envelope.data.object, &envelope.id)
                .await?
        }
        "customer.subscription.updated" => {
            process_subscription_updated(state, runtime, &envelope.data.object).await?
        }
        "customer.subscription.deleted" => {
            process_subscription_deleted(state, runtime, &envelope.data.object).await?
        }
        _ => false,
    };

    Ok(PaymentWebhookResponse {
        recibido: true,
        procesado,
    })
}

async fn process_checkout_session_completed(
    state: &AppState,
    runtime: &StripeRuntime,
    object: &Value,
    event_id: &str,
) -> Result<bool, AppError> {
    if metadata_string(object, "tipo") == Some("compra_sample") {
        process_sample_checkout_completed(state, runtime, object, event_id).await
    } else {
        process_subscription_checkout_completed(state, object).await
    }
}

async fn process_subscription_checkout_completed(
    state: &AppState,
    object: &Value,
) -> Result<bool, AppError> {
    let Some(user_id) = metadata_i32(object, "user_id") else {
        return Ok(false);
    };
    let Some(plan) = metadata_string(object, "plan").and_then(parse_plan_id) else {
        return Ok(false);
    };

    BillingRepository::update_user_plan(&state.pool, user_id, plan.as_str()).await?;

    if let Some(subscription_id) = object_string(object, "subscription") {
        BillingRepository::upsert_subscription_from_stripe(
            &state.pool,
            &UpsertStripeSubscriptionRecord {
                user_id,
                plan: plan.as_str().to_string(),
                status: "activa".to_string(),
                stripe_subscription_id: subscription_id.to_string(),
                start_at: None,
                end_at: None,
            },
        )
        .await?;
    }

    Ok(true)
}

async fn process_subscription_updated(
    state: &AppState,
    runtime: &StripeRuntime,
    object: &Value,
) -> Result<bool, AppError> {
    let Some(customer_id) = object_string(object, "customer") else {
        return Ok(false);
    };
    let Some(profile) =
        BillingRepository::find_user_profile_by_customer_id(&state.pool, customer_id).await?
    else {
        return Ok(false);
    };

    let stripe_status = object_string(object, "status").unwrap_or("unknown");
    let current_plan = parse_plan_id(&profile.plan).unwrap_or(KamplesPlanId::Free);
    let subscription_plan = resolve_subscription_plan(object, runtime).unwrap_or(current_plan);
    let effective_plan = if is_active_subscription_status(stripe_status) {
        subscription_plan
    } else {
        KamplesPlanId::Free
    };

    BillingRepository::update_user_plan(&state.pool, profile.user_id, effective_plan.as_str())
        .await?;

    if let Some(subscription_id) = object_string(object, "id") {
        BillingRepository::upsert_subscription_from_stripe(
            &state.pool,
            &UpsertStripeSubscriptionRecord {
                user_id: profile.user_id,
                plan: subscription_plan.as_str().to_string(),
                status: map_subscription_status(stripe_status).to_string(),
                stripe_subscription_id: subscription_id.to_string(),
                start_at: timestamp_field(object, "current_period_start"),
                end_at: timestamp_field(object, "current_period_end"),
            },
        )
        .await?;
    }

    Ok(true)
}

async fn process_subscription_deleted(
    state: &AppState,
    runtime: &StripeRuntime,
    object: &Value,
) -> Result<bool, AppError> {
    let Some(customer_id) = object_string(object, "customer") else {
        return Ok(false);
    };
    let Some(profile) =
        BillingRepository::find_user_profile_by_customer_id(&state.pool, customer_id).await?
    else {
        return Ok(false);
    };

    BillingRepository::update_user_plan(&state.pool, profile.user_id, KamplesPlanId::Free.as_str())
        .await?;

    if let Some(subscription_id) = object_string(object, "id") {
        let subscription_plan = resolve_subscription_plan(object, runtime)
            .or_else(|| parse_plan_id(&profile.plan))
            .unwrap_or(KamplesPlanId::Free);
        BillingRepository::upsert_subscription_from_stripe(
            &state.pool,
            &UpsertStripeSubscriptionRecord {
                user_id: profile.user_id,
                plan: subscription_plan.as_str().to_string(),
                status: "cancelada".to_string(),
                stripe_subscription_id: subscription_id.to_string(),
                start_at: timestamp_field(object, "current_period_start"),
                end_at: timestamp_field(object, "current_period_end"),
            },
        )
        .await?;
    }

    Ok(true)
}

async fn process_sample_checkout_completed(
    state: &AppState,
    runtime: &StripeRuntime,
    object: &Value,
    event_id: &str,
) -> Result<bool, AppError> {
    let Some(user_id) = metadata_i32(object, "user_id") else {
        return Ok(false);
    };
    let Some(sample_id) = metadata_i32(object, "sample_id") else {
        return Ok(false);
    };
    let Some(creator_id) = metadata_i32(object, "creador_id") else {
        return Ok(false);
    };
    let Some(price_cents) = metadata_string(object, "precio").and_then(price_text_to_cents) else {
        return Ok(false);
    };

    let Some(sample) =
        BillingRepository::find_sample_checkout_candidate(&state.pool, sample_id).await?
    else {
        return Ok(false);
    };
    let Some(db_price) = sample.price else {
        return Ok(false);
    };
    let db_price_cents = price_to_cents(db_price)?;
    if !sample.is_premium || sample.creator_id != creator_id || db_price_cents != price_cents {
        return Ok(false);
    }

    let creator_plan = BillingRepository::find_stripe_user_profile(&state.pool, creator_id)
        .await?
        .and_then(|profile| parse_plan_id(&profile.plan))
        .unwrap_or(KamplesPlanId::Free);
    let revenue = runtime.calculate_sample_revenue_share(price_cents, creator_plan);
    let stripe_payment_id = object_string(object, "payment_intent");
    let idempotency_key = format!("stripe-webhook-{event_id}");
    let inserted = BillingRepository::insert_completed_sample_purchase(
        &state.pool,
        &CompletedSamplePurchaseInsert {
            buyer_id: user_id,
            creator_id,
            sample_id,
            amount_cents: price_cents,
            creator_amount_cents: revenue.creator_payout_cents,
            platform_fee_cents: revenue.platform_fee_cents,
            stripe_payment_id: stripe_payment_id.map(str::to_string),
            idempotency_key: Some(idempotency_key),
        },
    )
    .await?;

    Ok(inserted
        || BillingRepository::has_completed_sample_purchase(&state.pool, user_id, sample_id)
            .await?)
}

fn metadata_string<'a>(object: &'a Value, key: &str) -> Option<&'a str> {
    object
        .get("metadata")
        .and_then(|metadata| metadata.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn metadata_i32(object: &Value, key: &str) -> Option<i32> {
    metadata_string(object, key).and_then(|value| value.parse::<i32>().ok())
}

fn object_string<'a>(object: &'a Value, key: &str) -> Option<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn timestamp_field(object: &Value, key: &str) -> Option<DateTime<Utc>> {
    let timestamp = object.get(key).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str().and_then(|text| text.parse::<i64>().ok()))
    })?;
    DateTime::<Utc>::from_timestamp(timestamp, 0)
}

fn resolve_subscription_plan(object: &Value, runtime: &StripeRuntime) -> Option<KamplesPlanId> {
    metadata_string(object, "plan")
        .and_then(parse_plan_id)
        .or_else(|| {
            object
                .get("items")
                .and_then(|items| items.get("data"))
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("price"))
                .and_then(|price| {
                    price
                        .get("id")
                        .and_then(Value::as_str)
                        .and_then(|price_id| runtime.plan_from_price_id(price_id))
                        .or_else(|| {
                            price
                                .get("lookup_key")
                                .and_then(Value::as_str)
                                .and_then(plan_from_lookup_key)
                        })
                })
        })
}

fn plan_from_lookup_key(lookup_key: &str) -> Option<KamplesPlanId> {
    let lookup_key = lookup_key.trim().to_ascii_lowercase();
    if lookup_key.contains("premium") {
        Some(KamplesPlanId::Premium)
    } else if lookup_key.contains("pro") {
        Some(KamplesPlanId::Pro)
    } else if lookup_key.contains("free") {
        Some(KamplesPlanId::Free)
    } else {
        None
    }
}

fn parse_plan_id(plan: &str) -> Option<KamplesPlanId> {
    match plan.trim().to_ascii_lowercase().as_str() {
        "free" => Some(KamplesPlanId::Free),
        "pro" => Some(KamplesPlanId::Pro),
        "premium" => Some(KamplesPlanId::Premium),
        _ => None,
    }
}

fn is_active_subscription_status(status: &str) -> bool {
    matches!(status, "active" | "trialing")
}

fn map_subscription_status(status: &str) -> &'static str {
    match status {
        "trialing" => "periodo_prueba",
        "active" => "activa",
        "canceled" => "cancelada",
        _ => "vencida",
    }
}

fn price_text_to_cents(price: &str) -> Option<i64> {
    price
        .trim()
        .parse::<f64>()
        .ok()
        .and_then(|amount| price_to_cents(amount).ok())
}

#[cfg(test)]
mod tests {
    use super::{
        map_subscription_status, parse_plan_id, plan_from_lookup_key, price_text_to_cents,
    };
    use crate::domain::KamplesPlanId;

    #[test]
    fn parse_plan_id_only_accepts_known_values() {
        assert_eq!(parse_plan_id("premium"), Some(KamplesPlanId::Premium));
        assert_eq!(parse_plan_id("enterprise"), None);
    }

    #[test]
    fn lookup_key_plan_resolution_is_case_insensitive() {
        assert_eq!(
            plan_from_lookup_key("KAMPLES_PRO_MONTHLY"),
            Some(KamplesPlanId::Pro)
        );
    }

    #[test]
    fn subscription_status_maps_to_domain_values() {
        assert_eq!(map_subscription_status("trialing"), "periodo_prueba");
        assert_eq!(map_subscription_status("past_due"), "vencida");
    }

    #[test]
    fn price_text_to_cents_parses_decimal_metadata() {
        assert_eq!(price_text_to_cents("19.99"), Some(1_999));
    }
}
