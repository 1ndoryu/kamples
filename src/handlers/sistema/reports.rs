mod support;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{Duration, Utc};
use validator::ValidateEmail;

use self::support::{
    client_ip_from_headers, create_content_report, create_platform_error_report_record,
    create_user_report, create_user_report_record, enforce_user_rate_limit, ensure_active_profile,
    map_admin_legal_report, normalize_legal_reason, normalize_optional_details,
    normalize_optional_reason, normalize_optional_url, normalize_reason,
    normalize_required_details, normalize_required_field, DEFAULT_CONTENT_REASON,
    ERROR_REPORT_RATE_LIMIT_PER_DAY, LEGAL_REPORT_RATE_LIMIT_PER_HOUR, MAX_ERROR_REASON_LEN,
    MAX_REASON_LEN, MIN_NAME_LEN, MIN_WORK_LEN, REPORT_RATE_LIMIT_PER_HOUR,
    USER_REPORT_RATE_LIMIT_PER_DAY,
};
use crate::errors::AppError;
#[allow(unused_imports)]
use crate::errors::ErrorResponse;
use crate::middleware::CurrentUser;
use crate::models::{
    AdminLegalReportsQuery, AdminLegalReportsResponse, CreateGenericReportRequest,
    CreateLegalReportRequest, CreatePlatformErrorReportRequest, CreateReportReasonRequest,
    CreateScopedReportRequest, ErrorReportResponse, GenericReportType, LegalReportDetails,
    LegalReportResponse, LegalReportType, ReportResponse,
};
use crate::repositories::ReportRepository;
use crate::AppState;

#[utoipa::path(
    post,
    path = "/api/reportar",
    tag = "reports",
    request_body = CreateGenericReportRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Reporte duplicado aceptado de forma idempotente", body = ReportResponse),
        (status = 201, description = "Reporte creado", body = ReportResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 404, description = "Target no encontrado", body = ErrorResponse),
        (status = 422, description = "Payload inválido", body = ErrorResponse),
        (status = 429, description = "Límite de reportes excedido", body = ErrorResponse)
    )
)]
pub async fn report_generic(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(request): Json<CreateGenericReportRequest>,
) -> Result<Response, AppError> {
    ensure_active_profile(&state, user.user_id).await?;

    let reason = normalize_reason(&request.razon, MAX_REASON_LEN)?;
    let details = normalize_optional_details(request.detalles.as_deref())?;
    let url = normalize_optional_url(request.url.as_deref())?;

    match request.tipo {
        GenericReportType::Usuario => {
            enforce_user_rate_limit(
                &state,
                user.user_id,
                request.tipo.as_str(),
                REPORT_RATE_LIMIT_PER_HOUR,
                Duration::hours(1),
            )
            .await?;
            let response = create_user_report(
                &state,
                user.user_id,
                request.target_id,
                &reason,
                details.as_deref(),
            )
            .await?;
            Ok(response.into_response())
        }
        GenericReportType::Publicacion
        | GenericReportType::Comentario
        | GenericReportType::Sample => {
            let response = create_content_report(
                &state,
                user.user_id,
                request.tipo,
                request.target_id,
                &reason,
                details.as_deref(),
            )
            .await?;
            Ok(response.into_response())
        }
        GenericReportType::ErrorPlataforma => {
            enforce_user_rate_limit(
                &state,
                user.user_id,
                request.tipo.as_str(),
                REPORT_RATE_LIMIT_PER_HOUR,
                Duration::hours(1),
            )
            .await?;
            let id = create_platform_error_report_record(
                &state,
                user.user_id,
                &reason,
                details.as_deref(),
                url.as_deref(),
            )
            .await?;
            Ok((
                StatusCode::CREATED,
                Json(ReportResponse {
                    ok: true,
                    message: format!("Reporte enviado (#{id})"),
                }),
            )
                .into_response())
        }
    }
}

