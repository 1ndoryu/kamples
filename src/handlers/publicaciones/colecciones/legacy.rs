use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::errors::AppError;
use crate::middleware::{CurrentUser, OptionalUser};
use crate::repositories::{
    ColeccionesRepository, LegacyColeccionParentRecord, LegacyColeccionRecord,
    LegacyColeccionSampleRecord,
};
use crate::AppState;

const DEFAULT_PAGE: i64 = 1;
const DEFAULT_PER_PAGE: i64 = 20;
const DEFAULT_SAVED_PER_PAGE: i64 = 30;
const MAX_PER_PAGE: i64 = 100;

#[derive(Debug, Deserialize, ToSchema, Default)]
pub struct LegacyColeccionesQuery {
    pub busqueda: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema, Default)]
pub struct ExploreColeccionesQuery {
    pub busqueda: Option<String>,
    pub page: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema, Default)]
pub struct SavedColeccionesQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema, Default)]
pub struct ColeccionDetailQuery {
    #[serde(default, alias = "incluirSubcolecciones")]
    pub incluir_subcolecciones: Option<String>,
    pub orden: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[allow(clippy::struct_excessive_bools)]
pub struct LegacyColeccionResponse {
    pub id: i64,
    pub usuario_id: i32,
    pub nombre: String,
    pub slug: Option<String>,
    pub descripcion: String,
    pub publica: bool,
    pub parent_id: Option<i64>,
    pub imagen_url: Option<String>,
    pub total_samples: i32,
    pub total_items: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub tags: Vec<String>,
    pub username: Option<String>,
    pub nombre_visible: Option<String>,
    pub avatar_url: Option<String>,
    pub verificado: bool,
    pub esta_guardada: bool,
    pub esta_likeada: bool,
    pub total_likes: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contiene_el_sample: Option<bool>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LegacyColeccionParentResponse {
    pub id: i64,
    pub nombre: String,
    pub slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LegacySampleCreatorResponse {
    pub id: i32,
    pub username: String,
    pub nombre_visible: Option<String>,
    pub avatar_url: Option<String>,
    pub verificado: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LegacyColeccionSampleResponse {
    pub id: i32,
    pub id_corto: Option<String>,
    pub slug: String,
    pub titulo: String,
    pub descripcion: String,
    pub bpm: Option<i32>,
    #[serde(rename = "key")]
    pub music_key: Option<String>,
    pub escala: Option<String>,
    pub duracion: f32,
    pub formato: String,
    pub metadata: serde_json::Value,
    pub tags: Vec<String>,
    pub tipo: String,
    pub es_premium: bool,
    pub precio: Option<f64>,
    pub verificado: bool,
    pub ruta_preview: Option<String>,
    pub ruta_waveform: Option<String>,
    pub imagen_url: Option<String>,
    pub total_descargas: i32,
    pub total_likes: i32,
    pub total_reproducciones: i32,
    pub total_comentarios: i32,
    pub creador: LegacySampleCreatorResponse,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LegacyColeccionDetailResponse {
    #[serde(flatten)]
    pub coleccion: LegacyColeccionResponse,
    pub samples: Vec<LegacyColeccionSampleResponse>,
    pub subcolecciones: Vec<LegacyColeccionResponse>,
    pub coleccion_padre: Option<LegacyColeccionParentResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LegacyColeccionesPayload {
    pub colecciones: Vec<LegacyColeccionResponse>,
    pub tags_frecuentes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LegacySavedColeccionesPayload {
    pub colecciones: Vec<LegacyColeccionResponse>,
    pub total: i64,
    pub page: i64,
}

#[utoipa::path(
    get,
    path = "/api/colecciones",
    params(("busqueda" = Option<String>, Query, description = "Filtra mis colecciones por nombre o descripción")),
    security(("bearer_auth" = [])),
    responses((status = 200, body = LegacyColeccionesPayload))
)]
pub async fn list_my_colecciones_legacy(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<LegacyColeccionesQuery>,
) -> Result<Json<LegacyColeccionesPayload>, AppError> {
    let rows = ColeccionesRepository::list_user_legacy(
        &state.pool,
        user.user_id,
        query.busqueda.as_deref(),
    )
    .await?;
    let tags_frecuentes = if query
        .busqueda
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        Vec::new()
    } else {
        ColeccionesRepository::tags_frecuentes_user_legacy(&state.pool, user.user_id, 15).await?
    };

    Ok(Json(LegacyColeccionesPayload {
        colecciones: rows
            .into_iter()
            .map(|row| map_collection(row, state.public_base_url.as_deref()))
            .collect(),
        tags_frecuentes,
    }))
}

#[utoipa::path(
    get,
    path = "/api/colecciones/explorar",
    params(
        ("busqueda" = Option<String>, Query, description = "Búsqueda textual"),
        ("page" = Option<i64>, Query, description = "Página 1-based. Default 1")
    ),
    responses((status = 200, body = LegacyColeccionesPayload))
)]
pub async fn explore_colecciones_legacy(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Query(query): Query<ExploreColeccionesQuery>,
) -> Result<Json<LegacyColeccionesPayload>, AppError> {
    let page = query.page.unwrap_or(DEFAULT_PAGE).max(1);
    let offset = (page - 1) * DEFAULT_PER_PAGE;
    let viewer_id = user.as_ref().map(|current| current.user_id);
    let rows = ColeccionesRepository::list_explore_legacy(
        &state.pool,
        viewer_id,
        query.busqueda.as_deref(),
        DEFAULT_PER_PAGE,
        offset,
    )
    .await?;
    let tags_frecuentes = if query
        .busqueda
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
    {
        Vec::new()
    } else {
        ColeccionesRepository::tags_frecuentes_explorar_legacy(&state.pool, 15).await?
    };

    Ok(Json(LegacyColeccionesPayload {
        colecciones: rows
            .into_iter()
            .map(|row| map_collection(row, state.public_base_url.as_deref()))
            .collect(),
        tags_frecuentes,
    }))
}

#[utoipa::path(
    get,
    path = "/api/colecciones/guardadas",
    params(
        ("page" = Option<i64>, Query, description = "Página 1-based. Default 1"),
        ("per_page" = Option<i64>, Query, description = "Tamaño de página. Default 30")
    ),
    security(("bearer_auth" = [])),
    responses((status = 200, body = LegacySavedColeccionesPayload))
)]
pub async fn list_saved_colecciones_legacy(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<SavedColeccionesQuery>,
) -> Result<Json<LegacySavedColeccionesPayload>, AppError> {
    let page = query.page.unwrap_or(DEFAULT_PAGE).max(1);
    let per_page = query
        .per_page
        .unwrap_or(DEFAULT_SAVED_PER_PAGE)
        .clamp(1, MAX_PER_PAGE);
    let offset = (page - 1) * per_page;
    let rows =
        ColeccionesRepository::list_saved_legacy(&state.pool, user.user_id, per_page, offset)
            .await?;
    let total = ColeccionesRepository::count_saved_legacy(&state.pool, user.user_id).await?;

    Ok(Json(LegacySavedColeccionesPayload {
        colecciones: rows
            .into_iter()
            .map(|row| map_collection(row, state.public_base_url.as_deref()))
            .collect(),
        total,
        page,
    }))
}

