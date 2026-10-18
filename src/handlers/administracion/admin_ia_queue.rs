use axum::extract::{Json, Query, State};
use axum::routing::{get, post};
use axum::Router;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::errors::AppError;
use crate::middleware::CurrentUser;
use crate::repositories::{
    AdminIaQueueItem, AdminIaQueueListParams, AdminIaQueueRepository, AdminIaQueueStats,
    ProcessingQueueRepository, QueueFailureDisposition,
};
use crate::services::{AudioPipelineRequest, AudioPipelineService};
use crate::AppState;

#[derive(Debug, Deserialize, IntoParams)]
pub struct AdminIaQueueQuery {
    #[serde(alias = "pagina")]
    pub page: Option<i64>,
    #[serde(alias = "limite")]
    pub limit: Option<i64>,
    pub estado: Option<String>,
    pub tipo: Option<String>,
    pub busqueda: Option<String>,
    #[serde(alias = "sortBy")]
    pub sort_by: Option<String>,
    #[serde(alias = "sortDir")]
    pub sort_dir: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AdminIaQueuePagination {
    pub page: i64,
    pub limit: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AdminIaQueueListResponse {
    pub data: Vec<AdminIaQueueItem>,
    pub pagination: AdminIaQueuePagination,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AdminIaQueueStatsResponse {
    pub ok: bool,
    pub data: AdminIaQueueStats,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RetryIaQueueRequest {
    pub id: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RetryIaQueueResponse {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RetryAllIaQueueResponse {
    pub ok: bool,
    pub reintentados: u64,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ProcessIaQueueResponse {
    pub ok: bool,
    pub procesados: u32,
    pub errores: u32,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GroqQuotaResponse {
    pub ok: bool,
    pub data: GroqQuotaData,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GroqQuotaData {
    pub limite_diario: u32,
    pub usado_hoy: u32,
    pub restante: u32,
    pub reset_en: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GroqKeysStatusResponse {
    pub ok: bool,
    pub data: Vec<GroqKeyStatus>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GroqKeyStatus {
    pub nombre: String,
    pub configurada: bool,
    pub activa: bool,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/cola-ia", get(list_queue))
        .route("/admin/cola-ia/estadisticas", get(stats_queue))
        .route("/admin/cola-ia/reintentar", post(retry_queue_item))
        .route(
            "/admin/cola-ia/reintentar-todos",
            post(retry_all_queue_items),
        )
        .route("/admin/cola-ia/procesar", post(process_queue_now))
        .route("/admin/cola-ia/cuota-groq", get(groq_quota))
        .route("/admin/cola-ia/estado-keys", get(groq_keys_status))
}

#[utoipa::path(get, path = "/api/admin/cola-ia", tag = "admin",
    params(AdminIaQueueQuery), security(("bearer_auth" = [])),
    responses((status = 200, body = AdminIaQueueListResponse)))]
pub async fn list_queue(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<AdminIaQueueQuery>,
) -> Result<Json<AdminIaQueueListResponse>, AppError> {
    user.require_admin()?;
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(10).clamp(5, 100);
    let data = AdminIaQueueRepository::list(
        &state.pool,
        AdminIaQueueListParams {
            page,
            limit,
            estado: query.estado.as_deref(),
            tipo: query.tipo.as_deref(),
            busqueda: query.busqueda.as_deref(),
            sort_col: query.sort_by.as_deref(),
            sort_dir: query.sort_dir.as_deref(),
        },
    )
    .await?;
    Ok(Json(AdminIaQueueListResponse {
        data,
        pagination: AdminIaQueuePagination { page, limit },
    }))
}

#[utoipa::path(get, path = "/api/admin/cola-ia/estadisticas", tag = "admin",
    security(("bearer_auth" = [])),
    responses((status = 200, body = AdminIaQueueStatsResponse)))]
pub async fn stats_queue(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<AdminIaQueueStatsResponse>, AppError> {
    user.require_admin()?;
    let data = AdminIaQueueRepository::stats(&state.pool).await?;
    Ok(Json(AdminIaQueueStatsResponse { ok: true, data }))
}

#[utoipa::path(post, path = "/api/admin/cola-ia/reintentar", tag = "admin",
    request_body = RetryIaQueueRequest, security(("bearer_auth" = [])),
    responses((status = 200, body = RetryIaQueueResponse)))]
pub async fn retry_queue_item(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(request): Json<RetryIaQueueRequest>,
) -> Result<Json<RetryIaQueueResponse>, AppError> {
    user.require_admin()?;
    let retried = AdminIaQueueRepository::retry_one(&state.pool, request.id).await?;
    if !retried {
        return Err(AppError::NotFound("item de cola IA no encontrado".into()));
    }
    Ok(Json(RetryIaQueueResponse {
        ok: true,
        message: "Item reencolado correctamente".into(),
    }))
}

#[utoipa::path(post, path = "/api/admin/cola-ia/reintentar-todos", tag = "admin",
    security(("bearer_auth" = [])),
    responses((status = 200, body = RetryAllIaQueueResponse)))]
pub async fn retry_all_queue_items(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<RetryAllIaQueueResponse>, AppError> {
    user.require_admin()?;
    let reintentados = AdminIaQueueRepository::retry_all(&state.pool).await?;
    Ok(Json(RetryAllIaQueueResponse {
        ok: true,
        reintentados,
        message: format!("{reintentados} items reencolados"),
    }))
}

#[utoipa::path(post, path = "/api/admin/cola-ia/procesar", tag = "admin",
    security(("bearer_auth" = [])),
    responses((status = 200, body = ProcessIaQueueResponse)))]
pub async fn process_queue_now(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<ProcessIaQueueResponse>, AppError> {
    user.require_admin()?;
    let Some(job) = ProcessingQueueRepository::claim_next_audio_analysis(&state.pool).await? else {
        return Ok(Json(ProcessIaQueueResponse {
            ok: true,
            procesados: 0,
            errores: 0,
            message: "No hay trabajos de audio pendientes para procesar".into(),
        }));
    };

    let service = AudioPipelineService::new(state.pool.clone(), state.storage.clone());
    let result = service
        .run(AudioPipelineRequest {
            sample_id: job.sample_id,
            force_recompute: false,
        })
        .await;

    match result {
        Ok(_) => {
            ProcessingQueueRepository::mark_audio_analysis_completed(&state.pool, job.id).await?;
            Ok(Json(ProcessIaQueueResponse {
                ok: true,
                procesados: 1,
                errores: 0,
                message: format!("Procesado job {} para sample {}", job.id, job.sample_id),
            }))
        }
        Err(error) => {
            let disposition = ProcessingQueueRepository::mark_audio_analysis_failed(
                &state.pool,
                &job,
                &error.to_string(),
                error.is_retryable(),
            )
            .await?;
            let message = match disposition {
                QueueFailureDisposition::RetryScheduled => {
                    format!("El job {} fallo y quedo reprogramado: {error}", job.id)
                }
                QueueFailureDisposition::FinalError => {
                    format!("El job {} fallo de forma definitiva: {error}", job.id)
                }
            };
            Ok(Json(ProcessIaQueueResponse {
                ok: false,
                procesados: 0,
                errores: 1,
                message,
            }))
        }
    }
}

#[utoipa::path(get, path = "/api/admin/cola-ia/cuota-groq", tag = "admin",
    security(("bearer_auth" = [])),
    responses((status = 200, body = GroqQuotaResponse)))]
pub async fn groq_quota(user: CurrentUser) -> Result<Json<GroqQuotaResponse>, AppError> {
    user.require_admin()?;
    Ok(Json(GroqQuotaResponse {
        ok: true,
        data: GroqQuotaData {
            limite_diario: 14_400,
            usado_hoy: 0,
            restante: 14_400,
            reset_en: "UTC midnight".into(),
        },
    }))
}

#[utoipa::path(get, path = "/api/admin/cola-ia/estado-keys", tag = "admin",
    security(("bearer_auth" = [])),
    responses((status = 200, body = GroqKeysStatusResponse)))]
pub async fn groq_keys_status(user: CurrentUser) -> Result<Json<GroqKeysStatusResponse>, AppError> {
    user.require_admin()?;
    let key_names = ["GROQ_API_KEY", "GROQ_API_KEY_2", "GROQ_API_KEY_3"];
    let data = key_names
        .into_iter()
        .map(|name| {
            let configured = std::env::var(name).is_ok_and(|value| !value.trim().is_empty());
            GroqKeyStatus {
                nombre: name.to_owned(),
                configurada: configured,
                activa: configured,
            }
        })
        .collect();
    Ok(Json(GroqKeysStatusResponse { ok: true, data }))
}
