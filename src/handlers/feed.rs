/* [174A-56] Endpoints del feed personalizado.
 *
 * Tres rutas:
 *   - `GET /api/feed`          → feed personalizado (requiere auth).
 *   - `GET /api/me/feed`       → alias del anterior (compatibilidad legado).
 *   - `GET /api/samples/random` ya está en sample_catalog (no se duplica).
 *
 * Internamente delegan en `RecommenderService::feed`, que ya implementa la
 * cascada cache fresh → stale → cómputo síncrono. El handler solo valida
 * paginación y empaqueta el resultado.
 *
 * Decisión: usar `RecommenderConfig::legacy_defaults()` por ahora (no en
 * AppState). Si en el futuro se necesita exponer configurabilidad por env,
 * mover a `AppState::recommender_config`. */

use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::algorithm::recommender::{RankedSample, RecommenderConfig, RecommenderService};
use crate::errors::AppError;
#[allow(unused_imports)]
use crate::errors::ErrorResponse;
use crate::middleware::CurrentUser;
use crate::models::SampleSummary;
use crate::repositories::{
    ReportRepository, SampleListFilters, SampleRepository, SampleSortOrder, SampleTextSearch,
    AUTO_HIDE_SAMPLE_REPORT_THRESHOLD,
};
use crate::services::build_sample_summary;
use crate::services::SampleCatalogService;
use crate::AppState;

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct FeedQuery {
    /// Tamaño de página. Default: 20, máximo: 100.
    pub limit: Option<i64>,
    /// Offset desde 0. Default: 0.
    pub offset: Option<i64>,
    /// Pagina 1-based (legacy). Si se envia, gana sobre `offset`.
    pub page: Option<i64>,
    /// Alias legado de `limit`.
    pub per_page: Option<i64>,
    /// Modo del feed: `descubrir` (default), `recientes`, `trending`.
    pub tipo: Option<String>,
    /// Busqueda full-text (legado).
    pub busqueda: Option<String>,
    /// Variante normalizada de la busqueda (sinonimos del frontend).
    pub busqueda_norm: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FeedResponse {
    pub items: Vec<SampleSummary>,
    pub limit: i64,
    pub offset: i64,
    pub total: i64,
    pub hay_mas: bool,
}

const DEFAULT_LIMIT: i64 = 20;
const DEFAULT_LIMIT_USIZE: usize = 20;
const MAX_LIMIT: i64 = 100;

fn normalize_pagination(query: &FeedQuery) -> (usize, usize) {
    /* Acepta `limit` o `per_page` (alias legacy). */
    let raw_limit = query.limit.or(query.per_page).unwrap_or(DEFAULT_LIMIT);
    let limit = raw_limit.clamp(1, MAX_LIMIT);
    /* `page` (1-based) tiene precedencia sobre `offset` para paridad con el
     * cliente legacy que envia ambos en la misma request. */
    let offset = match query.page {
        Some(page) if page >= 1 => (page - 1) * limit,
        _ => query.offset.unwrap_or(0).max(0),
    };
    (
        usize::try_from(limit).unwrap_or(DEFAULT_LIMIT_USIZE),
        usize::try_from(offset).unwrap_or(0),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FeedMode {
    Descubrir,
    Recientes,
    Trending,
}

fn parse_feed_mode(raw: Option<&str>) -> FeedMode {
    match raw.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("recientes") => FeedMode::Recientes,
        Some("trending" | "destacados") => FeedMode::Trending,
        _ => FeedMode::Descubrir,
    }
}

fn build_text_search(query: &FeedQuery) -> Result<(Option<SampleTextSearch>, Vec<String>), AppError> {
    let Some(raw) = query.busqueda.as_ref() else {
        return Ok((None, Vec::new()));
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok((None, Vec::new()));
    }

    /* [266A-1] Parse negative tag prefixes (-tag) from feed search. */
    let (clean_parts, exclude_tags) = parse_search_exclusions_feed(trimmed);
    /* Only negative terms — no positive search, but still return exclude_tags. */
    let Some(clean_str) = clean_parts else {
        return Ok((None, exclude_tags));
    };

    if clean_str.chars().count() < 2 {
        return Err(AppError::Validation(
            "busqueda debe tener al menos 2 caracteres".into(),
        ));
    }
    let normalized = query
        .busqueda_norm
        .as_ref()
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty() && v != &clean_str.to_ascii_lowercase());
    Ok((Some(SampleTextSearch::new(clean_str, normalized)), exclude_tags))
}

fn parse_search_exclusions_feed(trimmed: &str) -> (Option<String>, Vec<String>) {
    if !trimmed.contains('-') {
        return (Some(trimmed.to_string()), Vec::new());
    }

    let mut positive_parts = Vec::new();
    let mut exclude_tags = Vec::new();

    for part in trimmed.split_whitespace() {
        if let Some(tag) = part.strip_prefix('-') {
            let normalized = tag.trim().to_ascii_lowercase();
            if !normalized.is_empty() && normalized.len() <= 50 {
                exclude_tags.push(normalized);
            }
        } else {
            positive_parts.push(part);
        }
    }

    let clean = if positive_parts.is_empty() {
        None
    } else {
        let joined = positive_parts.join(" ");
        if joined.is_empty() { None } else { Some(joined) }
    };

    (clean, exclude_tags)
}

#[utoipa::path(
    get,
    path = "/api/feed",
    params(FeedQuery),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Feed personalizado del usuario autenticado", body = FeedResponse),
        (status = 401, description = "Autenticación requerida", body = ErrorResponse)
    )
)]
pub async fn get_feed(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<FeedQuery>,
) -> Result<Json<FeedResponse>, AppError> {
    let (limit, offset) = normalize_pagination(&query);
    let mode = parse_feed_mode(query.tipo.as_deref());
    let (search, exclude_tags) = build_text_search(&query)?;

    /* [304A-2] Solo el modo "descubrir" sin busqueda activa pasa por el motor
     * de recomendacion. Recientes/trending y cualquier busqueda usan el listado
     * publico estandar con ordenamiento explicito. Antes este handler ignoraba
     * `tipo` por completo y los 3 botones del UI devolvian lo mismo. */
    let use_recommender = matches!(mode, FeedMode::Descubrir) && search.is_none();

    if use_recommender {
        let config = RecommenderConfig::legacy_defaults();
        let ranked_items = RecommenderService::feed(
            state.pool.clone(),
            state.redis.clone(),
            current_user.user_id,
            limit,
            offset,
            &config,
        )
        .await?;
        let hay_mas = ranked_items.len() == limit;
        let visible_ranked_items =
            filter_hidden_samples(&state, current_user.user_id, ranked_items).await?;
        let sample_ids = visible_ranked_items
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>();
        let items = SampleCatalogService::list_public_samples_by_ids(
            &state.pool,
            state.public_base_url.as_deref(),
            Some(current_user.user_id),
            &sample_ids,
        )
        .await?;
        /* [166B-1] Total activo para el contador del frontend.
         * En modo recomendador no hay filtros, así que contamos todos los samples
         * activos públicos de una pasada. */
        let total: i64 = SampleRepository::contar_activos(&state.pool).await?;

        return Ok(Json(FeedResponse {
            items,
            limit: i64::try_from(limit).unwrap_or(DEFAULT_LIMIT),
            offset: i64::try_from(offset).unwrap_or(0),
            total,
            hay_mas,
        }));
    }

    /* Recientes / trending / busqueda → SampleListFilters con sort explicito.
     * `page` se calcula desde el offset normalizado para reusar la paginacion
     * 1-based del repositorio sin duplicar logica. */
    let page =
        (i64::try_from(offset).unwrap_or(0) / i64::try_from(limit).unwrap_or(DEFAULT_LIMIT)) + 1;
    let sort = match mode {
        FeedMode::Trending => SampleSortOrder::Trending,
        FeedMode::Recientes => SampleSortOrder::Recent,
        FeedMode::Descubrir => SampleSortOrder::Smart,
    };
    let filters = SampleListFilters {
        page,
        per_page: i64::try_from(limit).unwrap_or(DEFAULT_LIMIT),
        viewer_id: Some(current_user.user_id),
        search,
        bpm: None,
        music_key: None,
        sample_type: None,
        tags: Vec::new(),
        exclude_tags,
        premium: None,
        creator: None,
        sort,
    };
    let result = SampleRepository::list_public_samples(&state.pool, &filters).await?;
    let total = result.total;
    let items: Vec<SampleSummary> = result
        .items
        .into_iter()
        .map(|record| build_sample_summary(record, state.public_base_url.as_deref()))
        .collect();
    let hay_mas =
        (i64::try_from(offset).unwrap_or(0) + i64::try_from(items.len()).unwrap_or(0)) < total;

    Ok(Json(FeedResponse {
        items,
        limit: i64::try_from(limit).unwrap_or(DEFAULT_LIMIT),
        offset: i64::try_from(offset).unwrap_or(0),
        total,
        hay_mas,
    }))
}

