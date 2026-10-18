use validator::Validate;

use crate::errors::AppError;
use crate::models::{
    CancionOrigenResumen, DeleteSampleResponse, ExtraccionSampleResponse, ListSamplesQuery,
    ListSamplesResponse, SampleCreatorSummary, SampleDetailResponse, SampleSummary,
    SamplesPagination, SimilarSamplesQuery, SimilarSamplesResponse, UpdateSampleRequest,
};
use crate::repositories::{
    OwnedSampleRecord, ReportRepository, SampleCatalogDetailRecord, SampleCatalogSummaryRecord,
    SampleListFilters, SampleRepository, SampleSortOrder, SampleTextSearch, UpdateSamplePatch,
    AUTO_HIDE_SAMPLE_REPORT_THRESHOLD,
};

#[cfg(test)]
mod tests;

mod ai_correction;

pub use ai_correction::{correct_sample_metadata, CorrectionOutcome};

const DEFAULT_PAGE: i64 = 1;
const DEFAULT_PER_PAGE: i64 = 20;
const MAX_TAG_FILTERS: usize = 10;
const MAX_CREATOR_LENGTH: usize = 50;
const MIN_SAMPLE_TAGS: usize = 2;
const MAX_SAMPLE_TAGS: usize = 20;
const MAX_SEARCH_LENGTH: usize = 120;
const DEFAULT_SIMILAR_LIMIT: i64 = 5;

/* [174A-44] Servicio de catálogo público de samples.
 * Centraliza la normalización de filtros para que el handler quede fino y el
 * repositorio solo reciba parámetros ya validados. Así evitamos duplicar reglas
 * de alias (`type`/`tipo`, `creator`/`creador`) en cada capa. */

pub struct SampleCatalogService;

impl SampleCatalogService {
    pub async fn list_public_samples(
        pool: &sqlx::PgPool,
        public_base_url: Option<&str>,
        current_user_id: Option<i32>,
        query: ListSamplesQuery,
    ) -> Result<ListSamplesResponse, AppError> {
        query
            .validate()
            .map_err(|error| AppError::Validation(error.to_string()))?;

        let filters = normalize_filters(query, current_user_id)?;
        let result = SampleRepository::list_public_samples(pool, &filters).await?;
        let pages = if result.total == 0 {
            0
        } else {
            (result.total + filters.per_page - 1) / filters.per_page
        };

        Ok(ListSamplesResponse {
            data: result
                .items
                .into_iter()
                .map(|record| build_sample_summary(record, public_base_url))
                .collect(),
            pagination: SamplesPagination {
                page: filters.page,
                per_page: filters.per_page,
                total: result.total,
                pages,
            },
        })
    }

    pub async fn list_public_samples_by_ids(
        pool: &sqlx::PgPool,
        public_base_url: Option<&str>,
        current_user_id: Option<i32>,
        sample_ids: &[i32],
    ) -> Result<Vec<SampleSummary>, AppError> {
        let records = SampleRepository::find_public_samples_by_ids_in_order(
            pool,
            sample_ids,
            current_user_id,
        )
        .await?;

        Ok(records
            .into_iter()
            .map(|record| build_sample_summary(record, public_base_url))
            .collect())
    }

    pub async fn get_sample_detail(
        pool: &sqlx::PgPool,
        public_base_url: Option<&str>,
        current_user_id: Option<i32>,
        slug_or_short_id: &str,
    ) -> Result<SampleDetailResponse, AppError> {
        let sample = SampleRepository::find_sample_by_slug_or_short_id(pool, slug_or_short_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("sample {slug_or_short_id}")))?;

        if ReportRepository::is_auto_hidden_for_viewer(
            pool,
            "sample",
            sample.id,
            sample.creator_id,
            current_user_id,
            AUTO_HIDE_SAMPLE_REPORT_THRESHOLD,
        )
        .await?
        {
            return Err(AppError::NotFound(format!("sample {slug_or_short_id}")));
        }

