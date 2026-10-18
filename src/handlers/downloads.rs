use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::free_codes::normalize_optional_free_code;
use crate::algorithm::InteractionKind;
use crate::domain::{calculate_subscription_download_revenue_share, KamplesPlanId};
use crate::errors::AppError;
use crate::middleware::CurrentUser;
use crate::models::{DownloadGrantRequest, SampleSummary, SyncChangelogTipo};
use crate::repositories::{
    BillingRepository, CompletedDownloadRevenueShareInsert, DownloadRepository, FreeCodeRepository,
    SampleDownloadInfo, SyncChangelogRepository,
};
use crate::services::download_token;
use crate::services::SampleCatalogService;
use crate::AppState;

/* [174A-61] Downloads — port mínimo de DescargasController.php.
 *
 * Endpoints:
 * - POST /api/samples/:id/descargar → registra descarga (consume crédito si
 *   aplica), retorna URL relativa del archivo y restantes del día.
 * - GET  /api/descargas/limites    → informa límite del plan + restantes hoy.
 *
 * Reglas portadas (mínimas):
 * - Sample debe existir y permitir descarga (a menos que el usuario sea
 *   creador) → 403 si no permite.
 * - Re-descargas y samples propios NO consumen crédito.
 * - Plan free: límite base 5/día + `creditos_bonus`. Plan pro/premium: ilimitado.
 * - Sample con `precio > 0` y usuario no comprador y no plan pro+ → 403
 *   `requiere_compra`.
 * - Sample `es_premium` sin precio → requiere plan pro+ (403 si free).
 * - Compra individual completada en `transacciones` habilita descarga sin
 *   consumir crédito aunque el usuario siga en plan free.
 * - `codigoGratis` opcional salta checks de plan/crédito/precio si ya fue
 *   reclamado y sigue activo para este sample.
 * - Trigger AlgoPlanner Descarga al registrar.
 *
 * NO portado:
 * - Anti-abuso por IP.
 * - Calidad por plan (siempre se sirve `wav` legacy).
 *
 * [174A-63] Stream firmado HMAC:
 * - `GET /api/descargas/stream?token=...` valida token (HMAC sha256, exp 5min)
 *   y entrega el archivo desde el storage abstracto. NO requiere Authorization
 *   header (el token es la auth) — se puede usar desde un <a href> directo.
 * - `register_download` ahora devuelve URL absoluta `/api/descargas/stream?token=...`.
 * - Sin soporte de Range todavía (se sirve el archivo completo via storage.get_bytes).
 */

const PLAN_LIMIT_FREE: i64 = 5;

fn plan_daily_limit(plan: &str) -> Option<i64> {
    match plan {
        "free" => Some(PLAN_LIMIT_FREE),
        _ => None, // ilimitado
    }
}

