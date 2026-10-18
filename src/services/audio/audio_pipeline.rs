mod service;

use crate::audio::bpm::BpmAnalysis;
use crate::audio::embeddings::AudioEmbedding;
use crate::audio::ffmpeg::AudioMetadata;
use crate::audio::key::KeyAnalysis;
use crate::repositories::AudioPipelineSample;
use crate::services::FileStorage;
use crate::errors::AppError;
/* Solo los tests (vía `use super::*`) usan estas funciones desde la raíz. */
#[cfg(test)]
use crate::audio::bpm::detect_bpm_from_file;
#[cfg(test)]
use crate::audio::ffmpeg::inspect_audio_file;
#[cfg(test)]
use crate::audio::key::detect_key_from_file;
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
