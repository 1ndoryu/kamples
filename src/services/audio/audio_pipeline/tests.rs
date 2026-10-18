use super::*;
use std::f32::consts::PI;
use std::path::PathBuf;
use uuid::Uuid;

#[test]
fn derivative_key_keeps_parent_directory() {
    let key = build_derivative_key("samples/2026/04/77/demo.wav", "abc123", "waveform", "json");
    assert_eq!(key, "samples/2026/04/77/abc123_waveform.json");
}

#[test]
fn derivative_key_works_without_parent() {
    let key = build_derivative_key("demo.wav", "abc123", "optimizado", "mp3");
    assert_eq!(key, "abc123_optimizado.mp3");
}

#[test]
fn sample_type_normalization_preserves_legacy_one_shot() {
    assert_eq!(normalize_sample_type_for_embedding("oneshot"), "one_shot");
    assert_eq!(normalize_sample_type_for_embedding("loop"), "loop");
}

#[test]
fn music_key_labels_match_detector_indexes() {
    assert_eq!(music_key_label(0), "C");
    assert_eq!(music_key_label(10), "A#");
    assert_eq!(music_key_label(13), "C#");
}

#[test]
fn pipeline_metadata_carries_partial_progress() {
    let progress = PipelineProgress {
        stage: AudioPipelineStage::PersistOptimizedMp3,
        analysis: Some(AudioTechnicalAnalysis {
            format: "wav".to_owned(),
            duration_seconds: 12.5,
            sample_rate_hz: 44_100,
            channels: 2,
            file_size_bytes: 1_024,
            bpm: Some(120),
            bpm_confidence: Some(0.82),
            music_key: Some("C#".to_owned()),
            scale: Some("minor".to_owned()),
            key_confidence: Some(0.41),
        }),
        assets: GeneratedAudioAssets {
            original_key: "samples/demo.wav".to_owned(),
            optimized_key: Some("samples/demo_optimizado.mp3".to_owned()),
            waveform_key: Some("samples/demo_waveform.json".to_owned()),
        },
        warnings: vec!["ffprobe no disponible".to_owned()],
    };

    let metadata = build_pipeline_metadata(&progress, "optimized_ready", Some("sin preview"));

    assert_eq!(metadata["audio_pipeline"]["status"], "optimized_ready");
    assert_eq!(
        metadata["audio_pipeline"]["last_stage"],
        "persist_optimized_mp3"
    );
    assert_eq!(metadata["audio_pipeline"]["music_key"], "C#");
    assert_eq!(
        metadata["audio_pipeline"]["ruta_optimizada"],
        "samples/demo_optimizado.mp3"
    );
    assert_eq!(metadata["audio_pipeline"]["last_error"], "sin preview");
}

#[tokio::test]
async fn pipeline_fixture_major_loop_extracts_expected_analysis() {
    let path = temp_wav_path("major-loop");
    let wav = build_pipeline_fixture_wav(44_100, 120, 8, &[262, 294, 330, 349, 392, 440, 494, 523]);
    std::fs::write(&path, wav).expect("should write major loop fixture");

    let inspected = inspect_audio_file(&path, Some("wav"), None)
        .await
        .expect("should inspect pipeline fixture");
    let bpm_analysis = detect_bpm_from_file(&path, Some("wav"))
        .expect("should decode pipeline fixture")
        .expect("should detect bpm from pipeline fixture");
    let key_analysis = detect_key_from_file(&path, Some("wav"))
        .expect("should decode pipeline fixture")
        .expect("should detect key from pipeline fixture");

    let analysis = build_technical_analysis(&inspected, Some(&bpm_analysis), Some(&key_analysis));
    let mut progress = PipelineProgress {
        stage: AudioPipelineStage::DetectKey,
        analysis: Some(analysis.clone()),
        assets: GeneratedAudioAssets {
            original_key: "samples/tests/fixture-major.wav".to_owned(),
            optimized_key: None,
            waveform_key: None,
        },
        warnings: Vec::new(),
    };
    let sample = fixture_sample("fixture-major", "loop", false, &["House", "Bright"]);
    let embedding = AudioPipelineService::build_embedding(&sample, &analysis, &mut progress);

    let _ = std::fs::remove_file(&path);

    assert_eq!(analysis.format, "wav");
    assert_eq!(analysis.sample_rate_hz, 44_100);
    assert_eq!(analysis.channels, 1);
    assert!(analysis.duration_seconds > 3.9 && analysis.duration_seconds < 4.1);
    assert!(analysis
        .bpm
        .is_some_and(|value| (i64::from(value) - 120).abs() <= 2));
    assert_eq!(analysis.music_key.as_deref(), Some("C"));
    assert_eq!(analysis.scale.as_deref(), Some("major"));
    assert!(analysis.file_size_bytes > 1_000);
    assert!(inspected.waveform_peaks.iter().any(|peak| *peak > 0.1));
    assert_eq!(progress.stage, AudioPipelineStage::BuildEmbedding);
    assert_eq!(embedding.as_slice().len(), 128);
    assert!(embedding.as_slice()[0] > 0.35);
}

