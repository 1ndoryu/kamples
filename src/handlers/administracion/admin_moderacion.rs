/* [254A-4] Endpoints admin/moderacion — paridad con ModeracionController.php legacy.
 * GET /admin/moderacion?page=&reportes_page=&reportes_limit=
 *     - publicaciones pendientes (paginado 20/pag)
 *     - articulos pendientes (paginado 20/pag)
 *     - reportes pendientes (paginado configurable)
 *     - reportesTotal (count global)
 * GET /admin/moderacion/historial?dias=
 *     - publicaciones moderadas (no pendientes) en los ultimos N dias
 *
 * Devuelve `{ data: { ... } }` para alinear con la convencion legacy donde el frontend
 * extrae `respuesta.data` y rellena `imagenes/avatar` con fallbacks vacios.
 */

use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::errors::AppError;
#[allow(unused_imports)]
use crate::errors::ErrorResponse;
use crate::middleware::CurrentUser;
use crate::repositories::AdminModerationRepository;
pub use crate::repositories::{ArticuloPendiente, PublicacionPendiente, ReportePendiente};
use crate::services::admin_moderation::{
    AdminModerationService, ManualBanInput, ModerateContentInput, ResolveReportInput,
};
use crate::AppState;

const PAGE_SIZE: i64 = 20;
const HISTORIAL_DIAS_DEFAULT: i32 = 7;
const HISTORIAL_DIAS_MAX: i32 = 90;

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ListarQuery {
    #[serde(default)]
    pub page: Option<i64>,
    #[serde(default)]
    pub reportes_page: Option<i64>,
    #[serde(default)]
    pub reportes_limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ListarData {
    pub publicaciones: Vec<PublicacionPendiente>,
    pub articulos: Vec<ArticuloPendiente>,
    pub reportes: Vec<ReportePendiente>,
    #[serde(rename = "reportesTotal")]
    pub reportes_total: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AdminModeracionListarResponse {
    pub data: ListarData,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct HistorialQuery {
    #[serde(default)]
    pub dias: Option<i32>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct HistorialData {
    pub publicaciones: Vec<PublicacionPendiente>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AdminModeracionHistorialResponse {
    pub data: HistorialData,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ModerarRequest {
    pub tipo: String,
    pub id: i32,
    pub accion: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ResolverReporteRequest {
    pub id: i32,
    pub accion: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct BanearUsuarioRequest {
    pub usuario_id: i32,
    pub duracion: String,
    pub razon: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct RechazarUsuarioPublicacionesRequest {
    pub autor_id: i32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OkResponse {
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AffectedResponse {
    pub ok: bool,
    pub afectados: i64,
}

#[utoipa::path(get, path = "/api/admin/moderacion", tag = "admin",
    params(ListarQuery),
    security(("bearer_auth" = [])),
    responses((status = 200, body = AdminModeracionListarResponse), (status = 403)))]
pub async fn listar(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<ListarQuery>,
) -> Result<Json<AdminModeracionListarResponse>, AppError> {
    user.require_admin()?;
    let page = query.page.unwrap_or(1).max(1);
    let offset = (page - 1) * PAGE_SIZE;
    let reportes_page = query.reportes_page.unwrap_or(1).max(1);
    let reportes_limit = query.reportes_limit.unwrap_or(20).clamp(1, 50);
    let reportes_offset = (reportes_page - 1) * reportes_limit;

    let publicaciones =
        AdminModerationRepository::list_pending_posts(&state.pool, PAGE_SIZE, offset).await?;
    let articulos =
        AdminModerationRepository::list_pending_articles(&state.pool, PAGE_SIZE, offset).await?;
    let reportes = AdminModerationRepository::list_pending_reports(
        &state.pool,
        reportes_limit,
        reportes_offset,
    )
    .await?;
    let reportes_total = AdminModerationRepository::count_pending_reports(&state.pool).await?;

    Ok(Json(AdminModeracionListarResponse {
        data: ListarData {
            publicaciones,
            articulos,
            reportes,
            reportes_total,
        },
    }))
}

#[utoipa::path(get, path = "/api/admin/moderacion/historial", tag = "admin",
    params(HistorialQuery),
    security(("bearer_auth" = [])),
    responses((status = 200, body = AdminModeracionHistorialResponse), (status = 403)))]
pub async fn historial(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<HistorialQuery>,
) -> Result<Json<AdminModeracionHistorialResponse>, AppError> {
    user.require_admin()?;
    let dias = query
        .dias
        .unwrap_or(HISTORIAL_DIAS_DEFAULT)
        .clamp(1, HISTORIAL_DIAS_MAX);

    let publicaciones =
        AdminModerationRepository::list_recent_moderated_posts(&state.pool, dias).await?;

    Ok(Json(AdminModeracionHistorialResponse {
        data: HistorialData { publicaciones },
    }))
}

#[utoipa::path(post, path = "/api/admin/moderar", tag = "admin",
    request_body = ModerarRequest,
    security(("bearer_auth" = [])),
    responses((status = 200, body = OkResponse), (status = 400, body = ErrorResponse), (status = 403, body = ErrorResponse), (status = 404, body = ErrorResponse)))]
pub async fn moderar(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(request): Json<ModerarRequest>,
) -> Result<Json<OkResponse>, AppError> {
    user.require_admin()?;
    AdminModerationService::moderate_content(
        &state.pool,
        ModerateContentInput {
            tipo: request.tipo,
            id: request.id,
            accion: request.accion,
        },
    )
    .await?;
    Ok(Json(OkResponse { ok: true }))
}

#[utoipa::path(post, path = "/api/admin/reportes/resolver", tag = "admin",
    request_body = ResolverReporteRequest,
    security(("bearer_auth" = [])),
    responses((status = 200, body = OkResponse), (status = 400, body = ErrorResponse), (status = 403, body = ErrorResponse), (status = 404, body = ErrorResponse)))]
pub async fn resolver_reporte(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(request): Json<ResolverReporteRequest>,
) -> Result<Json<OkResponse>, AppError> {
    user.require_admin()?;
    AdminModerationService::resolve_report(
        &state.pool,
        ResolveReportInput {
            admin_id: user.user_id,
            id: request.id,
            accion: request.accion,
        },
    )
    .await?;
    Ok(Json(OkResponse { ok: true }))
}

#[utoipa::path(post, path = "/api/admin/moderacion/rechazar-pendientes", tag = "admin",
    security(("bearer_auth" = [])),
    responses((status = 200, body = AffectedResponse), (status = 403, body = ErrorResponse)))]
pub async fn rechazar_todos_pendientes(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<AffectedResponse>, AppError> {
    user.require_admin()?;
    let afectados = AdminModerationService::reject_pending_posts(&state.pool).await?;
    Ok(Json(AffectedResponse {
        ok: true,
        afectados,
    }))
}

#[utoipa::path(post, path = "/api/admin/moderacion/banear-usuario", tag = "admin",
    request_body = BanearUsuarioRequest,
    security(("bearer_auth" = [])),
    responses((status = 200, body = OkResponse), (status = 400, body = ErrorResponse), (status = 403, body = ErrorResponse), (status = 404, body = ErrorResponse)))]
