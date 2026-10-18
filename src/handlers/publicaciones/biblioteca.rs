/* [254A-7c] Handlers de la biblioteca personal del usuario.
 *
 * Replica `BibliotecaSamplesController` (PHP):
 *   GET /api/me/coleccionados            -> listado paginado con filtros
 *   GET /api/me/coleccionados/carpetas   -> arbol primaria/secundaria con totales
 *   PUT /api/me/coleccionados/:id/carpeta -> mueve un sample a otra carpeta
 *
 * El contrato preserva el envoltorio { data: ... } esperado por el cliente
 * desktop/mobile y por el frontend web actual.
 */

use std::collections::BTreeMap;

use axum::extract::{Path, Query, State};
use axum::routing::{get, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::errors::AppError;
use crate::middleware::CurrentUser;
use crate::models::{SampleSummary, SamplesPagination};
use crate::repositories::{
    BibliotecaRepository, ColeccionadosFilters, FiltroReaccion, CARPETA_DEFAULT,
};
use crate::services::build_sample_summary;
use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/me/coleccionados", get(get_coleccionados))
        .route("/me/coleccionados/carpetas", get(get_carpetas))
        .route("/me/coleccionados/:id/carpeta", put(put_mover_carpeta))
        .route("/me/favoritos", get(get_favoritos))
        .route("/users/me/descargas", get(get_descargas))
        .route("/me/descargas", get(get_descargas))
}

#[derive(Debug, Clone, Deserialize, IntoParams, Default)]
#[into_params(parameter_in = Query)]
pub struct ColeccionadosQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub carpeta: Option<String>,
    pub orden: Option<String>,
    pub busqueda: Option<String>,
    /* [254A-7c] Filtros booleanos heredados de la API legacy. Se aceptan como
     * 1/0/true/false. */
    pub solo_encanta: Option<String>,
    pub solo_like: Option<String>,
}

fn parse_bool(s: &str) -> bool {
    matches!(s, "1" | "true" | "TRUE" | "True" | "yes" | "si")
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BibliotecaSamplesData {
    pub data: Vec<SampleSummary>,
    pub pagination: SamplesPagination,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BibliotecaSamplesResponse {
    pub data: BibliotecaSamplesData,
}

#[utoipa::path(
    get,
    path = "/api/me/coleccionados",
    tag = "biblioteca",
    params(ColeccionadosQuery),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Samples coleccionados del usuario", body = BibliotecaSamplesResponse),
        (status = 401, description = "No autenticado"),
    )
)]
pub async fn get_coleccionados(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<ColeccionadosQuery>,
) -> Result<Json<BibliotecaSamplesResponse>, AppError> {
    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(20).clamp(1, 100);

    let filtro_reaccion = if q.solo_encanta.as_deref().is_some_and(parse_bool) {
        Some(FiltroReaccion::Encanta)
    } else if q.solo_like.as_deref().is_some_and(parse_bool) {
        Some(FiltroReaccion::Like)
    } else {
        None
    };

    let filters = ColeccionadosFilters {
        user_id: user.user_id,
        page,
        per_page,
        carpeta: q.carpeta.unwrap_or_default(),
        orden: q.orden.unwrap_or_else(|| "recientes".to_string()),
        busqueda: q.busqueda.unwrap_or_default(),
        filtro_reaccion,
    };

    let records = BibliotecaRepository::coleccionados_de_usuario(&state.pool, &filters).await?;
    let total = BibliotecaRepository::contar_coleccionados(&state.pool, &filters).await?;

    let public_base = state.public_base_url.as_deref();
    let data: Vec<SampleSummary> = records
        .into_iter()
        .map(|r| build_sample_summary(r, public_base))
        .collect();

    let pages = total_pages(total, per_page);

    Ok(Json(BibliotecaSamplesResponse {
        data: BibliotecaSamplesData {
            data,
            pagination: SamplesPagination {
                page,
                per_page,
                total,
                pages,
            },
        },
    }))
}

/* [274A-7] GET /api/me/favoritos
 * Devuelve los samples a los que el usuario dio like/encanta. Mismo contrato
 * de respuesta que /me/coleccionados (BibliotecaSamplesResponse) para reusar
 * normalizadores existentes en el frontend. */
