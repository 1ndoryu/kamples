/* [174A-64] Colecciones — port mínimo de ColeccionesCrudController.php.
 *
 * Endpoints:
 * - POST   /api/colecciones                            Crear
 * - GET    /api/colecciones                            Listar mías
 * - GET    /api/colecciones/:id                        Obtener (público o propio)
 * - PUT    /api/colecciones/:id                        Actualizar
 * - DELETE /api/colecciones/:id                        Soft-delete
 * - POST   /api/colecciones/:id/samples                Agregar sample
 * - DELETE /api/colecciones/:id/samples/:sample_id     Quitar sample
 * - GET    /api/colecciones/:id/samples                Listar samples (en orden)
 *
 * NO portado:
 * - Optimistic locking (version) — pendiente en el roadmap (18AA-2).
 * - Sync changelog.
 * - Subir imagen multipart.
 * - Admin override.
 * - Eliminación con opciones (manejo hijas, borrar samples).
 */

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::errors::AppError;
use crate::middleware::{CurrentUser, OptionalUser};
use crate::models::SampleSummary;
use crate::repositories::{
    Coleccion, ColeccionSample, ColeccionesRepository, CriterioScoring, SampleContextRow,
    SampleRepository, SavedColeccion, SavedCollectionsRepository,
};
use crate::services::build_sample_summary;
use crate::AppState;

pub mod legacy;
pub mod zip;
pub use zip::descargar_zip_coleccion;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateColeccionRequest {
    pub nombre: String,
    pub descripcion: Option<String>,
    #[serde(default = "default_true")]
    pub publica: bool,
    pub parent_id: Option<i64>,
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, ToSchema)]
#[allow(clippy::option_option)] // serde necesita Option<Option<T>> para distinguir ausente vs null
pub struct UpdateColeccionRequest {
    pub nombre: Option<String>,
    /* Triple-state: ausente = no cambiar; null = poner NULL; valor = setear. */
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub descripcion: Option<Option<String>>,
    pub publica: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub imagen_url: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub parent_id: Option<Option<i64>>,
}

/* Helper para distinguir "campo ausente" vs "campo presente con null".
 * Sin esto, serde colapsa ambos a None y no podemos hacer "set to null". */
#[allow(clippy::option_option)]
fn deserialize_optional_field<'de, T, D>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Ok(Some(Option::deserialize(d)?))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OkResponse {
    pub ok: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddSampleRequest {
    #[serde(alias = "sampleId")]
    pub sample_id: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ColeccionListResponse {
    pub items: Vec<Coleccion>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ColeccionSamplesResponse {
    pub items: Vec<ColeccionSample>,
}

#[utoipa::path(
    post, path = "/api/colecciones", tag = "colecciones",
    request_body = CreateColeccionRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 201, body = Coleccion),
        (status = 400, description = "Datos inválidos"),
        (status = 409, description = "Nombre duplicado en la misma ubicación"),
    )
)]
pub async fn create_coleccion(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<CreateColeccionRequest>,
) -> Result<(StatusCode, Json<Coleccion>), AppError> {
    let nombre = body.nombre.trim();
    if nombre.is_empty() || nombre.len() > 200 {
        return Err(AppError::BadRequest(
            "nombre requerido (1..200 chars)".into(),
        ));
    }
    if let Some(d) = &body.descripcion {
        if d.len() > 2000 {
            return Err(AppError::BadRequest(
                "descripcion demasiado larga (>2000)".into(),
            ));
        }
    }
    let col = ColeccionesRepository::create(
        &state.pool,
        user.user_id,
        nombre,
        body.descripcion.as_deref(),
        body.publica,
        body.parent_id,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(col)))
}

#[allow(dead_code)]
#[utoipa::path(
    get, path = "/api/colecciones", tag = "colecciones",
    security(("bearer_auth" = [])),
    responses((status = 200, body = ColeccionListResponse))
)]
pub async fn list_my_colecciones(
    State(state): State<AppState>,
    user: CurrentUser,
) -> Result<Json<ColeccionListResponse>, AppError> {
    let items = ColeccionesRepository::list_by_user(&state.pool, user.user_id).await?;
    Ok(Json(ColeccionListResponse { items }))
}