fn effective_daily_limit(base_limit: Option<i64>, bonus_credits: i32) -> Option<i64> {
    base_limit.map(|limit| limit.saturating_add(i64::from(bonus_credits.max(0))))
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DownloadResponse {
    pub ok: bool,
    pub url: String,
    pub calidad: String,
    pub consume_credito: bool,
    pub restantes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DownloadLimitsResponse {
    pub plan: String,
    pub limite: Option<i64>,
    pub limite_base: Option<i64>,
    pub creditos_bonus: i32,
    pub usadas: i64,
    pub restantes: Option<i64>,
    pub calidad: String,
}

/* [274A-4] Respuesta de /descargas/comprados — replica el contrato legacy
 * `{ ok: true, data: SampleResumen[] }`. El frontend desempaqueta `data.data`
 * (ver legacy/services/apiDescargas.ts::obtenerComprados). */
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PurchasedSamplesResponse {
    pub ok: bool,
    pub data: Vec<SampleSummary>,
}

const PURCHASED_SAMPLES_LIMIT: i64 = 50;

#[utoipa::path(
    post,
    path = "/api/samples/{id}/descargar",
    tag = "downloads",
    params(("id" = i32, Path, description = "ID del sample a descargar")),
    request_body = Option<DownloadGrantRequest>,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Descarga registrada", body = DownloadResponse),
        (status = 401, description = "No autenticado"),
        (status = 403, description = "Sample bloqueado para el usuario"),
        (status = 404, description = "Sample no encontrado"),
        (status = 429, description = "Límite diario excedido"),
    )
)]
pub async fn register_download(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(sample_id): Path<i32>,
    payload: Option<Json<DownloadGrantRequest>>,
) -> Result<Json<DownloadResponse>, AppError> {
    let access_request = payload.map(|Json(value)| value).unwrap_or_default();
    let info = DownloadRepository::fetch_sample_info(&state.pool, sample_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("sample {sample_id} no existe")))?;

    let es_propietario = info.creador_id == user.user_id;
    if !info.permitir_descarga && !es_propietario {
        return Err(AppError::Forbidden(
            "Este sample no permite descargas".into(),
        ));
    }

    let allowance = DownloadRepository::user_download_allowance(&state.pool, user.user_id).await?;
    let bonus_credits = allowance.bonus_credits;
    let plan = allowance.plan;
    let codigo_gratis = normalize_optional_free_code(access_request.codigo_gratis)?;
    let es_codigo_gratis = if let Some(code) = codigo_gratis.as_deref() {
        !es_propietario
            && FreeCodeRepository::can_user_download(
                &state.pool,
                code,
                "sample",
                i64::from(sample_id),
                user.user_id,
            )
            .await?
    } else {
        false
    };
    let ya_comprado = !es_propietario
        && BillingRepository::has_completed_sample_purchase(&state.pool, user.user_id, sample_id)
            .await?;
    let ya_descargado = !es_propietario
        && DownloadRepository::already_downloaded(&state.pool, user.user_id, sample_id).await?;
    let mut consume_credito = !es_propietario && !ya_descargado && !ya_comprado;
    if es_codigo_gratis {
        consume_credito = false;
    }

    /* Solo el plan premium cubre la descarga sin consumir crédito; el resto se rechaza. */
    if !es_propietario
        && !es_codigo_gratis
        && cubre_plan_premium(&info, &plan, ya_descargado, ya_comprado)?
    {
        consume_credito = false;
    }

    let limite_base = plan_daily_limit(&plan);
    let limite = effective_daily_limit(limite_base, bonus_credits);
    let usadas = DownloadRepository::count_today(&state.pool, user.user_id).await?;
    let restantes = limite.map(|l| (l - usadas).max(0));

    if consume_credito {
        if let Some(limit) = limite {
            if usadas >= limit {
                return Err(AppError::RateLimited);
            }
        }
    }

    registrar_efectos_descarga(&state, user.user_id, sample_id).await?;

    if plan != "free" && !es_propietario {
        reparto_descarga_creador(&state, user.user_id, info.creador_id, sample_id, &plan).await;
    }

    let restantes_post = limite.map(|l| (l - usadas - i64::from(consume_credito)).max(0));

    /* [174A-63] Token firmado HMAC válido 5 min para servir el archivo. */
    let token = download_token::generate(sample_id, user.user_id, 300, &state.jwt_secret);
    let url = format!("/api/descargas/stream?token={token}");

    Ok(Json(DownloadResponse {
        ok: true,
        url,
        calidad: "wav".into(),
        consume_credito,
        restantes: restantes_post.or(restantes),
    }))
}

/* Reglas de compra y plan para descargar. Devuelve `true` si el plan premium cubre
 * la descarga sin consumir crédito; `Err` si el sample exige compra o plan superior. */
fn cubre_plan_premium(
    info: &SampleDownloadInfo,
    plan: &str,
    ya_descargado: bool,
    ya_comprado: bool,
) -> Result<bool, AppError> {
    let precio = info.precio.unwrap_or(0.0);
    if precio > 0.0 && !ya_descargado && !ya_comprado {
        if info.es_premium && plan != "free" {
            return Ok(true);
        }
        return Err(AppError::Forbidden(format!(
            "Este sample requiere compra individual (precio {precio:.2})"
        )));
    }
    if info.es_premium && plan == "free" && !ya_comprado {
        return Err(AppError::Forbidden(
            "Se requiere plan Pro o Premium para descargar este sample".into(),
        ));
    }
    Ok(false)
}

/* Efectos de una descarga registrada: contador diario, interacción del planner
 * y entrada en el changelog de sync (sin bloquear si falla). */
async fn registrar_efectos_descarga(
    state: &AppState,
    user_id: i32,
    sample_id: i32,
) -> Result<(), AppError> {
    DownloadRepository::register(&state.pool, user_id, sample_id, "wav").await?;

    state
        .algo_planner
        .register_interaction(&state.pool, &state.redis, user_id, InteractionKind::Descarga)
        .await?;

    /* [246A-1] Registrar en changelog para delta sync del desktop. */
    SyncChangelogRepository::insert_entry(
        &state.pool,
        user_id,
        SyncChangelogTipo::SampleAdded,
        sample_id,
        serde_json::json!({"origen": "download"}),
    )
    .await
    .ok();
    Ok(())
}

/* El reparto al creador no debe bloquear la descarga: si falla, solo se registra el error. */
async fn reparto_descarga_creador(
    state: &AppState,
    buyer_id: i32,
    creator_id: i32,
    sample_id: i32,
    plan: &str,
) {
    if let Err(error) =
        register_download_revenue_share(state, buyer_id, creator_id, sample_id, plan).await
    {
        tracing::error!(
            buyer_id,
            creator_id,
            sample_id,
            plan = %plan,
            error = %error,
            "revenue share fallido para descarga"
        );
    }
}

