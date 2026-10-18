use axum::http::HeaderMap;
use axum::{http::StatusCode, Json};
use chrono::{Duration, Utc};

use crate::errors::AppError;
use crate::models::{
    AdminLegalReportItem, GenericReportType, LegalReportDetails, LegalReportType, ReportResponse,
};
use crate::repositories::{ModerationRepository, ProfileRepository, ReportRepository};
use crate::AppState;

pub(super) const DEFAULT_CONTENT_REASON: &str = "contenido inapropiado";
pub(super) const MAX_REASON_LEN: usize = 500;
pub(super) const MAX_ERROR_REASON_LEN: usize = 200;
pub(super) const MAX_DETAILS_LEN: usize = 2_000;
pub(super) const MAX_URL_LEN: usize = 500;
pub(super) const MIN_LEGAL_REASON_LEN: usize = 10;
pub(super) const MIN_NAME_LEN: usize = 2;
pub(super) const MIN_WORK_LEN: usize = 3;
pub(super) const REPORT_RATE_LIMIT_PER_HOUR: i64 = 10;
pub(super) const USER_REPORT_RATE_LIMIT_PER_DAY: i64 = 5;
pub(super) const ERROR_REPORT_RATE_LIMIT_PER_DAY: i64 = 5;
pub(super) const LEGAL_REPORT_RATE_LIMIT_PER_HOUR: i64 = 3;

const AUTO_SUSPEND_REPORT_THRESHOLD: i64 = 4;
const AUTO_SUSPEND_WINDOW_HOURS: i64 = 2;
const AUTO_SUSPEND_DURATION_HOURS: i64 = 48;

pub(super) async fn create_content_report(
    state: &AppState,
    reportador_id: i32,
    tipo: GenericReportType,
    target_id: i32,
    razon: &str,
    detalles: Option<&str>,
) -> Result<(StatusCode, Json<ReportResponse>), AppError> {
    if target_id <= 0 {
        return Err(AppError::Validation("targetId requerido".to_string()));
    }
    enforce_user_rate_limit(
        state,
        reportador_id,
        tipo.as_str(),
        REPORT_RATE_LIMIT_PER_HOUR,
        Duration::hours(1),
    )
    .await?;

    if !ReportRepository::target_exists(&state.pool, tipo.as_str(), target_id).await? {
        return Err(AppError::NotFound(match tipo {
            GenericReportType::Publicacion => format!("Publicacion {target_id} no encontrada"),
            GenericReportType::Comentario => format!("Comentario {target_id} no encontrado"),
            GenericReportType::Sample => format!("Sample {target_id} no encontrado"),
            GenericReportType::Usuario | GenericReportType::ErrorPlataforma => {
                format!("Target {target_id} no encontrado")
            }
        }));
    }

    if tipo.checks_duplicate()
        && ReportRepository::has_reported_target(
            &state.pool,
            tipo.as_str(),
            target_id,
            reportador_id,
        )
        .await?
    {
        return Ok((
            StatusCode::OK,
            Json(ReportResponse {
                ok: true,
                message: tipo.duplicate_message().to_string(),
            }),
        ));
    }

    ReportRepository::create_report(
        &state.pool,
        &crate::repositories::CreateReportRecord {
            tipo: tipo.as_str(),
            target_id,
            reportador_id: Some(reportador_id),
            reportado_id: None,
            razon,
            detalles,
            ip_origen: None,
        },
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ReportResponse {
            ok: true,
            message: "Reporte enviado".to_string(),
        }),
    ))
}

pub(super) async fn create_user_report(
    state: &AppState,
    reportador_id: i32,
    target_id: i32,
    razon: &str,
    detalles: Option<&str>,
) -> Result<(StatusCode, Json<ReportResponse>), AppError> {
    create_user_report_record(state, reportador_id, target_id, razon, detalles).await?;
    Ok((
        StatusCode::CREATED,
        Json(ReportResponse {
            ok: true,
            message: "Reporte enviado".to_string(),
        }),
    ))
}