#[derive(Debug, Clone, Deserialize, IntoParams, Default)]
#[into_params(parameter_in = Query)]
pub struct FavoritosQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub orden: Option<String>,
    pub busqueda: Option<String>,
    pub solo_encanta: Option<String>,
    pub solo_like: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/me/favoritos",
    tag = "biblioteca",
    params(FavoritosQuery),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Samples favoritos del usuario", body = BibliotecaSamplesResponse),
        (status = 401, description = "No autenticado"),
    )
)]
pub async fn get_favoritos(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<FavoritosQuery>,
) -> Result<Json<BibliotecaSamplesResponse>, AppError> {
    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(20).clamp(1, 100);

    let filtro_reaccion = if q.solo_encanta.as_deref().is_some_and(parse_bool) {
        Some(FiltroReaccion::Encanta)
    } else if q.solo_like.as_deref().is_some_and(parse_bool) {
        Some(FiltroReaccion::Like)
    } else {
        None
    };

    let filters = ColeccionadosFilters {
        user_id: user.user_id,
        page,
        per_page,
        carpeta: String::new(),
        orden: q.orden.unwrap_or_else(|| "recientes".to_string()),
        busqueda: q.busqueda.unwrap_or_default(),
        filtro_reaccion,
    };

    let records = BibliotecaRepository::favoritos_de_usuario(&state.pool, &filters).await?;
    let total = BibliotecaRepository::contar_favoritos(&state.pool, &filters).await?;

    let public_base = state.public_base_url.as_deref();
    let data: Vec<SampleSummary> = records
        .into_iter()
        .map(|r| build_sample_summary(r, public_base))
        .collect();

    let pages = total_pages(total, per_page);

    Ok(Json(BibliotecaSamplesResponse {
        data: BibliotecaSamplesData {
            data,
            pagination: SamplesPagination {
                page,
                per_page,
                total,
                pages,
            },
        },
    }))
}

/* [274A-15] Query simple para descargas (subset de FavoritosQuery sin filtros de reaccion). */
#[derive(Debug, Clone, Deserialize, IntoParams, Default)]
#[into_params(parameter_in = Query)]
pub struct DescargasQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub orden: Option<String>,
}

/* [274A-15] GET /api/users/me/descargas (alias /me/descargas)
 * Lista paginada de samples descargados por el usuario.
 * Migrado desde BibliotecaSamplesController::misDescargas (PHP). */
#[utoipa::path(
    get,
    path = "/api/users/me/descargas",
    tag = "biblioteca",
    params(DescargasQuery),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Samples descargados del usuario", body = BibliotecaSamplesResponse),
        (status = 401, description = "No autenticado"),
    )
)]
pub async fn get_descargas(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<DescargasQuery>,
) -> Result<Json<BibliotecaSamplesResponse>, AppError> {
    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(20).clamp(1, 100);

    let filters = ColeccionadosFilters {
        user_id: user.user_id,
        page,
        per_page,
        carpeta: String::new(),
        orden: q.orden.unwrap_or_else(|| "recientes".to_string()),
        busqueda: String::new(),
        filtro_reaccion: None,
    };

    let records = BibliotecaRepository::descargados_de_usuario(&state.pool, &filters).await?;
    let total = BibliotecaRepository::contar_descargados(&state.pool, user.user_id).await?;

    let public_base = state.public_base_url.as_deref();
    let data: Vec<SampleSummary> = records
        .into_iter()
        .map(|r| build_sample_summary(r, public_base))
        .collect();

    let pages = total_pages(total, per_page);

    Ok(Json(BibliotecaSamplesResponse {
        data: BibliotecaSamplesData {
            data,
            pagination: SamplesPagination {
                page,
                per_page,
                total,
                pages,
            },
        },
    }))
}

