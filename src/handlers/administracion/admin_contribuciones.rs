/* sentinel-disable-file directory-size — modulo legacy dentro de `handlers/`;
 * la correccion estructural es migrar handlers a subdirectorios por dominio. */
/* [274A-23..26+48] Endpoints de contribuciones comunitarias.
 * Port real de ContribucionesController.php: propuestas de usuario, historial
 * propio y moderacion admin que aplica relaciones/ediciones/eliminaciones. */

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::{IntoParams, ToSchema};

use crate::errors::AppError;
#[allow(unused_imports)]
use crate::errors::ErrorResponse;
use crate::middleware::CurrentUser;
pub use crate::repositories::ContribucionPendiente;
use crate::repositories::{ActualizarContribucionRecord, ContribucionesRepository};
use crate::services::contribuciones::{
    ContribucionesService, CrearContribucionInput, ModerarContribucionInput, ProponerEdicionInput,
    ProponerEliminacionInput,
};
use crate::AppState;

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CrearContribucionRequest {
    pub tipo_relacion: String,
    #[serde(default)]
    pub tipo_elemento: Option<String>,
    #[serde(default)]
    pub cancion_destino_id: Option<i32>,
    #[serde(default)]
    pub cancion_fuente_id: Option<i32>,
    #[serde(default)]
    pub cancion_nueva_titulo: Option<String>,
    #[serde(default)]
    pub cancion_nueva_artista: Option<String>,
    #[serde(default)]
    pub cancion_nueva_youtube_url: Option<String>,
    #[serde(default)]
    pub cancion_nueva_lado: Option<String>,
    #[serde(default)]
    pub timing_fuente: Option<i32>,
    #[serde(default)]
    pub timing_destino: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ActualizarContribucionRequest {
    #[serde(default)]
    pub tipo_relacion: Option<String>,
    #[serde(default)]
    pub tipo_elemento: Option<String>,
    #[serde(default)]
    pub cancion_destino_id: Option<i32>,
    #[serde(default)]
    pub cancion_fuente_id: Option<i32>,
    #[serde(default)]
    pub cancion_nueva_titulo: Option<String>,
    #[serde(default)]
    pub cancion_nueva_artista: Option<String>,
    #[serde(default)]
    pub cancion_nueva_youtube_url: Option<String>,
    #[serde(default)]
    pub cancion_nueva_lado: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ProponerEdicionRequest {
    pub relacion_id: i32,
    #[schema(value_type = Object)]
    pub cambios: Value,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ProponerEliminacionRequest {
    pub relacion_id: i32,
    pub razon: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ModerarContribucionRequest {
    pub id: i32,
    pub accion: String,
    #[serde(default)]
    pub nota: Option<String>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ListarQuery {
    #[serde(default)]
    pub page: Option<i64>,
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RespuestaContribucion {
    pub ok: bool,
    pub id: Option<i32>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RespuestaOk {
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MisContribucionesResponse {
    pub ok: bool,
    pub items: Vec<ContribucionPendiente>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AdminContribucionesListarResponse {
    pub ok: bool,
    pub items: Vec<ContribucionPendiente>,
    pub total: i64,
    pub page: i64,
    pub limit: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ModerarContribucionResponse {
    pub ok: bool,
    pub relacion_id: Option<i32>,
}

#[utoipa::path(
    post, path = "/api/contribuciones", tag = "contribuciones",
    request_body = CrearContribucionRequest,
    security(("bearer_auth" = [])),
    responses((status = 201, body = RespuestaContribucion), (status = 409, body = ErrorResponse), (status = 422, body = ErrorResponse))
)]
pub async fn crear(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(request): Json<CrearContribucionRequest>,
) -> Result<(StatusCode, Json<RespuestaContribucion>), AppError> {
    let id = ContribucionesService::crear(
        &state.pool,
        CrearContribucionInput {
            contribuidor_id: user.user_id,
            cancion_destino_id: request.cancion_destino_id,
            cancion_fuente_id: request.cancion_fuente_id,
            cancion_nueva_titulo: request.cancion_nueva_titulo,
            cancion_nueva_artista: request.cancion_nueva_artista,
            cancion_nueva_youtube_url: request.cancion_nueva_youtube_url,
            cancion_nueva_lado: request.cancion_nueva_lado,
            tipo_relacion: request.tipo_relacion,
            tipo_elemento: request.tipo_elemento,
            timing_fuente: request.timing_fuente,
            timing_destino: request.timing_destino,
        },
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(RespuestaContribucion {
            ok: true,
            id: Some(id),
        }),
    ))
}

#[utoipa::path(
    get, path = "/api/contribuciones/mis", tag = "contribuciones",
    params(ListarQuery), security(("bearer_auth" = [])),
    responses((status = 200, body = MisContribucionesResponse))
)]
pub async fn mis(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<ListarQuery>,
) -> Result<Json<MisContribucionesResponse>, AppError> {
    let (page, limit, offset) = pagination(&query);
    let items =
        ContribucionesRepository::listar_usuario(&state.pool, user.user_id, limit, offset).await?;
    let _ = page;
    Ok(Json(MisContribucionesResponse { ok: true, items }))
}

#[utoipa::path(
    put, path = "/api/contribuciones/{id}", tag = "contribuciones",
    request_body = ActualizarContribucionRequest,
    security(("bearer_auth" = [])),
    responses((status = 200, body = RespuestaOk), (status = 403, body = ErrorResponse), (status = 422, body = ErrorResponse))
)]
pub async fn actualizar(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i32>,
    Json(request): Json<ActualizarContribucionRequest>,
) -> Result<Json<RespuestaOk>, AppError> {
    let input = ActualizarContribucionRecord {
        cancion_destino_id: request.cancion_destino_id,
        cancion_fuente_id: request.cancion_fuente_id,
        cancion_nueva_titulo: trim_optional(request.cancion_nueva_titulo),
        cancion_nueva_artista: trim_optional(request.cancion_nueva_artista),
        cancion_nueva_youtube_url: trim_optional(request.cancion_nueva_youtube_url),
        cancion_nueva_lado: request.cancion_nueva_lado,
        tipo_relacion: request.tipo_relacion,
        tipo_elemento: request.tipo_elemento,
    };
    if !input.has_changes() {
        return Err(AppError::Validation(
            "No se enviaron campos para actualizar.".into(),
        ));
    }
    let ok = ContribucionesRepository::actualizar_pendiente_usuario(
        &state.pool,
        id,
        user.user_id,
        input,
    )
    .await?;
    if !ok {
        return Err(AppError::Forbidden(
            "No se pudo actualizar. Verifica que la contribucion sea tuya y este pendiente.".into(),
        ));
    }
    Ok(Json(RespuestaOk { ok: true }))
}

#[utoipa::path(
    delete, path = "/api/contribuciones/{id}", tag = "contribuciones",
    security(("bearer_auth" = [])),
    responses((status = 200, body = RespuestaOk), (status = 403, body = ErrorResponse))
)]
pub async fn eliminar(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i32>,
) -> Result<Json<RespuestaOk>, AppError> {
    let ok =
        ContribucionesRepository::eliminar_pendiente_usuario(&state.pool, id, user.user_id).await?;
    if !ok {
        return Err(AppError::Forbidden(
            "No se pudo eliminar. Verifica que la contribucion sea tuya y este pendiente.".into(),
        ));
    }
    Ok(Json(RespuestaOk { ok: true }))
}

#[utoipa::path(
    post, path = "/api/contribuciones/edicion", tag = "contribuciones",
    request_body = ProponerEdicionRequest,
    security(("bearer_auth" = [])),
    responses((status = 201, body = RespuestaContribucion), (status = 404, body = ErrorResponse), (status = 409, body = ErrorResponse), (status = 422, body = ErrorResponse))
)]
pub async fn proponer_edicion(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(request): Json<ProponerEdicionRequest>,
) -> Result<(StatusCode, Json<RespuestaContribucion>), AppError> {
    let id = ContribucionesService::proponer_edicion(
        &state.pool,
        ProponerEdicionInput {
            contribuidor_id: user.user_id,
            relacion_id: request.relacion_id,
            cambios: request.cambios,
        },
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(RespuestaContribucion {
            ok: true,
            id: Some(id),
        }),
    ))
}