#[utoipa::path(
    post,
    path = "/api/reportar-usuario/{userId}",
    tag = "reports",
    request_body = CreateReportReasonRequest,
    params(("userId" = i32, Path, description = "ID del usuario reportado")),
    security(("bearer_auth" = [])),
    responses(
        (status = 201, description = "Reporte de usuario creado", body = ReportResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 404, description = "Usuario no encontrado", body = ErrorResponse),
        (status = 422, description = "Payload inválido", body = ErrorResponse),
        (status = 429, description = "Límite excedido", body = ErrorResponse)
    )
)]
pub async fn report_user_legacy(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(user_id): Path<i32>,
    Json(request): Json<CreateReportReasonRequest>,
) -> Result<(StatusCode, Json<ReportResponse>), AppError> {
    ensure_active_profile(&state, user.user_id).await?;
    enforce_user_rate_limit(
        &state,
        user.user_id,
        GenericReportType::Usuario.as_str(),
        USER_REPORT_RATE_LIMIT_PER_DAY,
        Duration::hours(24),
    )
    .await?;

    let reason = normalize_reason(&request.razon, MAX_REASON_LEN)?;
    let details = normalize_optional_details(request.detalles.as_deref())?;
    create_user_report_record(&state, user.user_id, user_id, &reason, details.as_deref()).await?;

    Ok((
        StatusCode::CREATED,
        Json(ReportResponse {
            ok: true,
            message: "Reporte enviado".to_string(),
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/api/reportar-error",
    tag = "reports",
    request_body = CreatePlatformErrorReportRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 201, description = "Reporte de error creado", body = ErrorReportResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 422, description = "Payload inválido", body = ErrorResponse),
        (status = 429, description = "Límite excedido", body = ErrorResponse)
    )
)]
pub async fn report_platform_error(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(request): Json<CreatePlatformErrorReportRequest>,
) -> Result<(StatusCode, Json<ErrorReportResponse>), AppError> {
    ensure_active_profile(&state, user.user_id).await?;
    enforce_user_rate_limit(
        &state,
        user.user_id,
        GenericReportType::ErrorPlataforma.as_str(),
        ERROR_REPORT_RATE_LIMIT_PER_DAY,
        Duration::hours(24),
    )
    .await?;

    let reason = normalize_reason(&request.razon, MAX_ERROR_REASON_LEN)?;
    let details = normalize_required_details(&request.detalles)?;
    let url = normalize_optional_url(request.url.as_deref())?;
    let id = create_platform_error_report_record(
        &state,
        user.user_id,
        &reason,
        Some(details.as_str()),
        url.as_deref(),
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ErrorReportResponse {
            ok: true,
            message: "Reporte enviado correctamente".to_string(),
            id,
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/api/reportar-legal",
    tag = "reports",
    request_body = CreateLegalReportRequest,
    responses(
        (status = 201, description = "Reclamación legal registrada", body = LegalReportResponse),
        (status = 400, description = "Payload inválido", body = ErrorResponse),
        (status = 429, description = "Límite excedido", body = ErrorResponse)
    )
)]
pub async fn report_legal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateLegalReportRequest>,
) -> Result<(StatusCode, Json<LegalReportResponse>), AppError> {
    if request.target_id <= 0 {
        return Err(AppError::BadRequest("target_id_requerido".to_string()));
    }
    if !ReportRepository::target_exists(&state.pool, request.tipo.as_str(), request.target_id)
        .await?
    {
        return Err(AppError::BadRequest("target_no_encontrado".to_string()));
    }

    let ip_origen = client_ip_from_headers(&headers).unwrap_or_else(|| "unknown".to_string());
    let since = Utc::now() - Duration::hours(1);
    let total = ReportRepository::count_by_ip_and_types_since(
        &state.pool,
        &ip_origen,
        &[
            LegalReportType::LegalSample.as_str(),
            LegalReportType::LegalRelacion.as_str(),
        ],
        since,
    )
    .await?;
    if total >= LEGAL_REPORT_RATE_LIMIT_PER_HOUR {
        return Err(AppError::TooManyRequests(
            "Se alcanzó el límite de reclamaciones legales para esta IP".to_string(),
        ));
    }

    let reason = normalize_legal_reason(&request.razon)?;
    let name = normalize_required_field(&request.nombre, MIN_NAME_LEN, 200, "nombre")?;
    let email = request.email.trim().to_string();
    if !email.validate_email() {
        return Err(AppError::BadRequest("email_invalido".to_string()));
    }
    let protected_work = normalize_required_field(
        &request.obra_protegida,
        MIN_WORK_LEN,
        1_000,
        "obra_protegida",
    )?;
    if !request.declaracion {
        return Err(AppError::BadRequest(
            "declaracion_buena_fe_requerida".to_string(),
        ));
    }

    let details = LegalReportDetails {
        nombre: name,
        email,
        tipo_derecho: request.tipo_derecho,
        obra_protegida: protected_work,
        declaracion_bf: true,
        ip_origen: Some(ip_origen.clone()),
        fecha_envio: Utc::now().to_rfc3339(),
    };
    let details_json = serde_json::to_string(&details)
        .map_err(|error| AppError::Internal(format!("serializar reporte legal: {error}")))?;

    let report_id = ReportRepository::create_report(
        &state.pool,
        &crate::repositories::CreateReportRecord {
            tipo: request.tipo.as_str(),
            target_id: request.target_id,
            reportador_id: None,
            reportado_id: None,
            razon: &reason,
            detalles: Some(details_json.as_str()),
            ip_origen: Some(ip_origen.as_str()),
        },
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(LegalReportResponse {
            ok: true,
            reporte_id: report_id,
            mensaje: "Reclamación registrada. Nuestro equipo la revisará en 72 horas hábiles."
                .to_string(),
        }),
    ))
}