fn total_pages(total: i64, per_page: i64) -> i64 {
    if per_page <= 0 || total <= 0 {
        0
    } else {
        (total + per_page - 1) / per_page
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SubcarpetaItem {
    pub nombre: String,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CarpetaItem {
    pub primaria: String,
    pub total: i64,
    pub subcarpetas: Vec<SubcarpetaItem>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CarpetasResponse {
    pub data: Vec<CarpetaItem>,
}

#[utoipa::path(
    get,
    path = "/api/me/coleccionados/carpetas",
    tag = "biblioteca",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Arbol de carpetas con totales", body = CarpetasResponse),
        (status = 401, description = "No autenticado"),
    )
)]
pub async fn get_carpetas(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<CarpetasResponse>, AppError> {
    let rows = BibliotecaRepository::carpetas_coleccionados(&state.pool, user.user_id).await?;

    /* Agrupamos en el handler para mantener el repo simple. */
    let mut tree: BTreeMap<String, (i64, BTreeMap<String, i64>)> = BTreeMap::new();
    for r in rows {
        let entry = tree.entry(r.primaria).or_insert((0, BTreeMap::new()));
        entry.0 += r.total;
        if let Some(sec) = r.secundaria {
            if !sec.is_empty() {
                *entry.1.entry(sec).or_insert(0) += r.total;
            }
        }
    }

    let data: Vec<CarpetaItem> = tree
        .into_iter()
        .map(|(primaria, (total, subs))| CarpetaItem {
            primaria,
            total,
            subcarpetas: subs
                .into_iter()
                .map(|(nombre, total)| SubcarpetaItem { nombre, total })
                .collect(),
        })
        .collect();

    Ok(Json(CarpetasResponse { data }))
}

#[derive(Debug, Clone, Deserialize, ToSchema, Validate)]
pub struct MoverCarpetaRequest {
    #[validate(length(min = 1, max = 100))]
    pub carpeta_primaria: String,
    #[validate(length(max = 100))]
    pub carpeta_secundaria: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MoverCarpetaData {
    pub movido: bool,
    pub sample_id: i32,
    pub carpeta_primaria: String,
    pub carpeta_secundaria: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct MoverCarpetaResponse {
    pub data: MoverCarpetaData,
}

#[utoipa::path(
    put,
    path = "/api/me/coleccionados/{id}/carpeta",
    tag = "biblioteca",
    params(("id" = i32, Path, description = "ID del sample")),
    request_body = MoverCarpetaRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Sample movido", body = MoverCarpetaResponse),
        (status = 400, description = "Carpeta invalida"),
        (status = 401, description = "No autenticado"),
        (status = 403, description = "El sample no esta en la biblioteca del usuario"),
        (status = 404, description = "Sample no encontrado"),
    )
)]
pub async fn put_mover_carpeta(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(sample_id): Path<i32>,
    Json(body): Json<MoverCarpetaRequest>,
) -> Result<Json<MoverCarpetaResponse>, AppError> {
    body.validate()
        .map_err(|e| AppError::BadRequest(format!("Carpeta invalida: {e}")))?;

    let primaria = body.carpeta_primaria.trim().to_string();
    if primaria.is_empty() {
        return Err(AppError::BadRequest(
            "carpeta_primaria no puede estar vacia".into(),
        ));
    }
    let secundaria = body
        .carpeta_secundaria
        .as_deref()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    let pertenece =
        BibliotecaRepository::es_coleccionado_por_usuario(&state.pool, sample_id, user.user_id)
            .await?;
    if !pertenece {
        return Err(AppError::Forbidden(
            "El sample no esta en tu biblioteca".into(),
        ));
    }

    let movido =
        BibliotecaRepository::mover_a_carpeta(&state.pool, sample_id, &primaria, &secundaria)
            .await?;
    if !movido {
        return Err(AppError::NotFound("Sample no encontrado".into()));
    }

    /* La primaria se normaliza con CARPETA_DEFAULT en el resto del sistema
     * (consultas COALESCE), pero al frontend siempre devolvemos lo que se
     * persistio. */
    let _ = CARPETA_DEFAULT;

    Ok(Json(MoverCarpetaResponse {
        data: MoverCarpetaData {
            movido: true,
            sample_id,
            carpeta_primaria: primaria,
            carpeta_secundaria: secundaria,
        },
    }))
}