#[tokio::test]
async fn pipeline_fixture_minor_one_shot_updates_metadata_and_embedding() {
    let path = temp_wav_path("minor-one-shot");
    let wav = build_pipeline_fixture_wav(44_100, 90, 8, &[262, 294, 311, 349, 392, 415, 466, 523]);
    std::fs::write(&path, wav).expect("should write minor fixture");

    let inspected = inspect_audio_file(&path, Some("wav"), None)
        .await
        .expect("should inspect pipeline fixture");
    let bpm_analysis = detect_bpm_from_file(&path, Some("wav"))
        .expect("should decode pipeline fixture")
        .expect("should detect bpm from pipeline fixture");
    let key_analysis = detect_key_from_file(&path, Some("wav"))
        .expect("should decode pipeline fixture")
        .expect("should detect key from pipeline fixture");

    let analysis = build_technical_analysis(&inspected, Some(&bpm_analysis), Some(&key_analysis));
    let mut progress = PipelineProgress {
        stage: AudioPipelineStage::PersistAnalysis,
        analysis: Some(analysis.clone()),
        assets: GeneratedAudioAssets {
            original_key: "samples/tests/fixture-minor.wav".to_owned(),
            optimized_key: Some("samples/tests/fixture-minor_optimizado.mp3".to_owned()),
            waveform_key: Some("samples/tests/fixture-minor_waveform.json".to_owned()),
        },
        warnings: vec!["fixture".to_owned()],
    };
    let sample = fixture_sample("fixture-minor", "oneshot", true, &["Dark", "Drums"]);
    let embedding = AudioPipelineService::build_embedding(&sample, &analysis, &mut progress);
    let metadata = build_pipeline_metadata(&progress, "analyzed", None);

    let _ = std::fs::remove_file(&path);

    assert!(analysis
        .bpm
        .is_some_and(|value| (i64::from(value) - 90).abs() <= 2));
    assert_eq!(analysis.music_key.as_deref(), Some("C"));
    assert_eq!(analysis.scale.as_deref(), Some("minor"));
    assert_eq!(metadata["audio_pipeline"]["status"], "analyzed");
    assert_eq!(metadata["audio_pipeline"]["music_key"], "C");
    assert_eq!(metadata["audio_pipeline"]["scale"], "minor");
    assert_eq!(
        metadata["audio_pipeline"]["ruta_optimizada"],
        "samples/tests/fixture-minor_optimizado.mp3"
    );
    assert!(embedding.as_slice()[16] > 0.99);
    assert!(embedding.as_slice()[21] > 0.99);
}

fn fixture_sample(
    id_corto: &str,
    tipo: &str,
    es_premium: bool,
    tags: &[&str],
) -> AudioPipelineSample {
    AudioPipelineSample {
        id: 99,
        id_corto: id_corto.to_owned(),
        formato: "wav".to_owned(),
        tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
        tipo: tipo.to_owned(),
        es_premium,
        metadata: serde_json::json!({}),
        estado: "procesando".to_owned(),
        ruta_original: format!("samples/tests/{id_corto}.wav"),
    }
}

fn build_pipeline_fixture_wav(
    sample_rate_hz: u32,
    bpm: u32,
    beats: u32,
    frequencies_hz: &[u32],
) -> Vec<u8> {
    let samples_per_beat =
        usize::try_from((u64::from(sample_rate_hz) * 60) / u64::from(bpm.max(1)))
            .expect("beat length should fit in usize");
    let total_samples =
        samples_per_beat * usize::try_from(beats).expect("beats should fit in usize");
    let note_samples = (samples_per_beat / 3).max(1);
    let sample_rate_f32 = f32::from(u16::try_from(sample_rate_hz).unwrap_or(u16::MAX));
    let note_samples_f32 = f32::from(u16::try_from(note_samples).unwrap_or(u16::MAX));
    let mut mix = vec![0.0_f32; total_samples];

    for beat in 0..usize::try_from(beats).expect("beats should fit in usize") {
        let beat_start = beat * samples_per_beat;
        let frequency_hz = frequencies_hz[beat % frequencies_hz.len()];
        let phase_step =
            2.0 * PI * f32::from(u16::try_from(frequency_hz).unwrap_or(u16::MAX)) / sample_rate_f32;
        let mut phase = 0.0_f32;

        for offset in 0..note_samples {
            let index = beat_start + offset;
            if index >= mix.len() {
                break;
            }

            let offset_f32 = f32::from(u16::try_from(offset).unwrap_or(u16::MAX));
            let envelope = (1.0 - (offset_f32 / note_samples_f32)).clamp(0.25, 1.0);
            mix[index] = phase.sin() * 0.75 * envelope;
            phase += phase_step;
            if phase >= 2.0 * PI {
                phase -= 2.0 * PI;
            }
        }
    }

    let peak = mix
        .iter()
        .fold(0.0_f32, |current, sample| current.max(sample.abs()));
    let scale = if peak > 0.95 { 0.95 / peak } else { 1.0 };
    let pcm: Vec<i16> = mix
        .iter()
        .map(|sample| quantize_f32_to_i16(*sample * scale * f32::from(i16::MAX)))
        .collect();

    write_wav_i16(sample_rate_hz, &pcm)
}

fn write_wav_i16(sample_rate_hz: u32, pcm: &[i16]) -> Vec<u8> {
    let data_len = u32::try_from(pcm.len() * 2).expect("fixture should fit in u32");
    let mut wav = Vec::with_capacity(44 + usize::try_from(data_len).unwrap_or(0));
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&sample_rate_hz.to_le_bytes());
    wav.extend_from_slice(&(sample_rate_hz * 2).to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for sample in pcm {
        wav.extend_from_slice(&sample.to_le_bytes());
    }

    wav
}

fn temp_wav_path(prefix: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "kamples-audio-pipeline-{prefix}-{}.wav",
        Uuid::new_v4()
    ));
    path
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn quantize_f32_to_i16(value: f32) -> i16 {
    value
        .round()
        .clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16
}