pub(super) async fn create_user_report_record(
    state: &AppState,
    reportador_id: i32,
    target_id: i32,
    razon: &str,
    detalles: Option<&str>,
) -> Result<i32, AppError> {
    if target_id <= 0 {
        return Err(AppError::Validation("userId requerido".to_string()));
    }
    if reportador_id == target_id {
        return Err(AppError::BadRequest(
            "No puedes reportarte a ti mismo".to_string(),
        ));
    }
    if !ReportRepository::target_exists(&state.pool, GenericReportType::Usuario.as_str(), target_id)
        .await?
    {
        return Err(AppError::NotFound(format!(
            "Usuario {target_id} no encontrado"
        )));
    }

    let id = ReportRepository::create_report(
        &state.pool,
        &crate::repositories::CreateReportRecord {
            tipo: GenericReportType::Usuario.as_str(),
            target_id,
            reportador_id: Some(reportador_id),
            reportado_id: Some(target_id),
            razon,
            detalles,
            ip_origen: None,
        },
    )
    .await?;

    maybe_auto_suspend_target_user(state, target_id).await?;
    Ok(id)
}

pub(super) async fn create_platform_error_report_record(
    state: &AppState,
    reportador_id: i32,
    razon: &str,
    detalles: Option<&str>,
    url: Option<&str>,
) -> Result<i32, AppError> {
    let details_text = merge_platform_error_details(detalles, url);
    ReportRepository::create_report(
        &state.pool,
        &crate::repositories::CreateReportRecord {
            tipo: GenericReportType::ErrorPlataforma.as_str(),
            target_id: 0,
            reportador_id: Some(reportador_id),
            reportado_id: None,
            razon,
            detalles: details_text.as_deref(),
            ip_origen: None,
        },
    )
    .await
}