#[utoipa::path(
    get,
    path = "/api/colecciones/{id}",
    params(
        ("id" = i64, Path, description = "ID de la colección"),
        ("incluirSubcolecciones" = Option<bool>, Query, description = "Incluye samples de subcolecciones cuando la colección es raíz"),
        ("orden" = Option<String>, Query, description = "Orden legacy; por ahora se ignora y se respeta el orden de la colección")
    ),
    responses(
        (status = 200, body = LegacyColeccionDetailResponse),
        (status = 404, description = "Colección no encontrada")
    )
)]
pub async fn get_coleccion_legacy(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Path(id): Path<i64>,
    Query(query): Query<ColeccionDetailQuery>,
) -> Result<Json<LegacyColeccionDetailResponse>, AppError> {
    let viewer_id = user.as_ref().map(|current| current.user_id);
    let row = ColeccionesRepository::fetch_legacy_by_id(&state.pool, id, viewer_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("coleccion {id} no encontrada")))?;

    build_collection_detail(&state, row, viewer_id, query).await
}

#[utoipa::path(
    get,
    path = "/api/colecciones/por-slug/{slug}",
    params(
        ("slug" = String, Path, description = "Slug público de la colección"),
        ("incluirSubcolecciones" = Option<bool>, Query, description = "Incluye samples de subcolecciones cuando la colección es raíz"),
        ("orden" = Option<String>, Query, description = "Orden legacy; por ahora se ignora y se respeta el orden de la colección")
    ),
    responses(
        (status = 200, body = LegacyColeccionDetailResponse),
        (status = 404, description = "Colección no encontrada")
    )
)]
pub async fn get_coleccion_by_slug_legacy(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Path(slug): Path<String>,
    Query(query): Query<ColeccionDetailQuery>,
) -> Result<Json<LegacyColeccionDetailResponse>, AppError> {
    let viewer_id = user.as_ref().map(|current| current.user_id);
    let row = ColeccionesRepository::fetch_legacy_by_slug(&state.pool, &slug, viewer_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("coleccion {slug} no encontrada")))?;

    build_collection_detail(&state, row, viewer_id, query).await
}