        Ok(build_sample_detail(
            sample,
            public_base_url,
            current_user_id,
        ))
    }

    pub async fn get_random_sample(
        pool: &sqlx::PgPool,
        public_base_url: Option<&str>,
        current_user_id: Option<i32>,
    ) -> Result<SampleDetailResponse, AppError> {
        for _ in 0..3 {
            let Some(sample) = SampleRepository::find_random_public_sample(pool).await? else {
                break;
            };
            if ReportRepository::is_auto_hidden_for_viewer(
                pool,
                "sample",
                sample.id,
                sample.creator_id,
                current_user_id,
                AUTO_HIDE_SAMPLE_REPORT_THRESHOLD,
            )
            .await?
            {
                continue;
            }

            return Ok(build_sample_detail(
                sample,
                public_base_url,
                current_user_id,
            ));
        }

        Err(AppError::NotFound("sample aleatorio".into()))
    }

    pub async fn get_similar_samples(
        pool: &sqlx::PgPool,
        public_base_url: Option<&str>,
        current_user_id: Option<i32>,
        sample_id: i32,
        query: SimilarSamplesQuery,
    ) -> Result<SimilarSamplesResponse, AppError> {
        query
            .validate()
            .map_err(|error| AppError::Validation(error.to_string()))?;

        let limit = normalize_similar_limit(&query);
        let records =
            SampleRepository::find_public_similar_samples(pool, sample_id, limit, current_user_id)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("sample {sample_id}")))?;

        Ok(SimilarSamplesResponse {
            data: records
                .into_iter()
                .map(|record| build_sample_summary(record, public_base_url))
                .collect(),
        })
    }

    pub async fn update_owned_sample(
        pool: &sqlx::PgPool,
        public_base_url: Option<&str>,
        current_user_id: i32,
        slug_or_short_id: &str,
        request: UpdateSampleRequest,
    ) -> Result<SampleDetailResponse, AppError> {
        request
            .validate()
            .map_err(|error| AppError::Validation(error.to_string()))?;

        let sample = load_owned_sample(pool, slug_or_short_id, current_user_id).await?;
        let patch = normalize_update_request(request)?;

        SampleRepository::update_sample_metadata(pool, sample.id, &patch).await?;

        let lookup_key = sample.id_corto.as_deref().unwrap_or(sample.slug.as_str());
        let updated = SampleRepository::find_sample_by_slug_or_short_id(pool, lookup_key)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("sample {slug_or_short_id}")))?;

        Ok(build_sample_detail(
            updated,
            public_base_url,
            Some(current_user_id),
        ))
    }

    pub async fn delete_owned_sample(
        pool: &sqlx::PgPool,
        current_user_id: i32,
        slug_or_short_id: &str,
    ) -> Result<DeleteSampleResponse, AppError> {
        let sample = load_owned_sample(pool, slug_or_short_id, current_user_id).await?;
        let deleted =
            SampleRepository::soft_delete_owned_sample(pool, sample.id, current_user_id).await?;

        if !deleted {
            return Err(AppError::NotFound(format!("sample {slug_or_short_id}")));
        }

        Ok(DeleteSampleResponse {
            ok: true,
            eliminado: true,
        })
    }
}

async fn load_owned_sample(
    pool: &sqlx::PgPool,
    slug_or_short_id: &str,
    current_user_id: i32,
) -> Result<OwnedSampleRecord, AppError> {
    let sample = SampleRepository::find_owned_sample_by_slug_or_short_id(pool, slug_or_short_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("sample {slug_or_short_id}")))?;

    if sample.creator_id != current_user_id {
        return Err(AppError::Forbidden(
            "No tienes permiso para modificar este sample".into(),
        ));
    }

    Ok(sample)
}