#[allow(dead_code)]
#[utoipa::path(
    get, path = "/api/colecciones/{id}", tag = "colecciones",
    params(("id" = i64, Path, description = "ID de la coleccion")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, body = Coleccion),
        (status = 403, description = "Privada y no es del usuario"),
        (status = 404, description = "No encontrada"),
    )
)]
pub async fn get_coleccion(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<Json<Coleccion>, AppError> {
    let col = ColeccionesRepository::fetch(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("coleccion {id} no existe")))?;
    if !col.publica && col.usuario_id != user.user_id {
        return Err(AppError::Forbidden("coleccion privada".into()));
    }
    Ok(Json(col))
}

#[utoipa::path(
    put, path = "/api/colecciones/{id}", tag = "colecciones",
    params(("id" = i64, Path, description = "ID de la coleccion")),
    request_body = UpdateColeccionRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, body = OkResponse),
        (status = 403, description = "No autorizado"),
        (status = 404, description = "No encontrada"),
        (status = 409, description = "Nombre duplicado"),
    )
)]
pub async fn update_coleccion(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    Json(body): Json<UpdateColeccionRequest>,
) -> Result<Json<OkResponse>, AppError> {
    if !ColeccionesRepository::is_owner(&state.pool, id, user.user_id).await? {
        return Err(AppError::Forbidden(
            "no eres dueño de esta coleccion".into(),
        ));
    }
    if let Some(n) = &body.nombre {
        let n = n.trim();
        if n.is_empty() || n.len() > 200 {
            return Err(AppError::BadRequest("nombre invalido".into()));
        }
    }
    /* parent_id propio = circular */
    if let Some(Some(pid)) = body.parent_id {
        if pid == id {
            return Err(AppError::BadRequest(
                "una coleccion no puede ser su propio padre".into(),
            ));
        }
        ColeccionesRepository::check_parent_valid(&state.pool, user.user_id, pid).await?;
    }
    let nombre = body.nombre.as_deref().map(str::trim);
    let descripcion = body.descripcion.as_ref().map(|o| o.as_deref());
    let imagen_url = body.imagen_url.as_ref().map(|o| o.as_deref());
    let ok = ColeccionesRepository::update(
        &state.pool,
        id,
        nombre,
        descripcion,
        body.publica,
        imagen_url,
        body.parent_id,
    )
    .await?;
    if !ok {
        return Err(AppError::NotFound(format!("coleccion {id} no encontrada")));
    }
    Ok(Json(OkResponse { ok: true }))
}

#[utoipa::path(
    delete, path = "/api/colecciones/{id}", tag = "colecciones",
    params(("id" = i64, Path, description = "ID de la coleccion")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, body = OkResponse),
        (status = 403, description = "No autorizado"),
        (status = 404, description = "No encontrada"),
    )
)]
pub async fn delete_coleccion(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<Json<OkResponse>, AppError> {
    if !ColeccionesRepository::is_owner(&state.pool, id, user.user_id).await? {
        return Err(AppError::Forbidden(
            "no eres dueño de esta coleccion".into(),
        ));
    }
    let ok = ColeccionesRepository::soft_delete(&state.pool, id).await?;
    if !ok {
        return Err(AppError::NotFound(format!("coleccion {id} no encontrada")));
    }
    Ok(Json(OkResponse { ok: true }))
}

#[utoipa::path(
    post, path = "/api/colecciones/{id}/samples", tag = "colecciones",
    params(("id" = i64, Path, description = "ID de la coleccion")),
    request_body = AddSampleRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, body = OkResponse),
        (status = 403, description = "No autorizado"),
        (status = 404, description = "Coleccion no encontrada"),
    )
)]
pub async fn add_sample(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(coleccion_id): Path<i64>,
    Json(body): Json<AddSampleRequest>,
) -> Result<Json<OkResponse>, AppError> {
    if !ColeccionesRepository::is_owner(&state.pool, coleccion_id, user.user_id).await? {
        return Err(AppError::Forbidden(
            "no eres dueño de esta coleccion".into(),
        ));
    }
    if body.sample_id <= 0 {
        return Err(AppError::BadRequest("sample_id requerido".into()));
    }
    let _op = ColeccionesRepository::add_sample(
        &state.pool,
        coleccion_id,
        body.sample_id,
        user.user_id,
    )
    .await?;
    /* [296A-1] Legacy: 1 sample = 1 coleccion. Si ya existia en otra, se mueve. */
    Ok(Json(OkResponse { ok: true }))
}