#[utoipa::path(
    get,
    path = "/api/colecciones/relevantes/{sample_id}",
    params(("sample_id" = i32, Path, description = "ID del sample")),
    security(("bearer_auth" = [])),
    responses((status = 200, body = [LegacyColeccionResponse]))
)]
pub async fn list_relevant_for_sample_legacy(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(sample_id): Path<i32>,
) -> Result<Json<Vec<LegacyColeccionResponse>>, AppError> {
    if sample_id <= 0 {
        return Err(AppError::BadRequest("sample_id invalido".into()));
    }
    let rows = ColeccionesRepository::list_relevant_for_sample_legacy(
        &state.pool,
        user.user_id,
        sample_id,
        12,
    )
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| map_collection(row, state.public_base_url.as_deref()))
            .collect(),
    ))
}

async fn build_collection_detail(
    state: &AppState,
    row: LegacyColeccionRecord,
    viewer_id: Option<i32>,
    query: ColeccionDetailQuery,
) -> Result<Json<LegacyColeccionDetailResponse>, AppError> {
    let _orden = query.orden.as_deref();
    if !row.publica && viewer_id != Some(row.usuario_id) {
        return Err(AppError::NotFound(format!(
            "coleccion {} no encontrada",
            row.id
        )));
    }

    let include_subcollections =
        legacy_bool(query.incluir_subcolecciones.as_deref()) && row.parent_id.is_none();
    let mut coleccion_ids = vec![row.id];
    if include_subcollections {
        coleccion_ids.extend(
            ColeccionesRepository::list_subcollection_ids_legacy(&state.pool, row.id).await?,
        );
    }

    let samples = ColeccionesRepository::list_detail_samples_legacy(&state.pool, &coleccion_ids)
        .await?
        .into_iter()
        .map(|record| map_sample(record, state.public_base_url.as_deref()))
        .collect::<Vec<_>>();

    let subcolecciones = if row.parent_id.is_none() {
        ColeccionesRepository::list_subcollections_legacy(&state.pool, row.id, viewer_id)
            .await?
            .into_iter()
            .map(|record| map_collection(record, state.public_base_url.as_deref()))
            .collect()
    } else {
        Vec::new()
    };

    let coleccion_padre = match row.parent_id {
        Some(parent_id) => ColeccionesRepository::fetch_parent_legacy(&state.pool, parent_id)
            .await?
            .map(map_parent),
        None => None,
    };

    let total_samples = i32::try_from(samples.len()).unwrap_or(i32::MAX);
    let mut coleccion = map_collection(row, state.public_base_url.as_deref());
    coleccion.total_samples = total_samples;
    coleccion.total_items = i64::from(total_samples);

    Ok(Json(LegacyColeccionDetailResponse {
        coleccion,
        samples,
        subcolecciones,
        coleccion_padre,
    }))
}