pub(crate) fn build_sample_summary(
    record: SampleCatalogSummaryRecord,
    public_base_url: Option<&str>,
) -> SampleSummary {
    SampleSummary {
        id: record.id,
        id_corto: record.id_corto,
        slug: record.slug,
        titulo: record.titulo,
        descripcion: record.descripcion,
        bpm: record.bpm,
        music_key: record.music_key,
        escala: record.escala,
        duracion: record.duracion,
        formato: record.formato,
        tags: record.tags,
        tipo: record.tipo,
        es_premium: record.es_premium,
        precio: record.precio,
        verificado: record.verificado,
        ruta_preview: asset_to_public_url(public_base_url, record.ruta_preview),
        ruta_waveform: asset_to_public_url(public_base_url, record.ruta_waveform),
        imagen_url: asset_to_public_url(public_base_url, record.imagen_url),
        total_descargas: record.total_descargas,
        total_likes: record.total_likes,
        total_reproducciones: record.total_reproducciones,
        total_comentarios: record.total_comentarios,
        publicado_at: record.publicado_at,
        metadata: record.metadata,
        ya_guardado_en_coleccion: record.ya_guardado_en_coleccion,
        liked: record.liked,
        reaccion: record.reaccion,
        creador: build_creator_summary(
            record.creator_id,
            record.creator_username,
            record.creator_nombre_visible,
            record.creator_avatar_url,
            record.creator_verificado,
            public_base_url,
        ),
    }
}

/* Campos del JSONB metadata_extraccion, tipados; ausentes o de otro tipo quedan en None. */
#[derive(Default)]
struct MetaExtraccion {
    fuente_titulo: Option<String>,
    fuente_artista: Option<String>,
    dest_titulo: Option<String>,
    dest_artista: Option<String>,
    tipo_elemento: Option<String>,
    recorte_por_compas: Option<String>,
    formato: Option<String>,
    descarga_metodo: Option<String>,
    descarga_fuente_url: Option<String>,
    descarga_fuente_titulo: Option<String>,
    descarga_fuente_artista: Option<String>,
    votos_total: Option<i32>,
    duracion_extraida: Option<f64>,
    tamano_bytes: Option<i32>,
}

impl MetaExtraccion {
    fn desde_json(valor: Option<&serde_json::Value>) -> Self {
        let Some(obj) = valor.and_then(serde_json::Value::as_object) else {
            return Self::default();
        };
        let texto = |k: &str| obj.get(k).and_then(|v| v.as_str()).map(str::to_owned);
        // Enteros fuera de rango i32 se descartan en lugar de truncarse.
        let entero = |k: &str| {
            obj.get(k)
                .and_then(serde_json::Value::as_i64)
                .and_then(|n| i32::try_from(n).ok())
        };
        Self {
            fuente_titulo: texto("fuente_titulo"),
            fuente_artista: texto("fuente_artista"),
            dest_titulo: texto("destino_titulo"),
            dest_artista: texto("destino_artista"),
            tipo_elemento: texto("tipo_elemento"),
            recorte_por_compas: texto("recorte_por_compas"),
            formato: texto("formato"),
            descarga_metodo: texto("descarga_metodo"),
            descarga_fuente_url: texto("descarga_fuente_url"),
            descarga_fuente_titulo: texto("descarga_fuente_titulo"),
            descarga_fuente_artista: texto("descarga_fuente_artista"),
            votos_total: entero("votos_total"),
            duracion_extraida: obj.get("duracion").and_then(serde_json::Value::as_f64),
            tamano_bytes: entero("tamano_bytes"),
        }
    }
}

/* [166A-1] QQ51: Construir cancion_origen enriquecido si hay datos */
fn build_cancion_origen(record: &SampleCatalogDetailRecord) -> Option<CancionOrigenResumen> {
    record.cancion_origen_id.and_then(|_| {
        let titulo = record.cancion_origen_titulo.as_deref()?;
        let slug = record.cancion_origen_slug.as_deref()?;
        Some(CancionOrigenResumen {
            id: record.cancion_origen_id?,
            titulo: titulo.to_string(),
            slug: slug.to_string(),
            artista: record.cancion_origen_artista.clone(),
            whosampled_url: record.cancion_origen_whosampled_url.clone(),
            bpm: record.cancion_origen_bpm,
        })
    })
}