#[utoipa::path(
    delete, path = "/api/colecciones/{id}/samples/{sample_id}", tag = "colecciones",
    params(("id" = i64, Path, description = "ID de la coleccion"), ("sample_id" = i32, Path, description = "ID del sample")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, body = OkResponse),
        (status = 403, description = "No autorizado"),
    )
)]
pub async fn remove_sample(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((coleccion_id, sample_id)): Path<(i64, i32)>,
) -> Result<Json<OkResponse>, AppError> {
    if !ColeccionesRepository::is_owner(&state.pool, coleccion_id, user.user_id).await? {
        return Err(AppError::Forbidden(
            "no eres dueño de esta coleccion".into(),
        ));
    }
    let _removed =
        ColeccionesRepository::remove_sample(&state.pool, coleccion_id, sample_id).await?;
    Ok(Json(OkResponse { ok: true }))
}

#[utoipa::path(
    get, path = "/api/colecciones/{id}/samples", tag = "colecciones",
    params(("id" = i64, Path, description = "ID de la coleccion")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, body = ColeccionSamplesResponse),
        (status = 403, description = "Privada y no es del usuario"),
        (status = 404, description = "No encontrada"),
    )
)]
pub async fn list_samples(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<Json<ColeccionSamplesResponse>, AppError> {
    let col = ColeccionesRepository::fetch(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("coleccion {id} no existe")))?;
    if !col.publica && col.usuario_id != user.user_id {
        return Err(AppError::Forbidden("coleccion privada".into()));
    }
    let items = ColeccionesRepository::list_samples(&state.pool, id).await?;
    Ok(Json(ColeccionSamplesResponse { items }))
}

/* [274A-9] GET /api/colecciones/:id/combinacion-pendiente
 * El feature de combinar+undo (legacy `combinar` / `deshacer-combinacion` /
 * tabla undo_combinaciones) NO esta portado a Rust todavia. Por contrato
 * estable, retornamos siempre `hayCombinacion: false` para colecciones que
 * el usuario puede ver. Esto evita 404 en cada carga del detalle.
 *
 * Cuando se implemente combinar/deshacer, este handler debera consultar la
 * tabla de undos pendientes y poblar undoId/origenNombre/combinadoEn/expiraEn. */
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CombinacionPendienteResponse {
    #[serde(rename = "hayCombinacion")]
    pub hay_combinacion: bool,
    #[serde(rename = "undoId", skip_serializing_if = "Option::is_none")]
    pub undo_id: Option<i64>,
    #[serde(rename = "origenNombre", skip_serializing_if = "Option::is_none")]
    pub origen_nombre: Option<String>,
    #[serde(rename = "combinadoEn", skip_serializing_if = "Option::is_none")]
    pub combinado_en: Option<String>,
    #[serde(rename = "expiraEn", skip_serializing_if = "Option::is_none")]
    pub expira_en: Option<String>,
}

#[utoipa::path(
    get, path = "/api/colecciones/{id}/combinacion-pendiente", tag = "colecciones",
    params(("id" = i64, Path, description = "ID de la coleccion")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, body = CombinacionPendienteResponse),
        (status = 403, description = "Privada y no es del usuario"),
        (status = 404, description = "No encontrada"),
    )
)]
pub async fn combinacion_pendiente(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Path(id): Path<i64>,
) -> Result<Json<CombinacionPendienteResponse>, AppError> {
    let col = ColeccionesRepository::fetch(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("coleccion {id} no existe")))?;
    let viewer_id = user.as_ref().map(|u| u.user_id);
    if !col.publica && viewer_id != Some(col.usuario_id) {
        return Err(AppError::Forbidden("coleccion privada".into()));
    }
    Ok(Json(CombinacionPendienteResponse {
        hay_combinacion: false,
        undo_id: None,
        origen_nombre: None,
        combinado_en: None,
        expira_en: None,
    }))
}

/* [274A-10] GET /api/colecciones/:id/sugerencias
 * "Mas Ideas" para una coleccion: scoring agregado por tags+BPM+key de los
 * samples ya incluidos, excluyendo esos mismos. Reusa el motor de
 * SampleRepository::find_samples_by_aggregated_scoring (creado en 274A-6)
 * y el agregador de contexto del handler de sugerencias de descargas. */
#[derive(Debug, Clone, Deserialize, IntoParams, Default)]
#[into_params(parameter_in = Query)]
pub struct ColeccionSugerenciasQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ColeccionSugerenciasResponse {
    pub data: Vec<SampleSummary>,
}