pub async fn banear_usuario(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(request): Json<BanearUsuarioRequest>,
) -> Result<Json<OkResponse>, AppError> {
    user.require_admin()?;
    AdminModerationService::apply_manual_ban(
        &state.pool,
        ManualBanInput {
            admin_id: user.user_id,
            usuario_id: request.usuario_id,
            duracion: request.duracion,
            razon: request.razon,
        },
    )
    .await?;
    Ok(Json(OkResponse { ok: true }))
}

#[utoipa::path(post, path = "/api/admin/moderacion/rechazar-usuario-publicaciones", tag = "admin",
    request_body = RechazarUsuarioPublicacionesRequest,
    security(("bearer_auth" = [])),
    responses((status = 200, body = AffectedResponse), (status = 400, body = ErrorResponse), (status = 403, body = ErrorResponse)))]
pub async fn rechazar_publicaciones_usuario(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(request): Json<RechazarUsuarioPublicacionesRequest>,
) -> Result<Json<AffectedResponse>, AppError> {
    user.require_admin()?;
    let afectados =
        AdminModerationService::reject_user_posts(&state.pool, request.autor_id).await?;
    Ok(Json(AffectedResponse {
        ok: true,
        afectados,
    }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin/moderacion", get(listar))
        .route("/admin/moderacion/historial", get(historial))
        .route("/admin/moderar", post(moderar))
        .route("/admin/reportes/resolver", post(resolver_reporte))
        .route(
            "/admin/moderacion/rechazar-pendientes",
            post(rechazar_todos_pendientes),
        )
        .route("/admin/moderacion/banear-usuario", post(banear_usuario))
        .route(
            "/admin/moderacion/rechazar-usuario-publicaciones",
            post(rechazar_publicaciones_usuario),
        )
}