/* [166A-1] QQ117: Construir extraccion si existe al menos una fila */
fn build_extraccion(record: &SampleCatalogDetailRecord) -> Option<ExtraccionSampleResponse> {
    let has_extraccion = record.extraccion_youtube_id.is_some()
        || record.extraccion_bpm_detectado.is_some()
        || record.extraccion_estado.is_some();
    if !has_extraccion {
        return None;
    }
    let meta = MetaExtraccion::desde_json(record.extraccion_metadata.as_ref());
    Some(ExtraccionSampleResponse {
        youtube_id: record.extraccion_youtube_id.clone(),
        spotify_id: record.extraccion_spotify_id.clone(),
        timing_inicio_seg: record.extraccion_timing_inicio_seg,
        bpm_detectado: record.extraccion_bpm_detectado,
        duracion_compas_seg: record.extraccion_duracion_compas_seg,
        compas_inicio_seg: record.extraccion_compas_inicio_seg,
        compas_fin_seg: record.extraccion_compas_fin_seg,
        lado: record.extraccion_lado.clone(),
        estado: record.extraccion_estado.clone(),
        ruta_audio_extraido: record.extraccion_ruta_audio.clone(),
        tiene_audio_completo: record.extraccion_ruta_audio_completo.is_some(),
        fuente_url: meta.descarga_fuente_url,
        fuente_titulo: meta.descarga_fuente_titulo,
        fuente_artista: meta.descarga_fuente_artista,
        descarga_metodo: meta.descarga_metodo,
        sampleo_fuente_titulo: meta.fuente_titulo,
        sampleo_fuente_artista: meta.fuente_artista,
        sampleo_destino_titulo: meta.dest_titulo,
        sampleo_destino_artista: meta.dest_artista,
        fuente_slug: record.extraccion_fuente_slug.clone(),
        fuente_album: record.extraccion_fuente_album.clone(),
        destino_slug: record.extraccion_destino_slug.clone(),
        destino_album: record.extraccion_destino_album.clone(),
        votos_total: meta.votos_total,
        tipo_elemento: meta.tipo_elemento,
        recorte_por_compas: meta.recorte_por_compas,
        duracion_extraida: meta.duracion_extraida,
        formato_extraido: meta.formato,
        tamano_bytes: meta.tamano_bytes.map(i64::from),
    })
}

fn build_sample_detail(
    record: SampleCatalogDetailRecord,
    public_base_url: Option<&str>,
    current_user_id: Option<i32>,
) -> SampleDetailResponse {
    let is_owner = current_user_id.is_some_and(|user_id| user_id == record.creator_id);
    let cancion_origen = build_cancion_origen(&record);
    let extraccion = build_extraccion(&record);

    /* [166A-1] Construir URL de WhoSampled desde whosampled_id */
    let whosampled_url = record
        .relacion_sampleo_whosampled_id
        .map(|id| format!("https://www.whosampled.com/sample/{id}/"));

    SampleDetailResponse {
        id: record.id,
        id_corto: record.id_corto,
        slug: record.slug,
        titulo: record.titulo,
        descripcion: record.descripcion,
        bpm: record.bpm,
        music_key: record.music_key,
        escala: record.escala,
        duracion: record.duracion,
        formato: record.formato,
        tamano: record.tamano,
        tags: record.tags,
        tipo: record.tipo,
        estado: record.estado,
        es_premium: record.es_premium,
        precio: record.precio,
        metadata: record.metadata,
        ruta_preview: asset_to_public_url(public_base_url, record.ruta_preview),
        ruta_waveform: asset_to_public_url(public_base_url, record.ruta_waveform),
        ruta_original: if is_owner {
            asset_to_public_url(public_base_url, record.ruta_original)
        } else {
            None
        },
        ruta_optimizada: if is_owner {
            asset_to_public_url(public_base_url, record.ruta_optimizada)
        } else {
            None
        },
        permitir_descarga: record.permitir_descarga,
        licencia_libre: record.licencia_libre,
        imagen_url: asset_to_public_url(public_base_url, record.imagen_url),
        total_descargas: record.total_descargas,
        total_likes: record.total_likes,
        total_reproducciones: record.total_reproducciones,
        total_comentarios: record.total_comentarios,
        audio_hash: record.audio_hash,
        verificado: record.verificado,
        mostrar_en_comunidad: record.mostrar_en_comunidad,
        publicado_at: record.publicado_at,
        created_at: record.created_at,
        cancion_origen_id: record.cancion_origen_id,
        relacion_sampleo_id: record.relacion_sampleo_id,
        cancion_origen,
        extraccion,
        whosampled_url,
        creador: build_creator_summary(
            record.creator_id,
            record.creator_username,
            record.creator_nombre_visible,
            record.creator_avatar_url,
            record.creator_verificado,
            public_base_url,
        ),
    }
}

