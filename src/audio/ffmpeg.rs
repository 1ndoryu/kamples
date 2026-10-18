use std::path::{Path, PathBuf};

use symphonia::core::errors::Error as SymphoniaError;
use thiserror::Error;

#[path = "ffmpeg/cli.rs"]
mod cli;
#[path = "ffmpeg/conversion.rs"]
mod conversion;
#[path = "ffmpeg/detection.rs"]
mod detection;
#[path = "ffmpeg/symphonia_fallback.rs"]
mod symphonia_fallback;

pub use conversion::{convert_to_flac, convert_to_mp3, cut_to_mp3};

use cli::{probe_duration_with_ffprobe, waveform_peaks_with_ffmpeg};
use symphonia_fallback::{probe_with_symphonia, waveform_peaks_with_symphonia};

pub const DEFAULT_WAVEFORM_BARS: usize = 120;

/* [174A-30] Capa híbrida de audio para Fase 4.
 * Usa ffprobe/ffmpeg CLI cuando están disponibles y cae en Symphonia para
 * mantener el pipeline operativo en desarrollo o CI sin dependencias C.
 * Gotcha: la conversión a MP3/FLAC sí requiere ffmpeg; metadata y waveform
 * pueden resolverse con Symphonia como fallback determinista. */

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FFmpegBinaries {
    ffmpeg: Option<PathBuf>,
    ffprobe: Option<PathBuf>,
}

impl FFmpegBinaries {
    #[must_use]
    pub fn new(ffmpeg: Option<PathBuf>, ffprobe: Option<PathBuf>) -> Self {
        Self { ffmpeg, ffprobe }
    }

    #[must_use]
    pub fn detect() -> Self {
        Self {
            ffmpeg: detection::detect_binary("ffmpeg"),
            ffprobe: detection::detect_binary("ffprobe"),
        }
    }

    #[must_use]
    pub fn ffmpeg(&self) -> Option<&Path> {
        self.ffmpeg.as_deref()
    }

    #[must_use]
    pub fn ffprobe(&self) -> Option<&Path> {
        self.ffprobe.as_deref()
    }

    #[must_use]
    pub fn has_ffmpeg(&self) -> bool {
        self.ffmpeg.is_some()
    }

    #[must_use]
    pub fn has_ffprobe(&self) -> bool {
        self.ffprobe.is_some()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioMetadata {
    pub format: String,
    pub duration_seconds: f32,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub file_size_bytes: u64,
    pub waveform_peaks: Vec<f32>,
}

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("No se encontró el archivo de audio: {0}")]
    MissingFile(PathBuf),
    #[error("No se encontró ffmpeg para la operación solicitada")]
    FfmpegNotFound,
    #[error("No se encontró una pista de audio por defecto en {0}")]
    MissingDefaultTrack(PathBuf),
    #[error("La pista de audio no expone sample_rate en {0}")]
    MissingSampleRate(PathBuf),
    #[error("La pista de audio no expone canales en {0}")]
    MissingChannels(PathBuf),
    #[error("No fue posible detectar el formato del audio en {0}")]
    MissingFormat(PathBuf),
    #[error("{tool} falló con código {status:?}: {stderr}")]
    ExternalToolFailed {
        tool: String,
        status: Option<i32>,
        stderr: String,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("Symphonia devolvió un error: {0}")]
    Symphonia(String),
}

impl From<SymphoniaError> for AudioError {
    fn from(value: SymphoniaError) -> Self {
        Self::Symphonia(value.to_string())
    }
}

