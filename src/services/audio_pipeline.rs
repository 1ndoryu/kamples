use crate::audio::bpm::{detect_bpm_from_file, BpmAnalysis};
use crate::audio::embeddings::{AudioEmbedding, EmbeddingInput};
use crate::audio::ffmpeg::{convert_to_mp3, inspect_audio_file, AudioMetadata};
use crate::audio::key::{detect_key_from_file, KeyAnalysis};
use crate::repositories::{
    AudioPipelineSample, CompleteAudioPipelineParams, SampleRepository, SaveAudioAnalysisParams,
    SaveAudioAssetsParams,
};
use crate::services::FileStorage;
use crate::{errors::AppError, repositories::MarkAudioPipelineFailedParams};
use chrono::Utc;
use serde_json::json;
use sqlx::PgPool;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Semaphore;
use tokio::time::timeout;

const MAX_PIPELINE_CONCURRENCY: usize = 2;
const INSPECT_TIMEOUT: Duration = Duration::from_secs(30);
const BPM_TIMEOUT: Duration = Duration::from_secs(20);
const KEY_TIMEOUT: Duration = Duration::from_secs(20);
const MP3_TIMEOUT: Duration = Duration::from_mins(1);
static AUDIO_PIPELINE_SEMAPHORE: Semaphore = Semaphore::const_new(MAX_PIPELINE_CONCURRENCY);
const MUSIC_KEY_LABELS: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/* [174A-34] Orquestador técnico del pipeline de audio.
 * Ejecuta inspección, BPM, key, derivados y embedding sin asumir LocalFs: el
 * original se materializa desde FileStorage en un workspace temporal y los
 * derivados se suben otra vez al storage lógico. */