#[utoipa::path(
    post, path = "/api/contribuciones/eliminacion", tag = "contribuciones",
    request_body = ProponerEliminacionRequest,
    security(("bearer_auth" = [])),
    responses((status = 201, body = RespuestaContribucion), (status = 404, body = ErrorResponse), (status = 409, body = ErrorResponse), (status = 422, body = ErrorResponse))
)]
pub async fn proponer_eliminacion(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(request): Json<ProponerEliminacionRequest>,
) -> Result<(StatusCode, Json<RespuestaContribucion>), AppError> {
    let id = ContribucionesService::proponer_eliminacion(
        &state.pool,
        ProponerEliminacionInput {
            contribuidor_id: user.user_id,
            relacion_id: request.relacion_id,
            razon: request.razon,
        },
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(RespuestaContribucion {
            ok: true,
            id: Some(id),
        }),
    ))
}

#[utoipa::path(
    get, path = "/api/admin/contribuciones", tag = "admin",
    params(ListarQuery), security(("bearer_auth" = [])),
    responses((status = 200, body = AdminContribucionesListarResponse), (status = 403, body = ErrorResponse))
)]
pub async fn listar(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<ListarQuery>,
) -> Result<Json<AdminContribucionesListarResponse>, AppError> {
    user.require_admin()?;
    let (page, limit, offset) = pagination(&query);
    let items =
        ContribucionesRepository::listar_pendientes_admin(&state.pool, limit, offset).await?;
    let total = ContribucionesRepository::contar_pendientes(&state.pool).await?;
    Ok(Json(AdminContribucionesListarResponse {
        ok: true,
        items,
        total,
        page,
        limit,
    }))
}

#[utoipa::path(
    post, path = "/api/admin/contribuciones/moderar", tag = "admin",
    request_body = ModerarContribucionRequest,
    security(("bearer_auth" = [])),
    responses((status = 200, body = ModerarContribucionResponse), (status = 403, body = ErrorResponse), (status = 404, body = ErrorResponse), (status = 409, body = ErrorResponse), (status = 422, body = ErrorResponse))
)]
pub async fn moderar(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(request): Json<ModerarContribucionRequest>,
) -> Result<Json<ModerarContribucionResponse>, AppError> {
    user.require_admin()?;
    let result = ContribucionesService::moderar(
        &state.pool,
        ModerarContribucionInput {
            moderador_id: user.user_id,
            id: request.id,
            accion: request.accion,
            nota: trim_optional(request.nota),
        },
    )
    .await?;
    Ok(Json(ModerarContribucionResponse {
        ok: true,
        relacion_id: result.relacion_id,
    }))
}

fn pagination(query: &ListarQuery) -> (i64, i64, i64) {
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    let offset = (page - 1) * limit;
    (page, limit, offset)
}

fn trim_optional(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/contribuciones", post(crear))
        .route("/contribuciones/mis", get(mis))
        .route("/contribuciones/:id", put(actualizar).delete(eliminar))
        .route("/contribuciones/edicion", post(proponer_edicion))
        .route("/contribuciones/eliminacion", post(proponer_eliminacion))
        .route("/admin/contribuciones", get(listar))
        .route("/admin/contribuciones/moderar", post(moderar))
}