pub async fn inspect_audio_file(
    input_path: &Path,
    format_hint: Option<&str>,
    binaries: Option<&FFmpegBinaries>,
) -> Result<AudioMetadata, AudioError> {
    ensure_input_exists(input_path)?;

    let binaries = binaries.cloned().unwrap_or_else(FFmpegBinaries::detect);
    let format = detect_audio_format(input_path, format_hint)?;
    let file_size_bytes = std::fs::metadata(input_path)?.len();

    /* [294A-5] Symphonia decode/probe es CPU-bound y bloquea el runtime
     * tokio cuando se invoca directamente desde un async fn. Eso causa
     * starvation del reactor: ffprobe/ffmpeg lanzados antes/despues fallan
     * con "task was cancelled" porque sus tareas I/O no avanzan.
     * Solucion: ejecutar Symphonia siempre en spawn_blocking. */
    let mut symphonia_probe = {
        let path = input_path.to_path_buf();
        let hint = format_hint.map(str::to_owned);
        tokio::task::spawn_blocking(move || probe_with_symphonia(&path, hint.as_deref()))
            .await
            .map_err(|error| AudioError::Symphonia(format!("join probe: {error}")))??
    };

    if let Some(ffprobe_path) = binaries.ffprobe() {
        match probe_duration_with_ffprobe(ffprobe_path, input_path).await {
            Ok(duration_seconds) if duration_seconds > 0.0 => {
                symphonia_probe.duration_seconds = duration_seconds;
            }
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(path = %input_path.display(), ?error, "ffprobe no pudo calcular duración; se usa Symphonia");
            }
        }
    }

    let waveform_peaks = if let Some(ffmpeg_path) = binaries.ffmpeg() {
        match waveform_peaks_with_ffmpeg(ffmpeg_path, input_path, DEFAULT_WAVEFORM_BARS).await {
            Ok(peaks) => peaks,
            Err(error) => {
                tracing::warn!(path = %input_path.display(), ?error, "ffmpeg no pudo generar waveform; se usa Symphonia");
                let path = input_path.to_path_buf();
                let hint = format_hint.map(str::to_owned);
                tokio::task::spawn_blocking(move || {
                    waveform_peaks_with_symphonia(&path, hint.as_deref(), DEFAULT_WAVEFORM_BARS)
                })
                .await
                .map_err(|error| AudioError::Symphonia(format!("join waveform: {error}")))??
            }
        }
    } else {
        let path = input_path.to_path_buf();
        let hint = format_hint.map(str::to_owned);
        tokio::task::spawn_blocking(move || {
            waveform_peaks_with_symphonia(&path, hint.as_deref(), DEFAULT_WAVEFORM_BARS)
        })
        .await
        .map_err(|error| AudioError::Symphonia(format!("join waveform: {error}")))??
    };

    Ok(AudioMetadata {
        format,
        duration_seconds: symphonia_probe.duration_seconds,
        sample_rate_hz: symphonia_probe.sample_rate_hz,
        channels: symphonia_probe.channels,
        file_size_bytes,
        waveform_peaks,
    })
}

fn ensure_input_exists(input_path: &Path) -> Result<(), AudioError> {
    if input_path.is_file() {
        return Ok(());
    }

    Err(AudioError::MissingFile(input_path.to_path_buf()))
}

fn detect_audio_format(input_path: &Path, format_hint: Option<&str>) -> Result<String, AudioError> {
    format_hint
        .and_then(normalize_audio_format)
        .or_else(|| {
            input_path
                .extension()
                .and_then(|value| value.to_str())
                .and_then(normalize_audio_format)
        })
        .or_else(|| {
            mime_guess::from_path(input_path)
                .first_raw()
                .and_then(normalize_audio_format)
        })
        .ok_or_else(|| AudioError::MissingFormat(input_path.to_path_buf()))
}

fn normalize_audio_format(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    let normalized = normalized.strip_prefix("audio/").unwrap_or(&normalized);

    match normalized {
        "wav" | "wave" | "x-wav" | "vnd.wave" => Some("wav".to_owned()),
        "mp3" | "mpeg" => Some("mp3".to_owned()),
        "flac" | "x-flac" => Some("flac".to_owned()),
        "ogg" | "oga" | "vorbis" | "opus" => Some("ogg".to_owned()),
        "aif" | "aiff" => Some("aiff".to_owned()),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