#[derive(Clone)]
pub struct AudioPipelineService {
    pool: PgPool,
    storage: Arc<dyn FileStorage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioPipelineRequest {
    pub sample_id: i32,
    pub force_recompute: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioPipelineResult {
    pub sample_id: i32,
    pub analysis: AudioTechnicalAnalysis,
    pub assets: GeneratedAudioAssets,
    pub embedding: AudioEmbedding,
    pub activated: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioTechnicalAnalysis {
    pub format: String,
    pub duration_seconds: f32,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub file_size_bytes: u64,
    pub bpm: Option<u32>,
    pub bpm_confidence: Option<f32>,
    pub music_key: Option<String>,
    pub scale: Option<String>,
    pub key_confidence: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedAudioAssets {
    pub original_key: String,
    pub optimized_key: Option<String>,
    pub waveform_key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioPipelineStage {
    LoadSample,
    DownloadOriginal,
    Inspect,
    DetectBpm,
    DetectKey,
    PersistAnalysis,
    GenerateWaveform,
    PersistWaveform,
    GenerateOptimizedMp3,
    PersistOptimizedMp3,
    BuildEmbedding,
    ActivateSample,
}

impl AudioPipelineStage {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LoadSample => "load_sample",
            Self::DownloadOriginal => "download_original",
            Self::Inspect => "inspect",
            Self::DetectBpm => "detect_bpm",
            Self::DetectKey => "detect_key",
            Self::PersistAnalysis => "persist_analysis",
            Self::GenerateWaveform => "generate_waveform",
            Self::PersistWaveform => "persist_waveform",
            Self::GenerateOptimizedMp3 => "generate_optimized_mp3",
            Self::PersistOptimizedMp3 => "persist_optimized_mp3",
            Self::BuildEmbedding => "build_embedding",
            Self::ActivateSample => "activate_sample",
        }
    }
}

#[derive(Debug, Error)]
pub enum AudioPipelineError {
    #[error("Sample no encontrado para pipeline: {0}")]
    SampleNotFound(i32),
    #[error("Estado inválido para pipeline en sample {sample_id}: {state}")]
    InvalidSampleState { sample_id: i32, state: String },
    #[error("El sample {0} no tiene ruta_original válida")]
    MissingOriginalAsset(i32),
    #[error("Timeout en etapa {stage} tras {seconds}s")]
    Timeout { stage: &'static str, seconds: u64 },
    #[error("Fallo en etapa {stage}: {message}")]
    Stage {
        stage: &'static str,
        message: String,
        retryable: bool,
    },
}

impl AudioPipelineError {
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Stage { retryable, .. } => *retryable,
            Self::Timeout { .. } => true,
            Self::SampleNotFound(_)
            | Self::InvalidSampleState { .. }
            | Self::MissingOriginalAsset(_) => false,
        }
    }

    fn stage(stage: AudioPipelineStage, message: impl Into<String>, retryable: bool) -> Self {
        Self::Stage {
            stage: stage.as_str(),
            message: message.into(),
            retryable,
        }
    }

    fn timeout(stage: AudioPipelineStage, duration: Duration) -> Self {
        Self::Timeout {
            stage: stage.as_str(),
            seconds: duration.as_secs(),
        }
    }
}

#[derive(Debug, Clone)]
struct PipelineProgress {
    stage: AudioPipelineStage,
    analysis: Option<AudioTechnicalAnalysis>,
    assets: GeneratedAudioAssets,
    warnings: Vec<String>,
}

impl AudioPipelineService {
    #[must_use]
    pub fn new(pool: PgPool, storage: Arc<dyn FileStorage>) -> Self {
        Self { pool, storage }
    }

    pub async fn run(
        &self,
        request: AudioPipelineRequest,
    ) -> Result<AudioPipelineResult, AudioPipelineError> {
        let sample = SampleRepository::find_pipeline_sample(&self.pool, request.sample_id)
            .await
            .map_err(|error| map_sqlx_error(AudioPipelineStage::LoadSample, &error))?
            .ok_or(AudioPipelineError::SampleNotFound(request.sample_id))?;

        if !request.force_recompute && sample.estado != "procesando" {
            return Err(AudioPipelineError::InvalidSampleState {
                sample_id: sample.id,
                state: sample.estado,
            });
        }
        if sample.ruta_original.trim().is_empty() {
            return Err(AudioPipelineError::MissingOriginalAsset(sample.id));
        }

        let _permit = AUDIO_PIPELINE_SEMAPHORE.acquire().await.map_err(|error| {
            AudioPipelineError::stage(AudioPipelineStage::LoadSample, error.to_string(), true)
        })?;

        let mut progress = PipelineProgress {
            stage: AudioPipelineStage::LoadSample,
            analysis: None,
            assets: GeneratedAudioAssets {
                original_key: sample.ruta_original.clone(),
                optimized_key: None,
                waveform_key: None,
            },
            warnings: Vec::new(),
        };

        let workspace = create_temporary_workspace(sample.id).await?;
        let result = self.run_inner(&sample, &workspace, &mut progress).await;

        if let Err(error) = &result {
            if let Err(persist_error) = self.persist_failure(sample.id, &progress, error).await {
                tracing::error!(
                    sample_id = sample.id,
                    ?persist_error,
                    "no se pudo persistir fallo del audio pipeline"
                );
            }
        }

        let cleanup_result = tokio::fs::remove_dir_all(&workspace).await;
        if let Err(error) = cleanup_result {
            tracing::warn!(sample_id = sample.id, path = %workspace.display(), %error, "no se pudo limpiar workspace temporal del pipeline");
        }

        result
    }

    async fn run_inner(
        &self,
        sample: &AudioPipelineSample,
        workspace: &Path,
        progress: &mut PipelineProgress,
    ) -> Result<AudioPipelineResult, AudioPipelineError> {
        // [294A-5] Logs por etapa para diagnostico de cuellos de botella.
        let started = std::time::Instant::now();
        tracing::debug!(sample_id = sample.id, "pipeline: materialize_original");
        let input_path = self
            .materialize_original(sample, workspace, progress)
            .await?;
        tracing::debug!(
            sample_id = sample.id,
            elapsed_secs = started.elapsed().as_secs_f64(),
            "pipeline: analyze_original"
        );
        let (inspected, bpm_analysis, key_analysis) =
            self.analyze_original(sample, &input_path, progress).await?;
        tracing::debug!(
            sample_id = sample.id,
            elapsed_secs = started.elapsed().as_secs_f64(),
            "pipeline: persist_analysis"
        );
        let analysis =
            build_technical_analysis(&inspected, bpm_analysis.as_ref(), key_analysis.as_ref());
        progress.analysis = Some(analysis.clone());
        self.persist_analysis(sample, progress, &analysis).await?;
        tracing::debug!(
            sample_id = sample.id,
            elapsed_secs = started.elapsed().as_secs_f64(),
            "pipeline: waveform"
        );
        self.generate_and_store_waveform(sample, progress, &inspected)
            .await?;
        tracing::debug!(
            sample_id = sample.id,
            elapsed_secs = started.elapsed().as_secs_f64(),
            "pipeline: optimized_mp3"
        );
        self.generate_and_store_optimized_mp3(sample, &input_path, workspace, progress)
            .await?;
        tracing::debug!(
            sample_id = sample.id,
            elapsed_secs = started.elapsed().as_secs_f64(),
            "pipeline: activate_sample"
        );
        let embedding = Self::build_embedding(sample, &analysis, progress);
        self.activate_sample(sample, progress, &embedding).await?;
        tracing::info!(
            sample_id = sample.id,
            elapsed_secs = started.elapsed().as_secs_f64(),
            "pipeline: completado"
        );

        Ok(AudioPipelineResult {
            sample_id: sample.id,
            analysis,
            assets: progress.assets.clone(),
            embedding,
            activated: true,
            warnings: progress.warnings.clone(),
        })
    }

    async fn persist_failure(
        &self,
        sample_id: i32,
        progress: &PipelineProgress,
        error: &AudioPipelineError,
    ) -> Result<(), sqlx::Error> {
        SampleRepository::mark_audio_pipeline_failed(
            &self.pool,
            MarkAudioPipelineFailedParams {
                sample_id,
                metadata: build_pipeline_metadata(progress, "error", Some(&error.to_string())),
            },
        )
        .await
    }

    async fn materialize_original(
        &self,
        sample: &AudioPipelineSample,
        workspace: &Path,
        progress: &mut PipelineProgress,
    ) -> Result<PathBuf, AudioPipelineError> {
        progress.stage = AudioPipelineStage::DownloadOriginal;
        let original_bytes = self
            .storage
            .get_bytes(&sample.ruta_original)
            .await
            .map_err(|error| {
                map_storage_error(AudioPipelineStage::DownloadOriginal, sample.id, error)
            })?;

        let input_path = workspace.join(format!("input.{}", infer_input_extension(sample)));
        tokio::fs::write(&input_path, &original_bytes)
            .await
            .map_err(|error| {
                AudioPipelineError::stage(
                    AudioPipelineStage::DownloadOriginal,
                    error.to_string(),
                    true,
                )
            })?;

        Ok(input_path)
    }

    async fn analyze_original(
        &self,
        sample: &AudioPipelineSample,
        input_path: &Path,
        progress: &mut PipelineProgress,
    ) -> Result<(AudioMetadata, Option<BpmAnalysis>, Option<KeyAnalysis>), AudioPipelineError> {
        let format_hint = Some(sample.formato.clone());

        progress.stage = AudioPipelineStage::Inspect;
        let inspected = run_async_stage(
            AudioPipelineStage::Inspect,
            INSPECT_TIMEOUT,
            inspect_audio_file(input_path, format_hint.as_deref(), None),
        )
        .await?;

        progress.stage = AudioPipelineStage::DetectBpm;
        let bpm_analysis = run_blocking_stage(AudioPipelineStage::DetectBpm, BPM_TIMEOUT, {
            let input_path = input_path.to_path_buf();
            let format_hint = format_hint.clone();
            move || detect_bpm_from_file(&input_path, format_hint.as_deref())
        })
        .await?;

        progress.stage = AudioPipelineStage::DetectKey;
        let key_analysis = run_blocking_stage(AudioPipelineStage::DetectKey, KEY_TIMEOUT, {
            let input_path = input_path.to_path_buf();
            let format_hint = format_hint.clone();
            move || detect_key_from_file(&input_path, format_hint.as_deref())
        })
        .await?;

        Ok((inspected, bpm_analysis, key_analysis))
    }

    async fn persist_analysis(
        &self,
        sample: &AudioPipelineSample,
        progress: &mut PipelineProgress,
        analysis: &AudioTechnicalAnalysis,
    ) -> Result<(), AudioPipelineError> {
        progress.stage = AudioPipelineStage::PersistAnalysis;
        SampleRepository::save_audio_analysis(
            &self.pool,
            SaveAudioAnalysisParams {
                sample_id: sample.id,
                duration_seconds: analysis.duration_seconds,
                formato: analysis.format.clone(),
                tamano: i64::try_from(analysis.file_size_bytes).unwrap_or(i64::MAX),
                bpm: analysis.bpm.and_then(|value| i32::try_from(value).ok()),
                music_key: analysis.music_key.clone(),
                scale: analysis.scale.clone(),
                metadata: build_pipeline_metadata(progress, "analyzed", None),
            },
        )
        .await
        .map_err(|error| map_sqlx_error(AudioPipelineStage::PersistAnalysis, &error))
    }

    async fn generate_and_store_waveform(
        &self,
        sample: &AudioPipelineSample,
        progress: &mut PipelineProgress,
        inspected: &AudioMetadata,
    ) -> Result<(), AudioPipelineError> {
        progress.stage = AudioPipelineStage::GenerateWaveform;
        let waveform_key =
            build_derivative_key(&sample.ruta_original, &sample.id_corto, "waveform", "json");
        let waveform_bytes = serde_json::to_vec(&inspected.waveform_peaks).map_err(|error| {
            AudioPipelineError::stage(
                AudioPipelineStage::GenerateWaveform,
                error.to_string(),
                false,
            )
        })?;

        progress.stage = AudioPipelineStage::PersistWaveform;
        self.storage
            .put_bytes(&waveform_key, &waveform_bytes)
            .await
            .map_err(|error| {
                map_storage_error(AudioPipelineStage::PersistWaveform, sample.id, error)
            })?;
        progress.assets.waveform_key = Some(waveform_key.clone());

        SampleRepository::save_audio_assets(
            &self.pool,
            SaveAudioAssetsParams {
                sample_id: sample.id,
                ruta_waveform: Some(waveform_key),
                ruta_optimizada: None,
                metadata: build_pipeline_metadata(progress, "waveform_ready", None),
            },
        )
        .await
        .map_err(|error| map_sqlx_error(AudioPipelineStage::PersistWaveform, &error))
    }

    async fn generate_and_store_optimized_mp3(
        &self,
        sample: &AudioPipelineSample,
        input_path: &Path,
        workspace: &Path,
        progress: &mut PipelineProgress,
    ) -> Result<(), AudioPipelineError> {
        progress.stage = AudioPipelineStage::GenerateOptimizedMp3;
        let optimized_key =
            build_derivative_key(&sample.ruta_original, &sample.id_corto, "optimizado", "mp3");
        let optimized_path = workspace.join("optimized.mp3");
        run_async_stage(
            AudioPipelineStage::GenerateOptimizedMp3,
            MP3_TIMEOUT,
            convert_to_mp3(input_path, &optimized_path, None),
        )
        .await?;

        let optimized_bytes = tokio::fs::read(&optimized_path).await.map_err(|error| {
            AudioPipelineError::stage(
                AudioPipelineStage::GenerateOptimizedMp3,
                error.to_string(),
                true,
            )
        })?;

        progress.stage = AudioPipelineStage::PersistOptimizedMp3;
        self.storage
            .put_bytes(&optimized_key, &optimized_bytes)
            .await
            .map_err(|error| {
                map_storage_error(AudioPipelineStage::PersistOptimizedMp3, sample.id, error)
            })?;
        progress.assets.optimized_key = Some(optimized_key.clone());

        SampleRepository::save_audio_assets(
            &self.pool,
            SaveAudioAssetsParams {
                sample_id: sample.id,
                ruta_waveform: progress.assets.waveform_key.clone(),
                ruta_optimizada: Some(optimized_key),
                metadata: build_pipeline_metadata(progress, "optimized_ready", None),
            },
        )
        .await
        .map_err(|error| map_sqlx_error(AudioPipelineStage::PersistOptimizedMp3, &error))
    }

    fn build_embedding(
        sample: &AudioPipelineSample,
        analysis: &AudioTechnicalAnalysis,
        progress: &mut PipelineProgress,
    ) -> AudioEmbedding {
        progress.stage = AudioPipelineStage::BuildEmbedding;
        AudioEmbedding::generate(&EmbeddingInput {
            bpm: analysis.bpm.and_then(|value| u16::try_from(value).ok()),
            music_key: analysis.music_key.clone(),
            scale: analysis.scale.clone(),
            sample_type: Some(normalize_sample_type_for_embedding(&sample.tipo)),
            duration_seconds: Some(analysis.duration_seconds),
            is_premium: sample.es_premium,
            tags: sample.tags.clone(),
        })
    }

    async fn activate_sample(
        &self,
        sample: &AudioPipelineSample,
        progress: &mut PipelineProgress,
        embedding: &AudioEmbedding,
    ) -> Result<(), AudioPipelineError> {
        progress.stage = AudioPipelineStage::ActivateSample;
        let mut metadata = build_pipeline_metadata(progress, "completed", None);
        if let Some(object) = metadata.as_object_mut() {
            object.insert("ia_pending".to_owned(), json!(true));
            object.insert("ia_queue_status".to_owned(), json!("pending"));
            object.insert("ia_enqueued_at".to_owned(), json!(Utc::now()));
        }

        let result = SampleRepository::complete_audio_pipeline(
            &self.pool,
            CompleteAudioPipelineParams {
                sample_id: sample.id,
                embedding: embedding.to_pgvector(),
                metadata,
                ia_queue_metadata: json!({
                    "queued_from": "audio_pipeline",
                    "queued_at": Utc::now(),
                }),
            },
        )
        .await;

        match result {
            Ok(()) => Ok(()),
            Err(error) if is_unique_violation(&error) => {
                /* [294A-5] El scraper genera samples con audio_hash duplicado
                 * (mismo source ya procesado). En lugar de marcar error_final,
                 * lo soft-deleteamos y cerramos el job como completado: no es
                 * un fallo real, simplemente el contenido ya existe. */
                let dup_meta = json!({
                    "audio_pipeline_duplicate": true,
                    "duplicate_detected_at": Utc::now(),
                    "duplicate_reason": "audio_hash_conflict",
                });
                if let Err(err) =
                    SampleRepository::mark_sample_as_duplicate(&self.pool, sample.id, dup_meta)
                        .await
                {
                    tracing::warn!(
                        sample_id = sample.id,
                        error = %err,
                        "no se pudo marcar sample duplicado tras conflicto de audio_hash"
                    );
                }
                tracing::info!(
                    sample_id = sample.id,
                    "pipeline: sample marcado como duplicado por audio_hash"
                );
                Ok(())
            }
            Err(error) => Err(map_activation_error(&error)),
        }
    }
}

fn build_technical_analysis(
    inspected: &AudioMetadata,
    bpm_analysis: Option<&BpmAnalysis>,
    key_analysis: Option<&KeyAnalysis>,
) -> AudioTechnicalAnalysis {
    AudioTechnicalAnalysis {
        format: inspected.format.clone(),
        duration_seconds: inspected.duration_seconds,
        sample_rate_hz: inspected.sample_rate_hz,
        channels: inspected.channels,
        file_size_bytes: inspected.file_size_bytes,
        bpm: bpm_analysis.map(|analysis| analysis.bpm),
        bpm_confidence: bpm_analysis.map(|analysis| analysis.confidence),
        music_key: key_analysis.map(|analysis| music_key_label(analysis.music_key).to_owned()),
        scale: key_analysis.map(|analysis| analysis.scale.clone()),
        key_confidence: key_analysis.map(|analysis| analysis.confidence),
    }
}

fn build_pipeline_metadata(
    progress: &PipelineProgress,
    status: &str,
    last_error: Option<&str>,
) -> serde_json::Value {
    let analysis = progress.analysis.as_ref();
    json!({
        "audio_pipeline": {
            "status": status,
            "last_stage": progress.stage.as_str(),
            "last_error": last_error,
            "warnings": progress.warnings,
            "duration_seconds": analysis.map(|value| value.duration_seconds),
            "format": analysis.as_ref().map(|value| value.format.clone()),
            "sample_rate_hz": analysis.map(|value| value.sample_rate_hz),
            "channels": analysis.map(|value| value.channels),
            "file_size_bytes": analysis.map(|value| value.file_size_bytes),
            "bpm": analysis.as_ref().and_then(|value| value.bpm),
            "bpm_confidence": analysis.as_ref().and_then(|value| value.bpm_confidence),
            "music_key": analysis.as_ref().and_then(|value| value.music_key.clone()),
            "scale": analysis.as_ref().and_then(|value| value.scale.clone()),
            "key_confidence": analysis.as_ref().and_then(|value| value.key_confidence),
            "ruta_waveform": progress.assets.waveform_key,
            "ruta_optimizada": progress.assets.optimized_key,
            "preview_ready": false,
        }
    })
}

async fn create_temporary_workspace(sample_id: i32) -> Result<PathBuf, AudioPipelineError> {
    let workspace = std::env::temp_dir().join(format!( // sentinel-disable path-join-sin-canonicalize — format! de un i32 y un Uuid: sin separadores
        "glory-audio-pipeline-{sample_id}-{}",
        uuid::Uuid::new_v4()
    ));
    tokio::fs::create_dir_all(&workspace)
        .await
        .map_err(|error| {
            AudioPipelineError::stage(
                AudioPipelineStage::DownloadOriginal,
                error.to_string(),
                true,
            )
        })?;
    Ok(workspace)
}

async fn run_async_stage<T, E, F>(
    stage: AudioPipelineStage,
    duration: Duration,
    future: F,
) -> Result<T, AudioPipelineError>
where
    F: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    match timeout(duration, future).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(AudioPipelineError::stage(stage, error.to_string(), false)),
        Err(_) => Err(AudioPipelineError::timeout(stage, duration)),
    }
}