#[utoipa::path(
    get,
    path = "/api/admin/reportes/legales",
    tag = "reports",
    params(AdminLegalReportsQuery),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Listado admin de reclamaciones legales pendientes", body = AdminLegalReportsResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "No admin", body = ErrorResponse)
    )
)]
pub async fn list_pending_legal_reports(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<AdminLegalReportsQuery>,
) -> Result<Json<AdminLegalReportsResponse>, AppError> {
    user.require_admin()?;
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);
    let rows = ReportRepository::list_pending_legal_reports(&state.pool, limit, offset).await?;
    let total = ReportRepository::count_pending_legal_reports(&state.pool).await?;
    let reportes = rows
        .into_iter()
        .map(map_admin_legal_report)
        .collect::<Result<Vec<_>, AppError>>()?;

    Ok(Json(AdminLegalReportsResponse {
        ok: true,
        reportes,
        total,
    }))
}

#[utoipa::path(
    post,
    path = "/api/comentarios/{commentId}/reportar",
    tag = "reports",
    request_body = Option<CreateScopedReportRequest>,
    params(("commentId" = i32, Path, description = "ID del comentario a reportar")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Reporte duplicado aceptado de forma idempotente", body = ReportResponse),
        (status = 201, description = "Reporte creado", body = ReportResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 404, description = "Comentario no encontrado", body = ErrorResponse),
        (status = 429, description = "Límite excedido", body = ErrorResponse)
    )
)]
pub async fn report_comment(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(comment_id): Path<i32>,
    payload: Option<Json<CreateScopedReportRequest>>,
) -> Result<Response, AppError> {
    ensure_active_profile(&state, user.user_id).await?;
    let payload = payload.map(|Json(value)| value).unwrap_or_default();
    let reason = normalize_optional_reason(
        payload.razon.as_deref(),
        DEFAULT_CONTENT_REASON,
        MAX_REASON_LEN,
    )?;
    let details = normalize_optional_details(payload.detalles.as_deref())?;
    let response = create_content_report(
        &state,
        user.user_id,
        GenericReportType::Comentario,
        comment_id,
        &reason,
        details.as_deref(),
    )
    .await?;
    Ok(response.into_response())
}

#[utoipa::path(
    post,
    path = "/api/publicaciones/{postId}/reportar",
    tag = "reports",
    request_body = Option<CreateScopedReportRequest>,
    params(("postId" = i32, Path, description = "ID de la publicación a reportar")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Reporte duplicado aceptado de forma idempotente", body = ReportResponse),
        (status = 201, description = "Reporte creado", body = ReportResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 404, description = "Publicación no encontrada", body = ErrorResponse),
        (status = 429, description = "Límite excedido", body = ErrorResponse)
    )
)]
pub async fn report_post(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(post_id): Path<i32>,
    payload: Option<Json<CreateScopedReportRequest>>,
) -> Result<Response, AppError> {
    ensure_active_profile(&state, user.user_id).await?;
    let payload = payload.map(|Json(value)| value).unwrap_or_default();
    let reason = normalize_optional_reason(
        payload.razon.as_deref(),
        DEFAULT_CONTENT_REASON,
        MAX_REASON_LEN,
    )?;
    let details = normalize_optional_details(payload.detalles.as_deref())?;
    let response = create_content_report(
        &state,
        user.user_id,
        GenericReportType::Publicacion,
        post_id,
        &reason,
        details.as_deref(),
    )
    .await?;
    Ok(response.into_response())
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/reportar", post(report_generic))
        .route("/reportar-usuario/:user_id", post(report_user_legacy))
        .route("/reportar-error", post(report_platform_error))
        .route("/reportar-legal", post(report_legal))
        .route("/admin/reportes/legales", get(list_pending_legal_reports))
        .route("/comentarios/:comment_id/reportar", post(report_comment))
        .route("/publicaciones/:post_id/reportar", post(report_post))
}
