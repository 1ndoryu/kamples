use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::errors::AppError;
#[allow(unused_imports)]
use crate::errors::ErrorResponse;
use crate::middleware::CurrentUser;
use crate::models::{
    CreatorDashboardIncomePoint, CreatorDashboardIncomeQuery, CreatorDashboardSampleStat,
    CreatorDashboardStats, CreatorDashboardTransaction, CreatorDashboardTransactionsQuery,
    CreatorPayoutResponse,
};
use crate::repositories::{BillingRepository, CreatorDashboardRepository, CreatorPayoutInsert};
use crate::services::{StripeRuntime, StripeService};
use crate::AppState;

#[utoipa::path(
    get,
    path = "/api/dashboard/stats",
    tag = "payments",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "KPIs del dashboard de creador", body = CreatorDashboardStats),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 404, description = "Usuario no encontrado", body = ErrorResponse)
    )
)]
pub async fn stats(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<CreatorDashboardStats>, AppError> {
    ensure_user_exists(&state, user.user_id).await?;
    let response = CreatorDashboardRepository::stats(&state.pool, user.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Usuario {} no encontrado", user.user_id)))?;
    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/dashboard/top-samples",
    tag = "payments",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Top samples del creador", body = [CreatorDashboardSampleStat]),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 404, description = "Usuario no encontrado", body = ErrorResponse)
    )
)]
pub async fn top_samples(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<Vec<CreatorDashboardSampleStat>>, AppError> {
    ensure_user_exists(&state, user.user_id).await?;
    Ok(Json(
        CreatorDashboardRepository::top_samples(&state.pool, user.user_id).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/dashboard/transacciones",
    tag = "payments",
    params(CreatorDashboardTransactionsQuery),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Historial de transacciones del creador", body = [CreatorDashboardTransaction]),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 404, description = "Usuario no encontrado", body = ErrorResponse)
    )
)]
pub async fn transactions(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<CreatorDashboardTransactionsQuery>,
) -> Result<Json<Vec<CreatorDashboardTransaction>>, AppError> {
    ensure_user_exists(&state, user.user_id).await?;
    Ok(Json(
        CreatorDashboardRepository::transactions(&state.pool, user.user_id, &query).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/dashboard/ingresos",
    tag = "payments",
    params(CreatorDashboardIncomeQuery),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Serie de ingresos del dashboard de creador", body = [CreatorDashboardIncomePoint]),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 404, description = "Usuario no encontrado", body = ErrorResponse)
    )
)]
pub async fn income_series(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<CreatorDashboardIncomeQuery>,
) -> Result<Json<Vec<CreatorDashboardIncomePoint>>, AppError> {
    ensure_user_exists(&state, user.user_id).await?;
    Ok(Json(
        CreatorDashboardRepository::income_series(&state.pool, user.user_id, &query).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/dashboard/payout",
    tag = "payments",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Payout solicitado al balance disponible de Stripe Connect", body = CreatorPayoutResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 404, description = "Usuario no encontrado", body = ErrorResponse),
        (status = 409, description = "Stripe/Connect no configurado o payout no disponible", body = ErrorResponse)
    )
)]
pub async fn request_payout(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<CreatorPayoutResponse>, AppError> {
    let profile = BillingRepository::find_stripe_user_profile(&state.pool, user.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Usuario {} no encontrado", user.user_id)))?;

    let connect_id = profile.stripe_connect_id.ok_or_else(|| {
        AppError::Conflict("Configura Stripe Connect antes de solicitar payouts".to_string())
    })?;
    let runtime = require_stripe_runtime(&state)?;
    let account = StripeService::retrieve_connect_account(runtime, &connect_id).await?;
    if !account.payouts_enabled.unwrap_or(false) {
        return Err(AppError::Conflict(
            "La cuenta de Stripe Connect aún no tiene payouts activos".to_string(),
        ));
    }
    if BillingRepository::has_pending_creator_payout(&state.pool, user.user_id).await? {
        return Err(AppError::Conflict(
            "Ya hay un payout pendiente para este creador".to_string(),
        ));
    }

    let balance = StripeService::retrieve_connect_balance(runtime, &connect_id).await?;
    if balance.available_cents <= 0 {
        return Err(AppError::Conflict(
            "No hay balance disponible para retirar".to_string(),
        ));
    }

    let payout = StripeService::create_connect_payout(
        runtime,
        &connect_id,
        balance.available_cents,
        &balance.currency,
        user.user_id,
    )
    .await?;
    let status = stripe_payout_status_to_db(&payout.status).to_string();
    BillingRepository::insert_creator_payout(
        &state.pool,
        &CreatorPayoutInsert {
            creator_id: user.user_id,
            amount_cents: payout.amount_cents,
            currency: payout.currency.clone(),
            status: status.clone(),
            stripe_payout_id: payout.id,
        },
    )
    .await?;

    Ok(Json(CreatorPayoutResponse {
        monto: cents_to_major_units(payout.amount_cents),
        estado: status,
    }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/dashboard/stats", get(stats))
        .route("/dashboard/top-samples", get(top_samples))
        .route("/dashboard/transacciones", get(transactions))
        .route("/dashboard/ingresos", get(income_series))
        .route("/dashboard/payout", post(request_payout))
}

async fn ensure_user_exists(state: &AppState, user_id: i32) -> Result<(), AppError> {
    let exists = BillingRepository::find_stripe_user_profile(&state.pool, user_id)
        .await?
        .is_some();
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound(format!(
            "Usuario {user_id} no encontrado"
        )))
    }
}

fn require_stripe_runtime(state: &AppState) -> Result<&StripeRuntime, AppError> {
    state
        .stripe_runtime
        .as_deref()
        .ok_or_else(|| AppError::Conflict("Stripe no esta configurado en este entorno".to_string()))
}

fn cents_to_major_units(cents: i64) -> f64 {
    let sign = if cents < 0 { "-" } else { "" };
    let cents_abs = cents.abs();
    let units = cents_abs / 100;
    let fraction = cents_abs % 100;
    format!("{sign}{units}.{fraction:02}")
        .parse::<f64>()
        .unwrap_or(0.0)
}

fn stripe_payout_status_to_db(status: &str) -> &'static str {
    match status {
        "paid" => "completada",
        "failed" | "canceled" => "fallida",
        _ => "pendiente",
    }
}