pub(super) async fn ensure_active_profile(state: &AppState, user_id: i32) -> Result<(), AppError> {
    let profile = ProfileRepository::find_by_id(&state.pool, user_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Usuario {user_id} no existe")))?;
    if profile.estado != "activo" {
        return Err(AppError::Forbidden(
            "La cuenta no esta activa para reportar".to_string(),
        ));
    }
    Ok(())
}

pub(super) async fn enforce_user_rate_limit(
    state: &AppState,
    reportador_id: i32,
    tipo: &str,
    max_reports: i64,
    window: Duration,
) -> Result<(), AppError> {
    let since = Utc::now() - window;
    let total =
        ReportRepository::count_by_reporter_and_type_since(&state.pool, reportador_id, tipo, since)
            .await?;
    if total >= max_reports {
        return Err(AppError::TooManyRequests(
            "Se alcanzó el limite de reportes para este tipo".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn normalize_reason(value: &str, max_len: usize) -> Result<String, AppError> {
    let reason = value.trim();
    if reason.is_empty() {
        return Err(AppError::Validation(
            "Indica el motivo del reporte".to_string(),
        ));
    }
    if reason.chars().count() > max_len {
        return Err(AppError::Validation(format!(
            "El motivo no puede superar {max_len} caracteres"
        )));
    }
    Ok(reason.to_string())
}

pub(super) fn normalize_optional_reason(
    value: Option<&str>,
    default_reason: &str,
    max_len: usize,
) -> Result<String, AppError> {
    match value {
        Some(raw) if !raw.trim().is_empty() => normalize_reason(raw, max_len),
        _ => Ok(default_reason.to_string()),
    }
}

pub(super) fn normalize_optional_details(value: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(details) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if details.chars().count() > MAX_DETAILS_LEN {
        return Err(AppError::Validation(format!(
            "Los detalles no pueden superar {MAX_DETAILS_LEN} caracteres"
        )));
    }
    Ok(Some(details.to_string()))
}

pub(super) fn normalize_required_details(value: &str) -> Result<String, AppError> {
    let details = value.trim();
    if details.is_empty() {
        return Err(AppError::Validation(
            "La descripcion del error es obligatoria".to_string(),
        ));
    }
    if details.chars().count() > MAX_DETAILS_LEN {
        return Err(AppError::Validation(format!(
            "La descripcion no puede superar {MAX_DETAILS_LEN} caracteres"
        )));
    }
    Ok(details.to_string())
}

pub(super) fn normalize_optional_url(value: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(url) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if url.chars().count() > MAX_URL_LEN {
        return Err(AppError::Validation(format!(
            "La URL no puede superar {MAX_URL_LEN} caracteres"
        )));
    }
    Ok(Some(url.to_string()))
}

pub(super) fn normalize_legal_reason(value: &str) -> Result<String, AppError> {
    let reason = value.trim();
    if reason.chars().count() < MIN_LEGAL_REASON_LEN {
        return Err(AppError::BadRequest("razon_demasiado_corta".to_string()));
    }
    if reason.chars().count() > MAX_DETAILS_LEN {
        return Err(AppError::BadRequest("razon_demasiado_larga".to_string()));
    }
    Ok(reason.to_string())
}

pub(super) fn normalize_required_field(
    value: &str,
    min_len: usize,
    max_len: usize,
    field_name: &str,
) -> Result<String, AppError> {
    let normalized = value.trim();
    let len = normalized.chars().count();
    if len < min_len {
        return Err(AppError::BadRequest(format!("{field_name}_requerido")));
    }
    if len > max_len {
        return Err(AppError::BadRequest(format!(
            "{field_name}_demasiado_largo"
        )));
    }
    Ok(normalized.to_string())
}

pub(super) fn map_admin_legal_report(
    row: crate::repositories::LegalReportRow,
) -> Result<AdminLegalReportItem, AppError> {
    let tipo = LegalReportType::from_db(&row.tipo).ok_or_else(|| {
        AppError::Internal(format!("tipo legal invalido almacenado: {}", row.tipo))
    })?;
    let mut details = row
        .detalles
        .as_deref()
        .and_then(|raw| serde_json::from_str::<LegalReportDetails>(raw).ok());
    if let Some(parsed) = details.as_mut() {
        if parsed.ip_origen.is_none() {
            parsed.ip_origen.clone_from(&row.ip_origen);
        }
    }

    Ok(AdminLegalReportItem {
        id: row.id,
        tipo,
        target_id: row.target_id,
        razon: row.razon,
        estado: row.estado,
        created_at: row.created_at,
        ip_origen: row.ip_origen,
        detalles: details,
    })
}

pub(super) fn client_ip_from_headers(headers: &HeaderMap) -> Option<String> {
    ["cf-connecting-ip", "x-real-ip", "x-forwarded-for"]
        .iter()
        .find_map(|header| {
            headers
                .get(*header)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.split(',').next())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
        })
}

async fn maybe_auto_suspend_target_user(state: &AppState, user_id: i32) -> Result<(), AppError> {
    let since = Utc::now() - Duration::hours(AUTO_SUSPEND_WINDOW_HOURS);
    let total =
        ReportRepository::count_recent_user_reports_about_user(&state.pool, user_id, since).await?;
    if total < AUTO_SUSPEND_REPORT_THRESHOLD {
        return Ok(());
    }

    let Some(profile) = ProfileRepository::find_by_id(&state.pool, user_id).await? else {
        return Ok(());
    };
    if profile.estado != "activo" {
        return Ok(());
    }

    let suspend_until = Utc::now() + Duration::hours(AUTO_SUSPEND_DURATION_HOURS);
    let reason = format!(
        "Suspension automatica: {total} reportes recibidos en las ultimas {AUTO_SUSPEND_WINDOW_HOURS} horas"
    );
    ModerationRepository::suspend(&state.pool, user_id, &reason, Some(suspend_until)).await
}

fn merge_platform_error_details(details: Option<&str>, url: Option<&str>) -> Option<String> {
    match (details, url) {
        (Some(details), Some(url)) if !url.is_empty() => Some(format!("URL: {url}\n\n{details}")),
        (Some(details), _) => Some(details.to_string()),
        (None, Some(url)) if !url.is_empty() => Some(format!("URL: {url}")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue};

    use super::{client_ip_from_headers, merge_platform_error_details};

    #[test]
    fn client_ip_prefers_forwarded_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("203.0.113.8, 10.0.0.1"),
        );

        assert_eq!(
            client_ip_from_headers(&headers).as_deref(),
            Some("203.0.113.8")
        );
    }

    #[test]
    fn merge_platform_error_details_keeps_legacy_shape() {
        assert_eq!(
            merge_platform_error_details(Some("Falla al guardar"), Some("https://kamples.com/x"))
                .as_deref(),
            Some("URL: https://kamples.com/x\n\nFalla al guardar")
        );
    }
}