async fn run_blocking_stage<T, E, F>(
    stage: AudioPipelineStage,
    duration: Duration,
    work: F,
) -> Result<T, AudioPipelineError>
where
    T: Send + 'static,
    E: Send + 'static + std::fmt::Display,
    F: Send + 'static + FnOnce() -> Result<T, E>,
{
    match timeout(duration, tokio::task::spawn_blocking(work)).await {
        Ok(Ok(Ok(value))) => Ok(value),
        Ok(Ok(Err(error))) => Err(AudioPipelineError::stage(stage, error.to_string(), false)),
        Ok(Err(error)) => Err(AudioPipelineError::stage(stage, error.to_string(), true)),
        Err(_) => Err(AudioPipelineError::timeout(stage, duration)),
    }
}

fn infer_input_extension(sample: &AudioPipelineSample) -> String {
    let from_format = sample.formato.trim().to_ascii_lowercase();
    if !from_format.is_empty() {
        return from_format;
    }

    Path::new(&sample.ruta_original)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "wav".to_owned())
}

fn build_derivative_key(
    original_key: &str,
    id_corto: &str,
    suffix: &str,
    extension: &str,
) -> String {
    let parent = Path::new(original_key)
        .parent()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let file_name = format!("{id_corto}_{suffix}.{extension}");

    if parent.is_empty() || parent == "." {
        file_name
    } else {
        format!("{parent}/{file_name}")
    }
}