fn build_creator_summary(
    id: i32,
    username: String,
    nombre_visible: Option<String>,
    avatar_url: Option<String>,
    verificado: bool,
    public_base_url: Option<&str>,
) -> SampleCreatorSummary {
    SampleCreatorSummary {
        id,
        username,
        nombre_visible,
        avatar_url: asset_to_public_url(public_base_url, avatar_url),
        verificado,
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

fn normalize_filters(
    query: ListSamplesQuery,
    viewer_id: Option<i32>,
) -> Result<SampleListFilters, AppError> {
    let search = normalize_search(query.search, query.search_normalized)?;
    let creator = normalize_creator(query.creator)?;
    let tags = normalize_tags(query.tags)?;

    Ok(SampleListFilters {
        page: query.page.unwrap_or(DEFAULT_PAGE),
        per_page: query.per_page.unwrap_or(DEFAULT_PER_PAGE),
        viewer_id,
        search,
        bpm: query.bpm,
        music_key: normalize_music_key(query.key)?,
        sample_type: normalize_sample_type(query.sample_type)?,
        tags,
        exclude_tags: Vec::new(),
        premium: query.premium,
        creator,
        sort: SampleSortOrder::Smart,
    })
}

fn normalize_similar_limit(query: &SimilarSamplesQuery) -> i64 {
    query.limit.unwrap_or(DEFAULT_SIMILAR_LIMIT)
}

fn normalize_search(
    raw: Option<String>,
    raw_normalized: Option<String>,
) -> Result<Option<SampleTextSearch>, AppError> {
    let Some(raw) = raw else {
        return Ok(None);
    };

    let query = raw.trim();
    if query.is_empty() {
        return Ok(None);
    }

    if query.chars().count() < 2 {
        return Err(AppError::Validation(
            "search debe tener al menos 2 caracteres".into(),
        ));
    }

    if query.len() > MAX_SEARCH_LENGTH {
        return Err(AppError::Validation(format!(
            "search no puede superar {MAX_SEARCH_LENGTH} caracteres"
        )));
    }

    let normalized = raw_normalized
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty() && value != &query.to_ascii_lowercase());

    Ok(Some(SampleTextSearch::new(query.to_string(), normalized)))
}

fn normalize_update_request(request: UpdateSampleRequest) -> Result<UpdateSamplePatch, AppError> {
    let mut patch = UpdateSamplePatch::default();

    if let Some(titulo) = request.titulo {
        let titulo = titulo.trim();
        if titulo.is_empty() {
            return Err(AppError::Validation("titulo no puede quedar vacio".into()));
        }
        patch.titulo = Some(titulo.to_string());
    }

    if let Some(descripcion) = request.descripcion {
        patch.descripcion = Some(descripcion.trim().to_string());
    }

    if let Some(tags) = request.tags {
        patch.tags = Some(normalize_sample_tags(tags)?);
    }

    patch.sample_type = normalize_sample_type(request.sample_type)?;

    if let Some(es_premium) = request.es_premium {
        patch.es_premium = Some(es_premium);
        if !es_premium {
            patch.precio = Some(None);
        }
    }

    if let Some(precio) = request.precio {
        if request.es_premium == Some(false) {
            return Err(AppError::Validation(
                "precio no puede enviarse cuando es_premium es false".into(),
            ));
        }
        patch.precio = Some(Some(precio));
    }

    patch.permitir_descarga = request.permitir_descarga;
    patch.licencia_libre = request.licencia_libre;
    patch.mostrar_en_comunidad = request.mostrar_en_comunidad;

    if let Some(imagen_url) = request.imagen_url {
        patch.imagen_url = Some(normalize_image_url_update(&imagen_url));
    }

    if !patch.has_changes() {
        return Err(AppError::Validation(
            "No se recibieron campos para actualizar".into(),
        ));
    }

    Ok(patch)
}

fn normalize_creator(raw: Option<String>) -> Result<Option<String>, AppError> {
    let Some(raw) = raw else {
        return Ok(None);
    };

    let creator = raw.trim();
    if creator.is_empty() {
        return Ok(None);
    }

    if creator.len() > MAX_CREATOR_LENGTH {
        return Err(AppError::Validation(format!(
            "creator no puede superar {MAX_CREATOR_LENGTH} caracteres"
        )));
    }

    Ok(Some(creator.to_string()))
}

fn normalize_tags(raw: Option<String>) -> Result<Vec<String>, AppError> {
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };

    let mut tags = Vec::new();
    for tag in raw.split(',') {
        let normalized = tag.trim().to_ascii_lowercase();
        if normalized.is_empty() || tags.iter().any(|value| value == &normalized) {
            continue;
        }
        tags.push(normalized);
    }

    if tags.len() > MAX_TAG_FILTERS {
        return Err(AppError::Validation(format!(
            "tags soporta hasta {MAX_TAG_FILTERS} valores por consulta"
        )));
    }

    Ok(tags)
}

