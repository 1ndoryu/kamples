use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde::Deserialize;
use utoipa::IntoParams;

use crate::errors::AppError;
use crate::middleware::CurrentUser;
use crate::models::{
    ClaimFreeCodeRequest, ClaimFreeCodeResponse, FreeCodeTargetType, GenerateFreeCodeRequest,
    GenerateFreeCodeResponse, InvalidateFreeCodeResponse, VerifyFreeCodeResponse,
};
use crate::repositories::{
    BillingRepository, ColeccionesRepository, CreateFreeCodeInput, FreeCodeRecord,
    FreeCodeRepository,
};
use crate::AppState;

/* [174A-84] CRUD mínimo de códigos gratis.
 *
 * Portado:
 * - POST /api/codigos-gratis/generar (admin)
 * - GET  /api/codigos-gratis/verificar (público)
 * - POST /api/codigos-gratis/reclamar (auth)
 * - POST /api/codigos-gratis/invalidar (admin)
 *
 * Reglas portadas:
 * - Expiración a 1 año.
 * - Reclamo idempotente.
 * - Código expirado => 50 créditos bonus una sola vez.
 * - Download endpoints consumen `codigoGratis` de forma opcional.
 *
 * NO portado:
 * - Rate limit 30/min por IP en verificar (falta RateLimiter global).
 */

const CREDITOS_COMPENSACION: i32 = 50;

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct VerifyFreeCodeQuery {
    pub codigo: String,
}

#[utoipa::path(
    post,
    path = "/api/codigos-gratis/generar",
    tag = "payments",
    request_body = GenerateFreeCodeRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 201, description = "Código generado", body = GenerateFreeCodeResponse),
        (status = 400, description = "Parámetros inválidos"),
        (status = 401, description = "No autenticado"),
        (status = 403, description = "No admin"),
        (status = 404, description = "Item no encontrado"),
    )
)]
pub async fn generate_free_code(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(request): Json<GenerateFreeCodeRequest>,
) -> Result<(StatusCode, Json<GenerateFreeCodeResponse>), AppError> {
    user.require_admin()?;

    if request.target_id <= 0 {
        return Err(AppError::BadRequest("targetId invalido".into()));
    }

    let nombre_item = match request.tipo {
        FreeCodeTargetType::Sample => {
            let sample_id = i32::try_from(request.target_id)
                .map_err(|_| AppError::BadRequest("targetId invalido para sample".into()))?;
            let sample = BillingRepository::find_sample_checkout_candidate(&state.pool, sample_id)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("sample {sample_id} no existe")))?;
            sample.sample_title
        }
        FreeCodeTargetType::Coleccion => {
            let collection = ColeccionesRepository::fetch(&state.pool, request.target_id)
                .await?
                .ok_or_else(|| {
                    AppError::NotFound(format!("coleccion {} no existe", request.target_id))
                })?;
            collection.nombre
        }
    };

    let codigo = uuid::Uuid::new_v4().simple().to_string();
    FreeCodeRepository::create(
        &state.pool,
        &CreateFreeCodeInput {
            codigo: &codigo,
            tipo: request.tipo.as_str(),
            target_id: request.target_id,
            creado_por_id: user.user_id,
            nombre_item: &nombre_item,
        },
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(GenerateFreeCodeResponse { ok: true, codigo }),
    ))
}

#[utoipa::path(
    get,
    path = "/api/codigos-gratis/verificar",
    tag = "payments",
    params(VerifyFreeCodeQuery),
    responses(
        (status = 200, description = "Código válido", body = VerifyFreeCodeResponse),
        (status = 400, description = "Código inválido"),
        (status = 404, description = "Código no existe o fue invalidado"),
        (status = 410, description = "Código expirado", body = VerifyFreeCodeResponse),
    )
)]
pub async fn verify_free_code(
    State(state): State<AppState>,
    Query(query): Query<VerifyFreeCodeQuery>,
) -> Result<Response, AppError> {
    let codigo = normalize_free_code(&query.codigo)?;
    let record = FreeCodeRepository::find_active(&state.pool, &codigo)
        .await?
        .ok_or_else(|| AppError::NotFound("Código inválido".into()))?;

    if record.expires_at <= Utc::now() {
        return Ok((
            StatusCode::GONE,
            Json(VerifyFreeCodeResponse {
                ok: false,
                tipo: None,
                target_id: None,
                expired: Some(true),
                nombre_item: Some(record.nombre_item),
            }),
        )
            .into_response());
    }

    let tipo = record_target_type(&record)?;
    Ok((
        StatusCode::OK,
        Json(VerifyFreeCodeResponse {
            ok: true,
            tipo: Some(tipo),
            target_id: Some(record.target_id),
            expired: None,
            nombre_item: None,
        }),
    )
        .into_response())
}

