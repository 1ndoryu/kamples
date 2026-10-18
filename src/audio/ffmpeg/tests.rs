use super::cli::temp_file_path;
use super::*;
use serial_test::serial;

#[test]
#[serial]
fn detect_binary_prefers_env_override() {
    let path = temp_file_path(if cfg!(windows) { "exe" } else { "bin" });
    std::fs::write(&path, b"fake-binary").expect("should create fake binary");
    std::env::set_var("FFMPEG_PATH", &path);

    let detected = detection::detect_binary("ffmpeg");

    std::env::remove_var("FFMPEG_PATH");
    let _ = std::fs::remove_file(&path);
    assert_eq!(detected.as_deref(), Some(Path::new(&path)));
}

#[tokio::test]
#[serial]
async fn inspect_audio_file_uses_symphonia_fallback() {
    let input_path = temp_file_path("wav");
    std::fs::write(&input_path, build_test_wav(8_000, 1_000)).expect("should write wav fixture");

    let metadata = inspect_audio_file(&input_path, None, Some(&FFmpegBinaries::default()))
        .await
        .expect("should inspect wav without ffmpeg");

    let _ = std::fs::remove_file(input_path);
    assert_eq!(metadata.format, "wav");
    assert_eq!(metadata.sample_rate_hz, 8_000);
    assert_eq!(metadata.channels, 1);
    assert_eq!(metadata.waveform_peaks.len(), DEFAULT_WAVEFORM_BARS);
    assert!(metadata.duration_seconds > 0.95 && metadata.duration_seconds < 1.05);
    assert!(metadata.waveform_peaks.iter().any(|peak| *peak > 0.1));
}

#[tokio::test]
#[serial]
async fn convert_to_mp3_requires_ffmpeg_binary() {
    let input_path = temp_file_path("wav");
    let output_path = temp_file_path("mp3");
    std::fs::write(&input_path, build_test_wav(8_000, 250)).expect("should write wav fixture");

    let error = convert_to_mp3(&input_path, &output_path, Some(&FFmpegBinaries::default()))
        .await
        .expect_err("should fail when ffmpeg is unavailable");

    let _ = std::fs::remove_file(input_path);
    let _ = std::fs::remove_file(output_path);
    assert!(matches!(error, AudioError::FfmpegNotFound));
}

#[tokio::test]
#[serial]
async fn converts_to_mp3_and_flac_when_ffmpeg_is_available() {
    let binaries = FFmpegBinaries::detect();
    if !binaries.has_ffmpeg() {
        return;
    }

    let input_path = temp_file_path("wav");
    let mp3_path = temp_file_path("mp3");
    let flac_path = temp_file_path("flac");
    std::fs::write(&input_path, build_test_wav(8_000, 500)).expect("should write wav fixture");

    convert_to_mp3(&input_path, &mp3_path, Some(&binaries))
        .await
        .expect("ffmpeg should convert wav to mp3");
    convert_to_flac(&input_path, &flac_path, Some(&binaries))
        .await
        .expect("ffmpeg should convert wav to flac");

    let mp3_metadata = inspect_audio_file(&mp3_path, None, Some(&binaries))
        .await
        .expect("should inspect generated mp3");
    let flac_metadata = inspect_audio_file(&flac_path, None, Some(&binaries))
        .await
        .expect("should inspect generated flac");

    let _ = std::fs::remove_file(input_path);
    let _ = std::fs::remove_file(mp3_path);
    let _ = std::fs::remove_file(flac_path);

    assert_eq!(mp3_metadata.format, "mp3");
    assert_eq!(flac_metadata.format, "flac");
    assert!(mp3_metadata.file_size_bytes > 0);
    assert!(flac_metadata.file_size_bytes > 0);
    assert!(mp3_metadata.duration_seconds > 0.45);
    assert!(flac_metadata.duration_seconds > 0.45);
}

fn build_test_wav(sample_rate: u32, duration_ms: u32) -> Vec<u8> {
    let frame_count = usize::try_from(u64::from(sample_rate) * u64::from(duration_ms) / 1_000)
        .expect("fixture should fit in usize");
    let square_wave_period =
        usize::try_from((sample_rate / 440).max(2)).expect("period should fit in usize");
    let positive_peak = i16::MAX / 2;
    let negative_peak = i16::MIN / 2;
    let mut pcm = Vec::with_capacity(frame_count * 2);

    for index in 0..frame_count {
        let value = if index % square_wave_period < square_wave_period / 2 {
            positive_peak
        } else {
            negative_peak
        };
        pcm.extend_from_slice(&value.to_le_bytes());
    }

    let data_len = u32::try_from(pcm.len()).expect("fixture should fit in u32");
    let mut wav = Vec::with_capacity(pcm.len() + 44);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(&pcm);
    wav
}