fn normalize_sample_tags(raw_tags: Vec<String>) -> Result<Vec<String>, AppError> {
    let mut tags = Vec::new();

    for tag in raw_tags {
        let normalized = tag.trim().to_ascii_lowercase();
        if normalized.is_empty() || tags.iter().any(|value| value == &normalized) {
            continue;
        }
        tags.push(normalized);
    }

    if tags.len() < MIN_SAMPLE_TAGS {
        return Err(AppError::Validation(format!(
            "Se requieren al menos {MIN_SAMPLE_TAGS} tags para actualizar un sample"
        )));
    }

    if tags.len() > MAX_SAMPLE_TAGS {
        return Err(AppError::Validation(format!(
            "tags soporta hasta {MAX_SAMPLE_TAGS} valores por sample"
        )));
    }

    Ok(tags)
}

fn normalize_image_url_update(raw: &str) -> Option<String> {
    let image = raw.trim();
    if image.is_empty() {
        None
    } else {
        Some(image.to_string())
    }
}

fn normalize_music_key(raw: Option<String>) -> Result<Option<String>, AppError> {
    let Some(raw) = raw else {
        return Ok(None);
    };

    let compact = raw.trim().replace(' ', "");
    if compact.is_empty() {
        return Ok(None);
    }

    let normalized = match compact.to_ascii_uppercase().as_str() {
        "A" => "A",
        "A#" => "A#",
        "BB" => "Bb",
        "B" => "B",
        "C" => "C",
        "C#" => "C#",
        "DB" => "Db",
        "D" => "D",
        "D#" => "D#",
        "EB" => "Eb",
        "E" => "E",
        "F" => "F",
        "F#" => "F#",
        "GB" => "Gb",
        "G" => "G",
        "G#" => "G#",
        "AB" => "Ab",
        _ => {
            return Err(AppError::Validation(
                "key inválida. Usa notas como C, F#, Bb o Eb".into(),
            ))
        }
    };

    Ok(Some(normalized.to_string()))
}

fn normalize_sample_type(raw: Option<String>) -> Result<Option<String>, AppError> {
    let Some(raw) = raw else {
        return Ok(None);
    };

    let compact = raw.trim().to_ascii_lowercase().replace([' ', '-'], "_");
    if compact.is_empty() {
        return Ok(None);
    }

    let normalized = match compact.as_str() {
        "loop" => "loop",
        "oneshot" | "one_shot" => "oneshot",
        "fx" => "fx",
        "vocal" | "vocals" => "vocal",
        "stem" | "stems" => "stem",
        "otro" | "other" => "otro",
        _ => {
            return Err(AppError::Validation(
                "type inválido. Usa loop, oneshot, fx, vocal, stem u otro".into(),
            ))
        }
    };

    Ok(Some(normalized.to_string()))
}
