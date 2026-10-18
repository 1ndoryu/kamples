use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::algorithm::InteractionKind;
use crate::errors::AppError;
use crate::middleware::CurrentUser;
use crate::models::SampleSummary;
use crate::repositories::{BibliotecaRepository, PlayRepository};
use crate::services::build_sample_summary;
use crate::AppState;

/* [174A-58] Tracking de reproducciones (`POST /api/samples/{id}/play`).
 *
 * Port directo de `ReproduccionesController::registrar`. Diferencias:
 * - Rate limit (60/min) NO portado: pendiente cuando llegue el RateLimiter
 *   global (pendiente en el roadmap, 18AA-2).
 * - Debounce 3s manejado por `PlayRepository::register_with_debounce`.
 * - Se invoca `AlgoPlanner::register_interaction(Reproduccion)` siempre y
 *   `Completa` adicionalmente cuando el cliente reporta `completada=true`.
 */

const PLAY_DEBOUNCE_SECONDS: i64 = 3;

#[derive(Debug, Clone, Deserialize, ToSchema, Validate, Default)]
pub struct RegisterPlayRequest {
    /// Segundos de audio efectivamente escuchados antes del POST.
    #[serde(default)]
    #[validate(range(min = 0.0, max = 86_400.0))]
    pub duracion_escuchada: f32,
    /// `true` si el sample se reprodujo hasta el final (>=80% típico).
    #[serde(default)]
    pub completada: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RegisterPlayResponse {
    pub ok: bool,
    /// `true` cuando el play se fusionó con uno reciente en lugar de crear fila.
    pub debounce: bool,
    /// Detalle de los recálculos disparados por el planificador.
    pub triggered: PlayTriggered,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PlayTriggered {
    /// `true` cuando el planificador disparó el recálculo rápido.
    pub fast: bool,
    /// `true` cuando el planificador disparó el recálculo preciso.
    pub precise: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PlayedIdsResponse {
    pub data: Vec<i32>,
}

#[utoipa::path(
    post,
    path = "/api/samples/{id}/play",
    tag = "samples",
    params(("id" = i32, Path, description = "ID del sample reproducido")),
    request_body = RegisterPlayRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 201, description = "Play registrado", body = RegisterPlayResponse),
        (status = 200, description = "Play debounced (fusionado con uno reciente)", body = RegisterPlayResponse),
        (status = 400, description = "Body inválido", body = ErrorResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
    )
)]
pub async fn register_play(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(sample_id): Path<i32>,
    Json(body): Json<RegisterPlayRequest>,
) -> Result<(StatusCode, Json<RegisterPlayResponse>), AppError> {
    body.validate()
        .map_err(|err| AppError::Validation(err.to_string()))?;

    /* [304A-1] Validar existencia del sample antes de tocar `reproducciones`.
     * Sin esto, el INSERT explota con `reproducciones_sample_id_fkey` y devuelve
     * 500 al frontend, que cachea IDs de samples ya borrados (p. ej. tras un wipe
     * de la cola de extraccion). 404 es la respuesta correcta. */
    let exists: Option<(i32,)> =
        sqlx::query_as("SELECT id FROM samples WHERE id = $1 AND eliminado_en IS NULL")
            .bind(sample_id)
            .fetch_optional(&state.pool)
            .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(format!("sample {sample_id} no existe")));
    }

    /* Rate limit 60/min por usuario: pendiente en el roadmap (18AA-2), cuando
     * exista un RateLimiter global. Mientras tanto el debounce de 3s mitiga abuso. */

    let outcome = PlayRepository::register_with_debounce(
        &state.pool,
        user.user_id,
        sample_id,
        body.duracion_escuchada,
        body.completada,
        PLAY_DEBOUNCE_SECONDS,
    )
    .await?;

    let mut triggered_fast = false;
    let mut triggered_precise = false;

    if !outcome.debounced {
        PlayRepository::increment_sample_counter(&state.pool, sample_id).await?;

        let triggered = state
            .algo_planner
            .register_interaction(
                &state.pool,
                &state.redis,
                user.user_id,
                InteractionKind::Reproduccion,
            )
            .await?;
        triggered_fast |= triggered.fast;
        triggered_precise |= triggered.precise;

        if outcome.completed_now {
            let extra = state
                .algo_planner
                .register_interaction(
                    &state.pool,
                    &state.redis,
                    user.user_id,
                    InteractionKind::Completa,
                )
                .await?;
            triggered_fast |= extra.fast;
            triggered_precise |= extra.precise;
        }
    }

    let status = if outcome.debounced {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };

    Ok((
        status,
        Json(RegisterPlayResponse {
            ok: true,
            debounce: outcome.debounced,
            triggered: PlayTriggered {
                fast: triggered_fast,
                precise: triggered_precise,
            },
        }),
    ))
}

#[utoipa::path(
    get,
    path = "/api/reproducciones/ids",
    tag = "samples",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "IDs únicos de samples reproducidos por el usuario", body = PlayedIdsResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
    )
)]
pub async fn list_played_ids(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<PlayedIdsResponse>, AppError> {
    let data = PlayRepository::list_played_ids(&state.pool, user.user_id).await?;
    Ok(Json(PlayedIdsResponse { data }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/samples/:id/play", post(register_play))
        /* Alias para compatibilidad con clientes legacy que usan /reproduccion */
        .route("/samples/:id/reproduccion", post(register_play))
        .route("/reproducciones/ids", get(list_played_ids))
        .route("/reproducciones/historial", get(play_history))
}

/* [274A-17] GET /api/reproducciones/historial — Historial paginado del usuario,
 * samples completos ordenados por reproducción más reciente. */
#[derive(Debug, Clone, Deserialize, IntoParams, Default)]
#[into_params(parameter_in = Query)]
pub struct PlayHistoryQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PlayHistoryResponse {
    pub data: Vec<SampleSummary>,
    pub page: i64,
}

#[utoipa::path(
    get,
    path = "/api/reproducciones/historial",
    tag = "samples",
    params(PlayHistoryQuery),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Historial de reproducciones", body = PlayHistoryResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
    )
)]
pub async fn play_history(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<PlayHistoryQuery>,
) -> Result<Json<PlayHistoryResponse>, AppError> {
    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * per_page;

    let records =
        BibliotecaRepository::historial_reproducciones(&state.pool, user.user_id, per_page, offset)
            .await?;

    let public_base = state.public_base_url.as_deref();
    let data: Vec<SampleSummary> = records
        .into_iter()
        .map(|r| build_sample_summary(r, public_base))
        .collect();

    Ok(Json(PlayHistoryResponse { data, page }))
}