#[utoipa::path(
    get,
    path = "/api/me/feed",
    params(FeedQuery),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Alias de /api/feed (compatibilidad con cliente legado)", body = FeedResponse),
        (status = 401, description = "Autenticación requerida", body = ErrorResponse)
    )
)]
pub async fn get_me_feed(
    state: State<AppState>,
    current_user: CurrentUser,
    query: Query<FeedQuery>,
) -> Result<Json<FeedResponse>, AppError> {
    get_feed(state, current_user, query).await
}

/* [274A-18] Alias `/feed/inicio` consumido por apiSocial.ts. Comparte
 * lógica con get_feed; mantiene el contrato del frontend. */
#[utoipa::path(
    get,
    path = "/api/feed/inicio",
    params(FeedQuery),
    tag = "feed",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Alias de /api/feed (compatibilidad cliente legado)", body = FeedResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
    )
)]
pub async fn get_feed_inicio(
    state: State<AppState>,
    current_user: CurrentUser,
    query: Query<FeedQuery>,
) -> Result<Json<FeedResponse>, AppError> {
    get_feed(state, current_user, query).await
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FeedRecargarResponse {
    pub ok: bool,
}

/* [274A-19] POST /feed/recargar — invalida cache fresco del feed para forzar
 * recálculo en la próxima petición. Port de SamplesController::recargarFeed. */
#[utoipa::path(
    post,
    path = "/api/feed/recargar",
    tag = "feed",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Cache fresco invalidado", body = FeedRecargarResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
    )
)]
pub async fn recargar_feed(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<FeedRecargarResponse>, AppError> {
    RecommenderService::invalidate_user_feed(&state.redis, user.user_id).await?;
    Ok(Json(FeedRecargarResponse { ok: true }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/feed", get(get_feed))
        .route("/feed/inicio", get(get_feed_inicio))
        .route("/feed/recargar", post(recargar_feed))
        .route("/me/feed", get(get_me_feed))
}

async fn filter_hidden_samples(
    state: &AppState,
    viewer_id: i32,
    items: Vec<RankedSample>,
) -> Result<Vec<RankedSample>, AppError> {
    let sample_ids = items.iter().map(|item| item.id).collect::<Vec<_>>();
    let pending_counts =
        ReportRepository::pending_counts_for_targets(&state.pool, "sample", &sample_ids).await?;

    Ok(items
        .into_iter()
        .filter(|item| {
            item.creador_id == viewer_id
                || pending_counts.get(&item.id).copied().unwrap_or(0)
                    < AUTO_HIDE_SAMPLE_REPORT_THRESHOLD
        })
        .collect())
}