fn music_key_label(index: u8) -> &'static str {
    MUSIC_KEY_LABELS[usize::from(index) % MUSIC_KEY_LABELS.len()]
}

fn normalize_sample_type_for_embedding(sample_type: &str) -> String {
    match sample_type.trim().to_ascii_lowercase().as_str() {
        "oneshot" | "one_shot" | "one shot" => "one_shot".to_owned(),
        other => other.to_owned(),
    }
}

fn map_sqlx_error(stage: AudioPipelineStage, error: &sqlx::Error) -> AudioPipelineError {
    AudioPipelineError::stage(stage, error.to_string(), is_retryable_sqlx_error(error))
}

fn map_activation_error(error: &sqlx::Error) -> AudioPipelineError {
    if is_unique_violation(error) {
        return AudioPipelineError::stage(
            AudioPipelineStage::ActivateSample,
            "conflicto de audio_hash al activar el sample",
            false,
        );
    }

    map_sqlx_error(AudioPipelineStage::ActivateSample, error)
}

fn map_storage_error(
    stage: AudioPipelineStage,
    sample_id: i32,
    error: AppError,
) -> AudioPipelineError {
    match error {
        AppError::NotFound(_) => AudioPipelineError::MissingOriginalAsset(sample_id),
        AppError::BadRequest(message)
        | AppError::Forbidden(message)
        | AppError::Conflict(message)
        | AppError::Internal(message)
        | AppError::UnsupportedMediaType(message)
        | AppError::Validation(message)
        | AppError::TooManyRequests(message) => AudioPipelineError::stage(stage, message, true),
        AppError::Unauthorized | AppError::RateLimited | AppError::PayloadTooLarge => {
            AudioPipelineError::stage(stage, error.to_string(), true)
        }
        AppError::Database(database_error) => map_sqlx_error(stage, &database_error),
        AppError::ExternalService { service, message } => {
            AudioPipelineError::stage(stage, format!("{service}: {message}"), true)
        }
    }
}

fn is_retryable_sqlx_error(error: &sqlx::Error) -> bool {
    !is_unique_violation(error)
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .is_some_and(sqlx::error::DatabaseError::is_unique_violation)
}

#[cfg(test)]
#[path = "audio_pipeline/tests.rs"]
mod tests;
