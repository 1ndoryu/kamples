/* [274A-6] Sugerencias "Más Ideas" basadas en el historial del usuario.
 *
 * Réplica funcional de `SugerenciasController::sugerenciasDescargas` (legacy):
 *   1. Reúne contexto: tags+BPM+key de samples descargados Y coleccionados.
 *   2. Calcula top 10 tags, BPM promedio (default 120), key dominante.
 *   3. Excluye los IDs ya descargados/coleccionados.
 *   4. Llama al motor de scoring de SampleRepository.
 *
 * Endpoint:
 *   GET /api/me/descargas/sugerencias?pagina=1&limite=20
 * Respuesta:
 *   { data: SampleSummary[] }   // contrato legacy preservado
 */

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::errors::AppError;
use crate::middleware::CurrentUser;
use crate::models::SampleSummary;
use crate::repositories::{
    BibliotecaRepository, CriterioScoring, SampleContextRow, SampleRepository,
};
use crate::services::build_sample_summary;
use crate::AppState;

const DEFAULT_BPM: i32 = 120;
const TOP_TAGS_LIMIT: usize = 10;
const MAX_LIMIT: i64 = 50;

#[derive(Debug, Clone, Deserialize, IntoParams, Default)]
#[into_params(parameter_in = Query)]
pub struct SugerenciasQuery {
    pub pagina: Option<i64>,
    pub limite: Option<i64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SugerenciasResponse {
    pub data: Vec<SampleSummary>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/me/descargas/sugerencias", get(sugerencias_descargas))
        .route("/me/favoritos/sugerencias", get(sugerencias_favoritos))
}

#[utoipa::path(
    get,
    path = "/api/me/descargas/sugerencias",
    tag = "suggestions",
    params(SugerenciasQuery),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Samples sugeridos a partir de descargas + coleccionados", body = SugerenciasResponse),
        (status = 401, description = "No autenticado"),
    )
)]
pub async fn sugerencias_descargas(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<SugerenciasQuery>,
) -> Result<Json<SugerenciasResponse>, AppError> {
    let pagina = q.pagina.unwrap_or(1).max(1);
    let limite = q.limite.unwrap_or(20).clamp(1, MAX_LIMIT);
    let offset = (pagina - 1) * limite;

    /* 1. Contexto: descargas + coleccionados (puede haber overlap, no se
     * deduplican porque el peso de tags/BPM debe reflejar la frecuencia
     * real de exposición — igual que el legacy con array_merge). */
    let mut contexto = BibliotecaRepository::contexto_descargas(&state.pool, user.user_id).await?;
    let coleccionados =
        BibliotecaRepository::contexto_coleccionados(&state.pool, user.user_id).await?;
    contexto.extend(coleccionados);

    if contexto.is_empty() {
        return Ok(Json(SugerenciasResponse { data: Vec::new() }));
    }

    /* 2. IDs a excluir: union de descargados + coleccionados. */
    let mut exclude: Vec<i32> =
        BibliotecaRepository::ids_descargados(&state.pool, user.user_id).await?;
    exclude.extend(BibliotecaRepository::ids_coleccionados(&state.pool, user.user_id).await?);
    exclude.sort_unstable();
    exclude.dedup();

    /* 3. Agregación: top tags, BPM promedio, key dominante. */
    let (top_tags, avg_bpm, dominant_key) = aggregate_context(&contexto);

    /* 4. Scoring + fetch. */
    let criterio = CriterioScoring {
        top_tags: &top_tags,
        avg_bpm,
        dominant_key: dominant_key.as_deref(),
        exclude_ids: &exclude,
    };
    let records = SampleRepository::find_samples_by_aggregated_scoring(
        &state.pool,
        criterio,
        limite,
        offset,
        Some(user.user_id),
    )
    .await?;

    let public_base = state.public_base_url.as_deref();
    let data: Vec<SampleSummary> = records
        .into_iter()
        .map(|r| build_sample_summary(r, public_base))
        .collect();

    Ok(Json(SugerenciasResponse { data }))
}

/* [274A-20] GET /api/me/favoritos/sugerencias — Misma lógica que sugerencias
 * de descargas pero usando likes (favoritos) como contexto y excluidos. */
#[utoipa::path(
    get,
    path = "/api/me/favoritos/sugerencias",
    tag = "suggestions",
    params(SugerenciasQuery),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Samples sugeridos a partir de favoritos", body = SugerenciasResponse),
        (status = 401, description = "No autenticado"),
    )
)]
pub async fn sugerencias_favoritos(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<SugerenciasQuery>,
) -> Result<Json<SugerenciasResponse>, AppError> {
    let pagina = q.pagina.unwrap_or(1).max(1);
    let limite = q.limite.unwrap_or(20).clamp(1, MAX_LIMIT);
    let offset = (pagina - 1) * limite;

    let contexto = BibliotecaRepository::contexto_favoritos(&state.pool, user.user_id).await?;
    if contexto.is_empty() {
        return Ok(Json(SugerenciasResponse { data: Vec::new() }));
    }
    let exclude = BibliotecaRepository::ids_favoritos(&state.pool, user.user_id).await?;
    let (top_tags, avg_bpm, dominant_key) = aggregate_context(&contexto);

    let criterio = CriterioScoring {
        top_tags: &top_tags,
        avg_bpm,
        dominant_key: dominant_key.as_deref(),
        exclude_ids: &exclude,
    };
    let records = SampleRepository::find_samples_by_aggregated_scoring(
        &state.pool,
        criterio,
        limite,
        offset,
        Some(user.user_id),
    )
    .await?;

    let public_base = state.public_base_url.as_deref();
    let data: Vec<SampleSummary> = records
        .into_iter()
        .map(|r| build_sample_summary(r, public_base))
        .collect();

    Ok(Json(SugerenciasResponse { data }))
}

fn aggregate_context(contexto: &[SampleContextRow]) -> (Vec<String>, i32, Option<String>) {
    let mut tag_counts: HashMap<&str, usize> = HashMap::new();
    let mut bpm_sum: i64 = 0;
    let mut bpm_n: i64 = 0;
    let mut key_counts: HashMap<&str, usize> = HashMap::new();

    for row in contexto {
        for tag in &row.tags {
            *tag_counts.entry(tag.as_str()).or_insert(0) += 1;
        }
        if let Some(b) = row.bpm {
            bpm_sum += i64::from(b);
            bpm_n += 1;
        }
        if let Some(k) = &row.music_key {
            if !k.is_empty() {
                *key_counts.entry(k.as_str()).or_insert(0) += 1;
            }
        }
    }

    /* Top N tags por frecuencia descendente. */
    let mut tag_vec: Vec<(&str, usize)> = tag_counts.into_iter().collect();
    tag_vec.sort_by_key(|b| std::cmp::Reverse(b.1));
    let top_tags: Vec<String> = tag_vec
        .into_iter()
        .take(TOP_TAGS_LIMIT)
        .map(|(t, _)| t.to_string())
        .collect();

    let avg_bpm = if bpm_n > 0 {
        i32::try_from(bpm_sum / bpm_n).unwrap_or(DEFAULT_BPM)
    } else {
        DEFAULT_BPM
    };

    let dominant_key = key_counts
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(k, _)| k.to_string());

    (top_tags, avg_bpm, dominant_key)
}
