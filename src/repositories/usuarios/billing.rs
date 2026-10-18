use chrono::{DateTime, Utc};

mod compras;
mod suscripciones;

/* [174A-79] BillingRepository concentra las columnas Stripe/pagos del dominio Kamples.
 * El wrapper usa este repositorio para evitar que checkout, portal y Connect accedan
 * a `usuarios_ext`/`suscripciones` con SQL duplicado. 174A-82 suma además
 * lookup por customer, actualización de plan y transacciones idempotentes. */

#[derive(Debug, Clone)]
pub struct StripeUserProfile {
    pub user_id: i32,
    pub username: String,
    pub email: Option<String>,
    pub display_name: String,
    pub plan: String,
    pub stripe_customer_id: Option<String>,
    pub stripe_connect_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SubscriptionRecord {
    pub id: i32,
    pub user_id: i32,
    pub plan: String,
    pub status: String,
    pub stripe_subscription_id: Option<String>,
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct UpsertStripeSubscriptionRecord {
    pub user_id: i32,
    pub plan: String,
    pub status: String,
    pub stripe_subscription_id: String,
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct SampleCheckoutCandidate {
    pub sample_id: i32,
    pub sample_title: String,
    pub slug: String,
    pub creator_id: i32,
    pub is_premium: bool,
    pub price: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct CompletedSamplePurchaseInsert {
    pub buyer_id: i32,
    pub creator_id: i32,
    pub sample_id: i32,
    pub amount_cents: i64,
    pub creator_amount_cents: i64,
    pub platform_fee_cents: i64,
    pub stripe_payment_id: Option<String>,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CompletedDownloadRevenueShareInsert {
    pub buyer_id: i32,
    pub creator_id: i32,
    pub sample_id: i32,
    pub amount_cents: i64,
    pub creator_amount_cents: i64,
    pub platform_fee_cents: i64,
}

#[derive(Debug, Clone)]
pub struct CreatorPayoutInsert {
    pub creator_id: i32,
    pub amount_cents: i64,
    pub currency: String,
    pub status: String,
    pub stripe_payout_id: String,
}

pub struct BillingRepository;

fn cents_to_decimal_string(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let cents = cents.abs();
    let units = cents / 100;
    let fractional = cents % 100;
    format!("{sign}{units}.{fractional:02}")
}