const SUGERENCIAS_DEFAULT_BPM: i32 = 120;
const SUGERENCIAS_TOP_TAGS: usize = 10;
const SUGERENCIAS_MAX_PER_PAGE: i64 = 50;

#[utoipa::path(
    get, path = "/api/colecciones/{id}/sugerencias", tag = "colecciones",
    params(
        ("id" = i64, Path, description = "ID de la coleccion"),
        ColeccionSugerenciasQuery,
    ),
    responses(
        (status = 200, body = ColeccionSugerenciasResponse),
        (status = 403, description = "Privada y no es del usuario"),
        (status = 404, description = "No encontrada"),
    )
)]
pub async fn sugerencias_coleccion(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Path(id): Path<i64>,
    Query(q): Query<ColeccionSugerenciasQuery>,
) -> Result<Json<ColeccionSugerenciasResponse>, AppError> {
    let col = ColeccionesRepository::fetch(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("coleccion {id} no existe")))?;
    let viewer_id = user.as_ref().map(|u| u.user_id);
    if !col.publica && viewer_id != Some(col.usuario_id) {
        return Err(AppError::Forbidden("coleccion privada".into()));
    }

    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(20).clamp(1, SUGERENCIAS_MAX_PER_PAGE);
    let offset = (page - 1) * per_page;

    let contexto = ColeccionesRepository::contexto_muestras(&state.pool, id).await?;
    if contexto.is_empty() {
        return Ok(Json(ColeccionSugerenciasResponse { data: Vec::new() }));
    }

    let exclude = ColeccionesRepository::ids_muestras(&state.pool, id).await?;
    let (top_tags, avg_bpm, dominant_key) = aggregate_coleccion_context(&contexto);

    let criterio = CriterioScoring {
        top_tags: &top_tags,
        avg_bpm,
        dominant_key: dominant_key.as_deref(),
        exclude_ids: &exclude,
    };
    let records = SampleRepository::find_samples_by_aggregated_scoring(
        &state.pool,
        criterio,
        per_page,
        offset,
        viewer_id,
    )
    .await?;

    let public_base = state.public_base_url.as_deref();
    let data: Vec<SampleSummary> = records
        .into_iter()
        .map(|r| build_sample_summary(r, public_base))
        .collect();

    Ok(Json(ColeccionSugerenciasResponse { data }))
}

fn aggregate_coleccion_context(
    contexto: &[SampleContextRow],
) -> (Vec<String>, i32, Option<String>) {
    use std::collections::HashMap;
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

    let mut tag_vec: Vec<(&str, usize)> = tag_counts.into_iter().collect();
    tag_vec.sort_by_key(|a| std::cmp::Reverse(a.1));
    let top_tags: Vec<String> = tag_vec
        .into_iter()
        .take(SUGERENCIAS_TOP_TAGS)
        .map(|(t, _)| t.to_string())
        .collect();

    let avg_bpm = if bpm_n > 0 {
        i32::try_from(bpm_sum / bpm_n).unwrap_or(SUGERENCIAS_DEFAULT_BPM)
    } else {
        SUGERENCIAS_DEFAULT_BPM
    };

    let dominant_key = key_counts
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(k, _)| k.to_string());

    (top_tags, avg_bpm, dominant_key)
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/colecciones",
            post(create_coleccion).get(legacy::list_my_colecciones_legacy),
        )
        .route(
            "/colecciones/explorar",
            get(legacy::explore_colecciones_legacy),
        )
        .route(
            "/colecciones/por-slug/:slug",
            get(legacy::get_coleccion_by_slug_legacy),
        )
        .route(
            "/colecciones/relevantes/:sample_id",
            get(legacy::list_relevant_for_sample_legacy),
        )
        .route(
            "/colecciones/:id",
            get(legacy::get_coleccion_legacy)
                .put(update_coleccion)
                .delete(delete_coleccion),
        )
        .route(
            "/colecciones/:id/samples",
            post(add_sample).get(list_samples),
        )
        .route("/colecciones/:id/samples/:sample_id", delete(remove_sample))
        .route(
            "/colecciones/:id/combinacion-pendiente",
            get(combinacion_pendiente),
        )
        .route("/colecciones/:id/sugerencias", get(sugerencias_coleccion))
        .route("/colecciones/:id/merge", post(merge_coleccion))
        .route(
            "/colecciones/:id/save",
            post(save_coleccion).delete(unsave_coleccion),
        )
        .route(
            "/colecciones/:id/guardar",
            post(save_coleccion).delete(unsave_coleccion),
        )
        .route(
            "/colecciones/guardadas",
            get(legacy::list_saved_colecciones_legacy),
        )
        .route(
            "/me/colecciones-guardadas",
            get(legacy::list_saved_colecciones_legacy),
        )
        .route(
            "/colecciones/:id/descargar-zip",
            post(descargar_zip_coleccion),
        )
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MergeColeccionRequest {
    pub source_id: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MergeColeccionResponse {
    pub ok: bool,
    pub moved: i64,
}

/* [174A-65] Merge: combina source en target, soft-deletea source.
 * Ambas deben pertenecer al usuario actual. */
#[utoipa::path(
    post, path = "/api/colecciones/{id}/merge", tag = "colecciones",
    params(("id" = i64, Path, description = "ID de la coleccion target")),
    request_body = MergeColeccionRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, body = MergeColeccionResponse),
        (status = 400, description = "source == target"),
        (status = 403, description = "No autorizado en alguna de las dos colecciones"),
        (status = 404, description = "Alguna coleccion no existe"),
    )
)]
pub async fn merge_coleccion(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(target_id): Path<i64>,
    Json(body): Json<MergeColeccionRequest>,
) -> Result<Json<MergeColeccionResponse>, AppError> {
    if !ColeccionesRepository::is_owner(&state.pool, target_id, user.user_id).await? {
        return Err(AppError::Forbidden(
            "no eres dueño de la coleccion target".into(),
        ));
    }
    if !ColeccionesRepository::is_owner(&state.pool, body.source_id, user.user_id).await? {
        return Err(AppError::Forbidden(
            "no eres dueño de la coleccion source".into(),
        ));
    }
    let moved = ColeccionesRepository::merge(&state.pool, target_id, body.source_id).await?;
    Ok(Json(MergeColeccionResponse { ok: true, moved }))
}

