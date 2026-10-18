/* sentinel-disable-file directory-size — modulo legacy dentro de `handlers/`;
 * mover todo el arbol a subdirectorios es una tarea arquitectonica separada. */
/* [264A-1] Endpoints admin para mutar la tabla `app_config`.
 *
 * Permite ajustar en caliente los parametros del scraper Python
 * (intervalo de ciclo, tamano de lote, on/off) sin redeploy.
 *
 * Tambien expone `/api/admin/extraccion/stats`, una vista agregada del
 * estado actual de `cola_extraccion_samples` para visibilidad operacional.
 *
 * Auth: admin via JWT (extractor CurrentUser + require_admin).
 */

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use utoipa::ToSchema;

use crate::errors::AppError;
use crate::middleware::CurrentUser;
use crate::repositories::{AppConfigRepository, ColaExtraccionRepository};
use crate::AppState;

const PREFIX_EXTRACCION: &str = "extraccion_";
const PREFIX_SCRAPING: &str = "scraping_";

/// Bloque de configuracion de extraccion expuesto al panel admin.
#[derive(Debug, Serialize, Deserialize, ToSchema, Default)]
pub struct ExtraccionConfig {
    /// Segundos entre ciclos del pipeline de extraccion. Minimo 5.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intervalo_seg: Option<i64>,
    /// Items procesados por ciclo. Rango sugerido 1..=200.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lote_size: Option<i64>,
    /// Si false, el scraper salta los ciclos de extraccion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// Bloque de configuracion del scraper de WhoSampled.
#[derive(Debug, Serialize, Deserialize, ToSchema, Default)]
pub struct ScrapingConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intervalo_seg: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lote_size: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

/// Estadisticas agregadas del estado actual de la cola.
#[derive(Debug, Serialize, ToSchema)]
pub struct ExtraccionStats {
    pub pendientes: i64,
    pub en_proceso: i64,
    pub revision_humana: i64,
    pub extraido_sin_publicar: i64,
    pub completados_24h: i64,
    pub errores_24h: i64,
    pub total: i64,
}

/* GET /api/admin/config/extraccion ------------------------------------- */

#[utoipa::path(
    get,
    path = "/api/admin/config/extraccion",
    tag = "admin-config",
    responses(
        (status = 200, body = ExtraccionConfig),
        (status = 401, description = "No autenticado"),
        (status = 403, description = "Requiere rol admin"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_extraccion_config(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<ExtraccionConfig>, AppError> {
    user.require_admin()?;
    let entries = AppConfigRepository::list_by_prefix(&state.pool, PREFIX_EXTRACCION).await?;
    Ok(Json(parse_extraccion(&entries)))
}

#[utoipa::path(
    put,
    path = "/api/admin/config/extraccion",
    tag = "admin-config",
    request_body = ExtraccionConfig,
    responses(
        (status = 200, body = ExtraccionConfig),
        (status = 400, description = "Parametros invalidos"),
        (status = 401, description = "No autenticado"),
        (status = 403, description = "Requiere rol admin"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn put_extraccion_config(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(payload): Json<ExtraccionConfig>,
) -> Result<Json<ExtraccionConfig>, AppError> {
    user.require_admin()?;

    if let Some(v) = payload.intervalo_seg {
        if !(5..=86_400).contains(&v) {
            return Err(AppError::BadRequest(
                "intervalo_seg debe estar entre 5 y 86400".into(),
            ));
        }
        AppConfigRepository::upsert(
            &state.pool,
            "extraccion_intervalo_seg",
            &serde_json::Value::from(v),
        )
        .await?;
    }
    if let Some(v) = payload.lote_size {
        if !(1..=500).contains(&v) {
            return Err(AppError::BadRequest(
                "lote_size debe estar entre 1 y 500".into(),
            ));
        }
        AppConfigRepository::upsert(
            &state.pool,
            "extraccion_lote_size",
            &serde_json::Value::from(v),
        )
        .await?;
    }
    if let Some(v) = payload.enabled {
        AppConfigRepository::upsert(
            &state.pool,
            "extraccion_enabled",
            &serde_json::Value::from(v),
        )
        .await?;
    }

    let entries = AppConfigRepository::list_by_prefix(&state.pool, PREFIX_EXTRACCION).await?;
    Ok(Json(parse_extraccion(&entries)))
}

/* GET /api/admin/config/scraping --------------------------------------- */

#[utoipa::path(
    get,
    path = "/api/admin/config/scraping",
    tag = "admin-config",
    responses(
        (status = 200, body = ScrapingConfig),
        (status = 401, description = "No autenticado"),
        (status = 403, description = "Requiere rol admin"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_scraping_config(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<ScrapingConfig>, AppError> {
    user.require_admin()?;
    let entries = AppConfigRepository::list_by_prefix(&state.pool, PREFIX_SCRAPING).await?;
    Ok(Json(parse_scraping(&entries)))
}

#[utoipa::path(
    put,
    path = "/api/admin/config/scraping",
    tag = "admin-config",
    request_body = ScrapingConfig,
    responses(
        (status = 200, body = ScrapingConfig),
        (status = 400, description = "Parametros invalidos"),
        (status = 401, description = "No autenticado"),
        (status = 403, description = "Requiere rol admin"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn put_scraping_config(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(payload): Json<ScrapingConfig>,
) -> Result<Json<ScrapingConfig>, AppError> {
    user.require_admin()?;

    if let Some(v) = payload.intervalo_seg {
        if !(30..=86_400).contains(&v) {
            return Err(AppError::BadRequest(
                "intervalo_seg debe estar entre 30 y 86400".into(),
            ));
        }
        AppConfigRepository::upsert(
            &state.pool,
            "scraping_intervalo_seg",
            &serde_json::Value::from(v),
        )
        .await?;
    }
    if let Some(v) = payload.lote_size {
        if !(1..=200).contains(&v) {
            return Err(AppError::BadRequest(
                "lote_size debe estar entre 1 y 200".into(),
            ));
        }
        AppConfigRepository::upsert(
            &state.pool,
            "scraping_lote_size",
            &serde_json::Value::from(v),
        )
        .await?;
    }
    if let Some(v) = payload.enabled {
        AppConfigRepository::upsert(&state.pool, "scraping_enabled", &serde_json::Value::from(v))
            .await?;
    }

    let entries = AppConfigRepository::list_by_prefix(&state.pool, PREFIX_SCRAPING).await?;
    Ok(Json(parse_scraping(&entries)))
}

/* GET /api/admin/extraccion/stats -------------------------------------- */

#[utoipa::path(
    get,
    path = "/api/admin/extraccion/stats",
    tag = "admin-config",
    responses(
        (status = 200, body = ExtraccionStats),
        (status = 401, description = "No autenticado"),
        (status = 403, description = "Requiere rol admin"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_extraccion_stats(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<ExtraccionStats>, AppError> {
    user.require_admin()?;

    let row = ColaExtraccionRepository::estadisticas(&state.pool).await?;

    Ok(Json(ExtraccionStats {
        pendientes: row.pendientes,
        en_proceso: row.en_proceso,
        revision_humana: row.revision_humana,
        extraido_sin_publicar: row.extraido_sin_publicar,
        completados_24h: row.completados_24h,
        errores_24h: row.errores_24h,
        total: row.total,
    }))
}

/* Helpers ------------------------------------------------------------- */

fn parse_extraccion(entries: &[crate::repositories::AppConfigEntry]) -> ExtraccionConfig {
    let map: BTreeMap<&str, &serde_json::Value> = entries
        .iter()
        .map(|e| (e.clave.as_str(), &e.valor))
        .collect();
    ExtraccionConfig {
        intervalo_seg: map.get("extraccion_intervalo_seg").and_then(|v| v.as_i64()),
        lote_size: map.get("extraccion_lote_size").and_then(|v| v.as_i64()),
        enabled: map.get("extraccion_enabled").and_then(|v| v.as_bool()),
    }
}

fn parse_scraping(entries: &[crate::repositories::AppConfigEntry]) -> ScrapingConfig {
    let map: BTreeMap<&str, &serde_json::Value> = entries
        .iter()
        .map(|e| (e.clave.as_str(), &e.valor))
        .collect();
    ScrapingConfig {
        intervalo_seg: map.get("scraping_intervalo_seg").and_then(|v| v.as_i64()),
        lote_size: map.get("scraping_lote_size").and_then(|v| v.as_i64()),
        enabled: map.get("scraping_enabled").and_then(|v| v.as_bool()),
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/config/extraccion",
            get(get_extraccion_config).put(put_extraccion_config),
        )
        .route(
            "/api/admin/config/scraping",
            get(get_scraping_config).put(put_scraping_config),
        )
        .route("/api/admin/extraccion/stats", get(get_extraccion_stats))
}
