/* [274A-54..58] Endpoints dev del modulo Sample Discovery.
 * Port real de DevController.php: purga tablas discovery, lanza Scrapy en
 * background, procesa cola, encola recorte bilateral y publica extracciones. */

use axum::extract::State;
use axum::routing::{delete, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::errors::AppError;
#[allow(unused_imports)]
use crate::errors::ErrorResponse;
use crate::middleware::CurrentUser;
use crate::repositories::DevToolsRepository;
use crate::services::dev::process::{
    spawn_extractor_pipeline, spawn_scrapy, spider_para_tipo, validate_spider,
    validate_whosampled_url,
};
use crate::services::extraccion_publisher::{ExtraccionPublisherService, ItemPublicacion};
use crate::AppState;

#[derive(Debug, Serialize, ToSchema)]
pub struct DevPurgarCancionesResponse {
    pub ok: bool,
    pub mensaje: String,
    pub tablas: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DevScraperRunRequest {
    #[serde(default = "default_spider")]
    pub spider: String,
    #[serde(default)]
    pub limit: i64,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DevScraperRunResponse {
    pub ok: bool,
    pub mensaje: String,
    pub pid: Option<u32>,
    pub log: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DevScraperColaResponse {
    pub ok: bool,
    pub cola_vacia: bool,
    pub mensaje: String,
    pub url: Option<String>,
    pub tipo: Option<String>,
    pub spider: Option<String>,
    pub pid: Option<u32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DevRecorteGenerarRequest {
    pub relacion_id: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DevRecorteGenerarResponse {
    pub ok: bool,
    pub mensaje: String,
    pub encolados: usize,
    pub cola_ids: Vec<i32>,
    pub pid: Option<u32>,
    pub log: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DevExtraccionPublicarRequest {
    #[serde(default = "default_publicar_limit")]
    pub limit: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DevExtraccionPublicarResponse {
    pub ok: bool,
    pub publicados: usize,
    pub errores: usize,
    pub mensaje: Option<String>,
    pub resultados: Vec<ItemPublicacion>,
}

fn default_spider() -> String {
    "hot_samples".to_string()
}

const fn default_publicar_limit() -> i64 {
    10
}

#[utoipa::path(
    delete, path = "/api/dev/canciones", tag = "dev",
    security(("bearer_auth" = [])),
    responses((status = 200, body = DevPurgarCancionesResponse), (status = 403, body = ErrorResponse))
)]
pub async fn purgar_canciones(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<DevPurgarCancionesResponse>, AppError> {
    user.require_admin()?;
    DevToolsRepository::purgar_canciones(&state.pool).await?;
    Ok(Json(DevPurgarCancionesResponse {
        ok: true,
        mensaje: "Tablas truncadas correctamente.".into(),
        tablas: vec![
            "relaciones_sample".into(),
            "canciones_artistas".into(),
            "canciones".into(),
            "artistas_musicales".into(),
            "scraping_log".into(),
            "cola_extraccion_samples".into(),
        ],
    }))
}

#[utoipa::path(
    post, path = "/api/dev/scraper/run", tag = "dev",
    request_body = DevScraperRunRequest,
    security(("bearer_auth" = [])),
    responses((status = 200, body = DevScraperRunResponse), (status = 400, body = ErrorResponse), (status = 403, body = ErrorResponse))
)]
pub async fn ejecutar_scraper(
    State(_state): State<AppState>,
    user: CurrentUser,
    Json(mut request): Json<DevScraperRunRequest>,
) -> Result<Json<DevScraperRunResponse>, AppError> {
    user.require_admin()?;
    let mut extra_args = Vec::new();
    if let Some(url) = request.url.as_deref().filter(|url| !url.trim().is_empty()) {
        validate_whosampled_url(url)?;
        request.spider = "track".into();
        extra_args.extend(["-a".into(), format!("start_url={url}")]);
        extra_args.extend(["-s".into(), "DEPTH_LIMIT=2".into()]);
    }
    validate_spider(&request.spider)?;
    if request.limit > 0 {
        extra_args.extend([
            "-s".into(),
            format!("CLOSESPIDER_ITEMCOUNT={}", request.limit),
        ]);
    }
    let started = spawn_scrapy(&request.spider, &extra_args, "scraper-output")?;
    Ok(Json(DevScraperRunResponse {
        ok: true,
        mensaje: format!("Spider '{}' iniciado en segundo plano.", request.spider),
        pid: started.pid,
        log: Some(started.log_display),
    }))
}

#[utoipa::path(
    post, path = "/api/dev/scraper/cola", tag = "dev",
    security(("bearer_auth" = [])),
    responses((status = 200, body = DevScraperColaResponse), (status = 403, body = ErrorResponse))
)]
pub async fn procesar_cola_scraper(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<DevScraperColaResponse>, AppError> {
    user.require_admin()?;
    let pendiente = DevToolsRepository::tomar_pendiente_scraping(&state.pool).await?;
    let (url_relativa, tipo, cola_vacia) = pendiente.map_or_else(
        || ("/hot-samples/".to_string(), "hot_samples".to_string(), true),
        |row| (row.url, row.tipo_pagina, false),
    );
    let url_completa = if url_relativa.starts_with("http") {
        url_relativa.clone()
    } else {
        format!("https://www.whosampled.com{url_relativa}")
    };
    let (spider, mut args) = spider_para_tipo(&tipo, &url_completa, true)?;
    if cola_vacia && spider == "hot_samples" {
        args = vec!["-s".into(), "CLOSESPIDER_PAGECOUNT=1".into()];
    }
    let started = spawn_scrapy(&spider, &args, "scraper-output")?;
    Ok(Json(DevScraperColaResponse {
        ok: true,
        cola_vacia,
        mensaje: format!("Procesando: [{tipo}] {url_relativa}"),
        url: Some(url_relativa),
        tipo: Some(tipo),
        spider: Some(spider),
        pid: started.pid,
    }))
}

#[utoipa::path(
    post, path = "/api/dev/recorte/generar", tag = "dev",
    request_body = DevRecorteGenerarRequest,
    security(("bearer_auth" = [])),
    responses((status = 200, body = DevRecorteGenerarResponse), (status = 400, body = ErrorResponse), (status = 403, body = ErrorResponse), (status = 404, body = ErrorResponse))
)]
pub async fn generar_recorte(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(request): Json<DevRecorteGenerarRequest>,
) -> Result<Json<DevRecorteGenerarResponse>, AppError> {
    user.require_admin()?;
    let cola_ids =
        DevToolsRepository::encolar_recorte_bilateral(&state.pool, request.relacion_id).await?;
    if cola_ids.is_empty() {
        return Ok(Json(DevRecorteGenerarResponse {
            ok: true,
            mensaje: "Samples ya generados para ambos lados o ambos lados sin fuente de audio disponible.".into(),
            encolados: 0,
            cola_ids,
            pid: None,
            log: None,
        }));
    }
    let started = spawn_extractor_pipeline(cola_ids.len())?;
    Ok(Json(DevRecorteGenerarResponse {
        ok: true,
        mensaje: "Pipeline iniciado. Publicacion automatica cuando el audio este listo.".into(),
        encolados: cola_ids.len(),
        cola_ids,
        pid: started.pid,
        log: Some(started.log_display),
    }))
}

#[utoipa::path(
    post, path = "/api/dev/extraccion/publicar", tag = "dev",
    request_body = DevExtraccionPublicarRequest,
    security(("bearer_auth" = [])),
    responses((status = 200, body = DevExtraccionPublicarResponse), (status = 403, body = ErrorResponse))
)]
pub async fn publicar_extracciones(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(request): Json<DevExtraccionPublicarRequest>,
) -> Result<Json<DevExtraccionPublicarResponse>, AppError> {
    user.require_admin()?;
    let resultado = ExtraccionPublisherService::publicar_pendientes(
        &state.pool,
        &state.storage,
        request.limit.clamp(1, 50),
    )
    .await?;
    let mensaje = if resultado.publicados == 0 && resultado.errores == 0 {
        Some("Sin extracciones pendientes de publicar.".to_string())
    } else {
        None
    };
    Ok(Json(DevExtraccionPublicarResponse {
        ok: true,
        publicados: resultado.publicados,
        errores: resultado.errores,
        mensaje,
        resultados: resultado.items,
    }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/dev/canciones", delete(purgar_canciones))
        .route("/dev/scraper/run", post(ejecutar_scraper))
        .route("/dev/scraper/cola", post(procesar_cola_scraper))
        .route("/dev/recorte/generar", post(generar_recorte))
        .route("/dev/extraccion/publicar", post(publicar_extracciones))
}