#[utoipa::path(
    get,
    path = "/api/descargas/limites",
    tag = "downloads",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Límites del plan", body = DownloadLimitsResponse),
        (status = 401, description = "No autenticado"),
    )
)]
pub async fn download_limits(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<DownloadLimitsResponse>, AppError> {
    let allowance = DownloadRepository::user_download_allowance(&state.pool, user.user_id).await?;
    let bonus_credits = allowance.bonus_credits;
    let plan = allowance.plan;
    let limite_base = plan_daily_limit(&plan);
    let limite = effective_daily_limit(limite_base, bonus_credits);
    let usadas = DownloadRepository::count_today(&state.pool, user.user_id).await?;
    let restantes = limite.map(|l| (l - usadas).max(0));
    Ok(Json(DownloadLimitsResponse {
        plan,
        limite,
        limite_base,
        creditos_bonus: bonus_credits,
        usadas,
        restantes,
        calidad: "wav".into(),
    }))
}

/* [274A-4] GET /api/descargas/comprados
 * Lista samples comprados (compra individual completada) por el usuario,
 * ordenados por compra más reciente primero. Reusa
 * `SampleCatalogService::list_public_samples_by_ids` para construir summaries
 * con los flags y assets normalizados que espera el frontend. El campo
 * `ya_comprado` se calcula on-the-fly por el catálogo cuando hay
 * `current_user_id`, así que aquí solo aportamos los IDs en el orden correcto. */
#[utoipa::path(
    get,
    path = "/api/descargas/comprados",
    tag = "downloads",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Samples comprados por el usuario", body = PurchasedSamplesResponse),
        (status = 401, description = "No autenticado"),
    )
)]
pub async fn list_purchased_samples(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<PurchasedSamplesResponse>, AppError> {
    let sample_ids = BillingRepository::list_purchased_sample_ids(
        &state.pool,
        user.user_id,
        PURCHASED_SAMPLES_LIMIT,
    )
    .await?;

    if sample_ids.is_empty() {
        return Ok(Json(PurchasedSamplesResponse {
            ok: true,
            data: Vec::new(),
        }));
    }

    let summaries = SampleCatalogService::list_public_samples_by_ids(
        &state.pool,
        state.public_base_url.as_deref(),
        Some(user.user_id),
        &sample_ids,
    )
    .await?;

    Ok(Json(PurchasedSamplesResponse {
        ok: true,
        data: summaries,
    }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/samples/:id/descargar", post(register_download))
        .route("/descargas/limites", get(download_limits))
        .route("/descargas/comprados", get(list_purchased_samples))
        .route("/descargas/stream", get(stream_download))
}

async fn register_download_revenue_share(
    state: &AppState,
    buyer_id: i32,
    creator_id: i32,
    sample_id: i32,
    plan: &str,
) -> Result<(), AppError> {
    let plan_id = plan.parse::<KamplesPlanId>().unwrap_or(KamplesPlanId::Free);
    if plan_id == KamplesPlanId::Free {
        return Ok(());
    }

    let revenue = calculate_subscription_download_revenue_share(plan_id);
    if revenue.price_cents <= 0 {
        return Ok(());
    }

    BillingRepository::insert_completed_download_revenue_share(
        &state.pool,
        &CompletedDownloadRevenueShareInsert {
            buyer_id,
            creator_id,
            sample_id,
            amount_cents: revenue.price_cents,
            creator_amount_cents: revenue.creator_payout_cents,
            platform_fee_cents: revenue.platform_fee_cents,
        },
    )
    .await?;

    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct StreamQuery {
    pub token: String,
}

#[utoipa::path(
    get,
    path = "/api/descargas/stream",
    tag = "downloads",
    params(("token" = String, Query, description = "Token HMAC firmado emitido por register_download")),
    responses(
        (status = 200, description = "Archivo entregado", content_type = "application/octet-stream"),
        (status = 401, description = "Token inválido o ausente"),
        (status = 403, description = "Token expirado"),
        (status = 404, description = "Sample o archivo no encontrado"),
    )
)]
pub async fn stream_download(
    State(state): State<AppState>,
    Query(q): Query<StreamQuery>,
) -> Result<Response, AppError> {
    let (sample_id, _user_id) = download_token::verify(&q.token, &state.jwt_secret)?;

    let info = DownloadRepository::fetch_file_info(&state.pool, sample_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("sample {sample_id} no existe")))?;

    let bytes = state.storage.get_bytes(&info.storage_key).await?;

    let extension = std::path::Path::new(&info.storage_key)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");
    let safe_titulo: String = info
        .titulo
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(100)
        .collect();
    let nombre = format!("{safe_titulo}.{extension}");

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{nombre}\"")) {
        headers.insert(header::CONTENT_DISPOSITION, v);
    }
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );

    Ok((StatusCode::OK, headers, bytes).into_response())
}