/* [174A-66] Saved collections (bookmarks). */

#[derive(Debug, Serialize, ToSchema)]
pub struct SavedListResponse {
    pub items: Vec<SavedColeccion>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SavedListQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}
#[allow(dead_code)]
fn default_limit() -> i64 {
    30
}

#[utoipa::path(
    post, path = "/api/colecciones/{id}/save", tag = "colecciones",
    params(("id" = i64, Path, description = "ID de la coleccion a guardar")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, body = OkResponse),
        (status = 403, description = "No se puede guardar una coleccion privada ajena"),
        (status = 404, description = "Coleccion no encontrada"),
    )
)]
pub async fn save_coleccion(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<Json<OkResponse>, AppError> {
    let col = ColeccionesRepository::fetch(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("coleccion {id} no existe")))?;
    /* Solo se pueden guardar colecciones públicas o propias. */
    if !col.publica && col.usuario_id != user.user_id {
        return Err(AppError::Forbidden("coleccion privada".into()));
    }
    SavedCollectionsRepository::save(&state.pool, user.user_id, id).await?;
    Ok(Json(OkResponse { ok: true }))
}

#[utoipa::path(
    delete, path = "/api/colecciones/{id}/save", tag = "colecciones",
    params(("id" = i64, Path, description = "ID de la coleccion a quitar de guardadas")),
    security(("bearer_auth" = [])),
    responses((status = 200, body = OkResponse))
)]
pub async fn unsave_coleccion(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> Result<Json<OkResponse>, AppError> {
    SavedCollectionsRepository::unsave(&state.pool, user.user_id, id).await?;
    Ok(Json(OkResponse { ok: true }))
}

#[allow(dead_code)]
#[utoipa::path(
    get, path = "/api/me/colecciones-guardadas", tag = "colecciones",
    params(
        ("limit" = Option<i64>, Query, description = "Tamaño de página (default 30, max 100)"),
        ("offset" = Option<i64>, Query, description = "Offset")
    ),
    security(("bearer_auth" = [])),
    responses((status = 200, body = SavedListResponse))
)]
pub async fn list_saved_colecciones(
    State(state): State<AppState>,
    user: CurrentUser,
    axum::extract::Query(q): axum::extract::Query<SavedListQuery>,
) -> Result<Json<SavedListResponse>, AppError> {
    let limit = q.limit.clamp(1, 100);
    let offset = q.offset.max(0);
    let items =
        SavedCollectionsRepository::list_by_user(&state.pool, user.user_id, limit, offset).await?;
    Ok(Json(SavedListResponse { items }))
}
