use crate::audio::ia::json_repairer::AudioCreativeMetadata;
use crate::audio::ia::prompts::AudioExtractionContext;
use crate::repositories::{ApplyAudioIaMetadataParams, AudioIaSample, SampleRepository};
use crate::services::{
    AudioIaAnalysisRequest, AudioIaAnalysisResult, AudioIaProvider, AudioIaService,
    AudioIaServiceError,
};
use chrono::Utc;
use serde_json::{json, Value};
use slug::slugify;
use sqlx::PgPool;
use std::fmt::Write as _;
use thiserror::Error;

const DEFAULT_FOLDER: &str = "General";

/* [174A-42] Orquestador de jobs de ia_queue.
 * Mantiene al worker delgado: carga el sample, arma el request para el LLM,
 * preserva carpetas manuales y persiste la metadata creativa ya normalizada. */

#[derive(Clone)]
pub struct IaQueueService {
    pool: PgPool,
    ia_service: AudioIaService,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IaQueueProcessRequest {
    pub sample_id: i32,
    pub job_metadata: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IaQueueProcessResult {
    pub sample_id: i32,
    pub provider: AudioIaProvider,
    pub model: String,
    pub title_updated: bool,
}

#[derive(Debug, Error)]
pub enum IaQueueServiceError {
    #[error("Sample no encontrado para IA: {0}")]
    SampleNotFound(i32),
    #[error("Estado invalido para IA en sample {sample_id}: {state}")]
    InvalidSampleState { sample_id: i32, state: String },
    #[error("Fallo de analisis IA: {0}")]
    Analysis(#[from] AudioIaServiceError),
    #[error("Fallo persistiendo metadata IA: {0}")]
    Persist(#[from] sqlx::Error),
}

#[derive(Debug, Clone, PartialEq)]
struct PreparedAudioIaUpdate {
    titulo: Option<String>,
    slug: Option<String>,
    descripcion: Option<String>,
    tipo: String,
    metadata: Value,
}

impl IaQueueService {
    pub fn from_env(pool: PgPool) -> Result<Self, AudioIaServiceError> {
        Ok(Self {
            pool,
            ia_service: AudioIaService::from_env()?,
        })
    }

    #[must_use]
    pub fn new(pool: PgPool, ia_service: AudioIaService) -> Self {
        Self { pool, ia_service }
    }

    pub async fn run(
        &self,
        request: IaQueueProcessRequest,
    ) -> Result<IaQueueProcessResult, IaQueueServiceError> {
        let sample = SampleRepository::find_audio_ia_sample(&self.pool, request.sample_id)
            .await?
            .ok_or(IaQueueServiceError::SampleNotFound(request.sample_id))?;

        if sample.estado != "activo" && sample.estado != "en_supervision" {
            return Err(IaQueueServiceError::InvalidSampleState {
                sample_id: sample.id,
                state: sample.estado,
            });
        }

        let analysis_request = build_analysis_request(&sample, &request.job_metadata);
        let analysis_result = self.ia_service.analyze_audio(&analysis_request).await?;
        let prepared_update = build_prepared_update(&sample, &analysis_result);

        SampleRepository::apply_audio_ia_metadata(
            &self.pool,
            ApplyAudioIaMetadataParams {
                sample_id: sample.id,
                titulo: prepared_update.titulo.clone(),
                slug: prepared_update.slug.clone(),
                descripcion: prepared_update.descripcion.clone(),
                tipo: prepared_update.tipo.clone(),
                metadata: prepared_update.metadata,
            },
        )
        .await?;

        Ok(IaQueueProcessResult {
            sample_id: sample.id,
            provider: analysis_result.provider,
            model: analysis_result.model,
            title_updated: prepared_update.titulo.is_some(),
        })
    }
}

impl IaQueueServiceError {
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Analysis(error) => error.is_retryable(),
            Self::Persist(_) => true,
            Self::SampleNotFound(_) | Self::InvalidSampleState { .. } => false,
        }
    }

    #[must_use]
    pub fn retry_after_seconds(&self) -> Option<f32> {
        match self {
            Self::Analysis(error) => error.retry_after_seconds(),
            Self::Persist(_) | Self::SampleNotFound(_) | Self::InvalidSampleState { .. } => None,
        }
    }
}

fn build_analysis_request(sample: &AudioIaSample, job_metadata: &Value) -> AudioIaAnalysisRequest {
    AudioIaAnalysisRequest {
        original_filename: metadata_string(job_metadata, "filename_original")
            .or_else(|| metadata_string(&sample.metadata, "filename_original"))
            .unwrap_or_else(|| sample.titulo.clone()),
        user_description: sample.descripcion.clone(),
        user_tags: sample.tags.clone(),
        bpm: sample
            .bpm
            .and_then(|value| i16::try_from(value).ok())
            .map(f32::from),
        musical_key: sample.music_key.clone(),
        scale: sample.scale.clone(),
        duration_seconds: Some(sample.duration_seconds),
        upload_origin: metadata_string(job_metadata, "origen_subida")
            .or_else(|| metadata_string(&sample.metadata, "origen_subida")),
        extraction_context: build_extraction_context(&sample.metadata),
    }
}

/* [294A-6] Reconstruye el contexto de extraccion (fuente/destino/tipo) que el
 * scraper Python guardo en `samples.metadata.extraccion`. El legacy PHP usaba
 * exactamente este bloque para enriquecer el prompt del LLM y producir titulos
 * y descripciones especificas del sampleo (ver PromptsIA::construirAnalisis). */
fn build_extraction_context(metadata: &Value) -> Option<AudioExtractionContext> {
    let raw = metadata.get("extraccion").filter(|v| v.is_object())?;
    let context = AudioExtractionContext {
        source_title: metadata_string(raw, "fuente_titulo"),
        source_artist: metadata_string(raw, "fuente_artista"),
        destination_title: metadata_string(raw, "destino_titulo"),
        destination_artist: metadata_string(raw, "destino_artista"),
        element_type: metadata_string(raw, "tipo_elemento"),
        vote_count: raw
            .get("votos_total")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok()),
    };
    if context.source_title.is_some()
        || context.destination_title.is_some()
        || context.element_type.is_some()
    {
        Some(context)
    } else {
        None
    }
}

fn build_prepared_update(
    sample: &AudioIaSample,
    analysis_result: &AudioIaAnalysisResult,
) -> PreparedAudioIaUpdate {
    let metadata = &analysis_result.metadata;
    let current_primary = metadata_string(&sample.metadata, "carpeta_primaria");
    let current_secondary = metadata_string(&sample.metadata, "carpeta_secundaria");
    let preserve_existing_folder = current_primary.as_deref().is_some_and(|value| {
        !value.trim().is_empty() && !value.eq_ignore_ascii_case(DEFAULT_FOLDER)
    });
    let carpeta_primaria = if preserve_existing_folder {
        current_primary
            .clone()
            .unwrap_or_else(|| DEFAULT_FOLDER.to_owned())
    } else {
        normalize_folder(&metadata.carpeta_primaria)
    };
    let carpeta_secundaria = if preserve_existing_folder {
        current_secondary.unwrap_or_else(|| DEFAULT_FOLDER.to_owned())
    } else {
        normalize_folder(&metadata.carpeta_secundaria)
    };
    let titulo = build_generated_title(sample, metadata);
    let slug = titulo
        .as_ref()
        .map(|value| format!("{}-{}", slugify(value), sample.id_corto));
    let descripcion = non_empty_text(&metadata.descripcion_corta_es)
        .or_else(|| non_empty_text(&metadata.descripcion_es));

    PreparedAudioIaUpdate {
        titulo,
        slug,
        descripcion,
        tipo: metadata.tipo.clone(),
        metadata: json!({
            "nombre_archivo_base": metadata.nombre_archivo_base,
            "tags": metadata.tags,
            "tags_es": metadata.tags_es,
            "genero": metadata.genero,
            "emocion": metadata.emocion,
            "emocion_es": metadata.emocion_es,
            "instrumentos": metadata.instrumentos,
            "artista_vibes": metadata.artista_vibes,
            "descripcion_corta": metadata.descripcion_corta,
            "descripcion_corta_es": metadata.descripcion_corta_es,
            "descripcion": metadata.descripcion,
            "descripcion_es": metadata.descripcion_es,
            "carpeta_primaria": carpeta_primaria,
            "carpeta_secundaria": carpeta_secundaria,
            "ia_carpeta_primaria": normalize_folder(&metadata.carpeta_primaria),
            "ia_carpeta_secundaria": normalize_folder(&metadata.carpeta_secundaria),
            "ia_pending": false,
            "ia_queue_status": "completed",
            "ia_provider": analysis_result.provider.as_str(),
            "ia_model": analysis_result.model,
            "ia_analizada_at": Utc::now(),
            "reprocesado_at": Utc::now(),
            "origen_subida": metadata_string(&sample.metadata, "origen_subida"),
        }),
    }
}

fn build_generated_title(
    sample: &AudioIaSample,
    metadata: &AudioCreativeMetadata,
) -> Option<String> {
    let base_name = non_empty_text(&metadata.nombre_archivo_base)?;
    let mut title = title_case(&base_name);

    if let Some(bpm) = sample.bpm.filter(|value| *value > 0) {
        title.push(' ');
        /* `write!` sobre un `String` no puede fallar; se descarta el Result. */
        let _ = write!(&mut title, "{bpm}bpm");
    }

    if let Some(music_key) = sample
        .music_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        title.push(' ');
        title.push_str(music_key.trim());
        if sample
            .scale
            .as_deref()
            .is_some_and(|value| value.eq_ignore_ascii_case("minor"))
        {
            title.push('m');
        }
    }

    Some(title)
}

fn title_case(value: &str) -> String {
    value
        .split_whitespace()
        .filter(|segment| !segment.is_empty())
        .map(capitalize_word)
        .collect::<Vec<_>>()
        .join(" ")
}

fn capitalize_word(value: &str) -> String {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return String::new();
    };

    let mut result = String::new();
    result.extend(first.to_uppercase());
    result.push_str(characters.as_str());
    result
}

fn metadata_string(metadata: &Value, key: &str) -> Option<String> {
    metadata
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn normalize_folder(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        DEFAULT_FOLDER.to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn non_empty_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

#[cfg(test)]
#[path = "ia_queue/tests.rs"]
mod tests;
