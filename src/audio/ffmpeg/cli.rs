/* [174A-30] Capa CLI: ffprobe para la duración y ffmpeg para el waveform
 * (PCM temporal). Si un binario falla, el llamador cae a Symphonia. */

use std::env;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use tokio::process::Command;
use uuid::Uuid;

use super::AudioError;

const WAVEFORM_SAMPLE_RATE: u32 = 8_000;

#[derive(Debug, Deserialize)]
struct FFprobeOutput {
    format: Option<FFprobeFormat>,
}

#[derive(Debug, Deserialize)]
struct FFprobeFormat {
    duration: Option<String>,
}

pub(super) async fn probe_duration_with_ffprobe(
    ffprobe_path: &Path,
    input_path: &Path,
) -> Result<f32, AudioError> {
    let output = Command::new(ffprobe_path)
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            input_path.to_string_lossy().as_ref(),
        ])
        .output()
        .await?;

    if !output.status.success() {
        return Err(AudioError::ExternalToolFailed {
            tool: ffprobe_path.display().to_string(),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }

    let parsed: FFprobeOutput = serde_json::from_slice(&output.stdout)?;
    Ok(parsed
        .format
        .and_then(|format| format.duration)
        .and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(0.0))
}

pub(super) async fn waveform_peaks_with_ffmpeg(
    ffmpeg_path: &Path,
    input_path: &Path,
    bars: usize,
) -> Result<Vec<f32>, AudioError> {
    let temp_path = temp_file_path("pcm");
    let args = vec![
        "-y".to_owned(),
        "-i".to_owned(),
        input_path.to_string_lossy().into_owned(),
        "-ac".to_owned(),
        "1".to_owned(),
        "-ar".to_owned(),
        WAVEFORM_SAMPLE_RATE.to_string(),
        "-f".to_owned(),
        "s16le".to_owned(),
        temp_path.to_string_lossy().into_owned(),
    ];

    let output = Command::new(ffmpeg_path).args(&args).output().await?;

    if !output.status.success() {
        let _ = std::fs::remove_file(&temp_path);
        return Err(AudioError::ExternalToolFailed {
            tool: ffmpeg_path.display().to_string(),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }

    let bytes = std::fs::read(&temp_path)?;
    let _ = std::fs::remove_file(&temp_path);

    if bytes.len() < 2 || bars == 0 {
        return Ok(vec![0.0; bars]);
    }

    let mut peaks = vec![0.0_f32; bars];
    let total_samples = bytes.len() / 2;
    let samples_per_bar = usize::max(1, total_samples.div_ceil(bars));

    for (index, chunk) in bytes.chunks_exact(2).enumerate() {
        // [294A-5] i16::MIN.abs() overflowa (panic en debug, UB conceptual en
        // release). unsigned_abs() devuelve u16 y cubre todo el rango.
        let raw = i16::from_le_bytes([chunk[0], chunk[1]]);
        let amplitude = f32::from(raw.unsigned_abs()) / f32::from(i16::MAX);
        let bucket = usize::min(index / samples_per_bar, bars - 1);
        peaks[bucket] = peaks[bucket].max(amplitude.min(1.0_f32));
    }

    Ok(peaks)
}

pub(super) fn temp_file_path(extension: &str) -> PathBuf {
    let mut path = env::temp_dir();
    path.push(format!(
        "kamples-audio-{}.{}",
        Uuid::new_v4(),
        extension.trim_start_matches('.')
    ));
    path
}
