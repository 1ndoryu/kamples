use super::{
    build_derivative_key, build_pipeline_metadata, build_technical_analysis,
    create_temporary_workspace, infer_input_extension, is_unique_violation,
    map_activation_error, map_sqlx_error, map_storage_error,
    normalize_sample_type_for_embedding, run_async_stage, run_blocking_stage, AudioPipelineError,
    AudioPipelineRequest, AudioPipelineResult, AudioPipelineService, AudioPipelineStage,
    AudioTechnicalAnalysis, GeneratedAudioAssets, PipelineProgress, AUDIO_PIPELINE_SEMAPHORE,
    BPM_TIMEOUT, INSPECT_TIMEOUT, KEY_TIMEOUT, MP3_TIMEOUT,
};
use crate::audio::bpm::{detect_bpm_from_file, BpmAnalysis};
use crate::audio::embeddings::{AudioEmbedding, EmbeddingInput};
use crate::audio::ffmpeg::{convert_to_mp3, inspect_audio_file, AudioMetadata};
use crate::audio::key::{detect_key_from_file, KeyAnalysis};
use crate::repositories::{
    AudioPipelineSample, CompleteAudioPipelineParams, SampleRepository, SaveAudioAnalysisParams,
    SaveAudioAssetsParams,
};
use crate::repositories::MarkAudioPipelineFailedParams;
use crate::services::FileStorage;
use chrono::Utc;
use serde_json::json;
use sqlx::PgPool;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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

    pub(super) fn build_embedding(
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