fn map_collection(
    row: LegacyColeccionRecord,
    public_base_url: Option<&str>,
) -> LegacyColeccionResponse {
    let generated_slug = Some(generate_collection_slug(&row.nombre, row.id));
    LegacyColeccionResponse {
        id: row.id,
        usuario_id: row.usuario_id,
        nombre: row.nombre,
        slug: row.slug.or(generated_slug),
        descripcion: row.descripcion,
        publica: row.publica,
        parent_id: row.parent_id,
        imagen_url: asset_to_public_url(public_base_url, row.imagen_url),
        total_samples: row.total_samples,
        total_items: row.total_items,
        created_at: row.created_at,
        updated_at: row.updated_at,
        tags: row.tags,
        username: row.username,
        nombre_visible: row.nombre_visible,
        avatar_url: asset_to_public_url(public_base_url, row.avatar_url),
        verificado: row.verificado,
        esta_guardada: row.esta_guardada,
        esta_likeada: row.esta_likeada,
        total_likes: row.total_likes,
        contiene_el_sample: row.contiene_el_sample,
    }
}

fn legacy_bool(value: Option<&str>) -> bool {
    matches!(
        value.map(str::trim).map(str::to_ascii_lowercase).as_deref(),
        Some("1" | "true" | "yes" | "si" | "on")
    )
}

fn map_parent(row: LegacyColeccionParentRecord) -> LegacyColeccionParentResponse {
    let generated_slug = generate_collection_slug(&row.nombre, row.id);
    LegacyColeccionParentResponse {
        id: row.id,
        nombre: row.nombre,
        slug: row.slug.or(Some(generated_slug)),
    }
}

fn map_sample(
    row: LegacyColeccionSampleRecord,
    public_base_url: Option<&str>,
) -> LegacyColeccionSampleResponse {
    LegacyColeccionSampleResponse {
        id: row.id,
        id_corto: row.id_corto,
        slug: row.slug,
        titulo: row.titulo,
        descripcion: row.descripcion,
        bpm: row.bpm,
        music_key: row.music_key,
        escala: row.escala,
        duracion: row.duracion,
        formato: row.formato,
        metadata: row.metadata,
        tags: row.tags,
        tipo: row.tipo,
        es_premium: row.es_premium,
        precio: row.precio,
        verificado: row.verificado,
        ruta_preview: asset_to_public_url(public_base_url, row.ruta_preview),
        ruta_waveform: asset_to_public_url(public_base_url, row.ruta_waveform),
        imagen_url: asset_to_public_url(public_base_url, row.imagen_url),
        total_descargas: row.total_descargas,
        total_likes: row.total_likes,
        total_reproducciones: row.total_reproducciones,
        total_comentarios: row.total_comentarios,
        creador: LegacySampleCreatorResponse {
            id: row.creator_id,
            username: row.creator_username,
            nombre_visible: row.creator_nombre_visible,
            avatar_url: asset_to_public_url(public_base_url, row.creator_avatar_url),
            verificado: row.creator_verificado,
        },
    }
}

fn asset_to_public_url(public_base_url: Option<&str>, raw: Option<String>) -> Option<String> {
    let raw = raw?.trim().replace('\\', "/");
    if raw.is_empty() {
        return None;
    }

    if raw.starts_with("http://") || raw.starts_with("https://") {
        return Some(raw);
    }

    let path = if raw.starts_with('/') {
        raw
    } else {
        format!("/uploads/{raw}")
    };

    Some(match public_base_url {
        Some(base) => format!("{}{}", base.trim_end_matches('/'), path),
        None => path,
    })
}

fn generate_collection_slug(nombre: &str, id: i64) -> String {
    let mut base = slug::slugify(nombre);
    if base.is_empty() {
        base = "coleccion".to_string();
    }
    format!("{base}-{id}")
}