#[utoipa::path(
    post,
    path = "/api/codigos-gratis/reclamar",
    tag = "payments",
    request_body = ClaimFreeCodeRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Código reclamado o expirado", body = ClaimFreeCodeResponse),
        (status = 400, description = "Código requerido"),
        (status = 401, description = "No autenticado"),
        (status = 404, description = "Código no existe o fue invalidado"),
    )
)]
pub async fn claim_free_code(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(request): Json<ClaimFreeCodeRequest>,
) -> Result<Json<ClaimFreeCodeResponse>, AppError> {
    if request.codigo.trim().is_empty() {
        return Err(AppError::BadRequest("Código requerido".into()));
    }

    let codigo = normalize_free_code(&request.codigo)?;
    let record = FreeCodeRepository::find_active(&state.pool, &codigo)
        .await?
        .ok_or_else(|| AppError::NotFound("Código inválido".into()))?;

    if record.expires_at <= Utc::now() {
        let compensado = FreeCodeRepository::compensate_expired_claim(
            &state.pool,
            record.id,
            user.user_id,
            CREDITOS_COMPENSACION,
        )
        .await?;

        return Ok(Json(ClaimFreeCodeResponse {
            ok: false,
            tipo: None,
            target_id: None,
            expired: Some(true),
            compensado: Some(compensado),
            nombre_item: Some(record.nombre_item),
        }));
    }

    FreeCodeRepository::register_claim(&state.pool, record.id, user.user_id).await?;
    let tipo = record_target_type(&record)?;

    Ok(Json(ClaimFreeCodeResponse {
        ok: true,
        tipo: Some(tipo),
        target_id: Some(record.target_id),
        expired: None,
        compensado: None,
        nombre_item: None,
    }))
}

#[utoipa::path(
    post,
    path = "/api/codigos-gratis/invalidar",
    tag = "payments",
    request_body = GenerateFreeCodeRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Códigos invalidados", body = InvalidateFreeCodeResponse),
        (status = 400, description = "Parámetros inválidos"),
        (status = 401, description = "No autenticado"),
        (status = 403, description = "No admin"),
    )
)]
pub async fn invalidate_free_code(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(request): Json<GenerateFreeCodeRequest>,
) -> Result<Json<InvalidateFreeCodeResponse>, AppError> {
    user.require_admin()?;

    if request.target_id <= 0 {
        return Err(AppError::BadRequest("targetId invalido".into()));
    }

    let invalidados = FreeCodeRepository::invalidate_target(
        &state.pool,
        request.tipo.as_str(),
        request.target_id,
    )
    .await?;

    Ok(Json(InvalidateFreeCodeResponse {
        ok: true,
        invalidados,
    }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/codigos-gratis/generar", post(generate_free_code))
        .route("/codigos-gratis/verificar", get(verify_free_code))
        .route("/codigos-gratis/reclamar", post(claim_free_code))
        .route("/codigos-gratis/invalidar", post(invalidate_free_code))
}

pub(crate) fn normalize_free_code(value: &str) -> Result<String, AppError> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized.len() > 64 {
        return Err(AppError::BadRequest("Código inválido".into()));
    }
    Ok(normalized)
}

pub(crate) fn normalize_optional_free_code(
    value: Option<String>,
) -> Result<Option<String>, AppError> {
    match value {
        Some(raw) if !raw.trim().is_empty() => normalize_free_code(&raw).map(Some),
        _ => Ok(None),
    }
}

fn record_target_type(record: &FreeCodeRecord) -> Result<FreeCodeTargetType, AppError> {
    FreeCodeTargetType::from_db(&record.tipo).ok_or_else(|| {
        AppError::Internal(format!(
            "tipo de codigo gratis invalido almacenado: {} ({})",
            record.tipo.as_str(),
            record.codigo.as_str()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::{normalize_free_code, normalize_optional_free_code};

    #[test]
    fn normalize_free_code_trims_and_lowercases() {
        let code = normalize_free_code("  ABCDEF0123  ").expect("code");
        assert_eq!(code, "abcdef0123");
    }

    #[test]
    fn normalize_optional_free_code_ignores_blank_values() {
        assert_eq!(
            normalize_optional_free_code(Some("   ".into())).unwrap(),
            None
        );
    }
}
