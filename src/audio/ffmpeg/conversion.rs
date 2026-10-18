/* [254A-8c] Conversiones con ffmpeg CLI: MP3 320 kbps, FLAC y recorte por
 * rango. Todas exigen el binario; sin ffmpeg devuelven FfmpegNotFound. */

use std::path::Path;

use tokio::process::Command;

use super::{ensure_input_exists, AudioError, FFmpegBinaries};

const MP3_BITRATE_KBPS: u32 = 320;

pub async fn convert_to_mp3(
    input_path: &Path,
    output_path: &Path,
    binaries: Option<&FFmpegBinaries>,
) -> Result<(), AudioError> {
    ensure_input_exists(input_path)?;

    let binaries = binaries.cloned().unwrap_or_else(FFmpegBinaries::detect);
    let ffmpeg_path = binaries.ffmpeg().ok_or(AudioError::FfmpegNotFound)?;

    run_ffmpeg(
        ffmpeg_path,
        input_path,
        output_path,
        &[
            "-codec:a",
            "libmp3lame",
            "-b:a",
            &format!("{MP3_BITRATE_KBPS}k"),
            "-ar",
            "44100",
        ],
    )
    .await
}

pub async fn convert_to_flac(
    input_path: &Path,
    output_path: &Path,
    binaries: Option<&FFmpegBinaries>,
) -> Result<(), AudioError> {
    ensure_input_exists(input_path)?;

    let binaries = binaries.cloned().unwrap_or_else(FFmpegBinaries::detect);
    let ffmpeg_path = binaries.ffmpeg().ok_or(AudioError::FfmpegNotFound)?;

    run_ffmpeg(ffmpeg_path, input_path, output_path, &["-codec:a", "flac"]).await
}

/* [254A-8c] Recorta un rango [start_sec, start_sec + duration_sec] del audio
 * fuente y lo convierte a MP3 320 kbps mono.
 *
 * `-ss` ANTES de `-i` es la forma rápida (seek antes de decode). Para audio
 * completo descargado esto es correcto y mucho más rápido que leer todo.
 * `-t` fija la duración máxima de salida.
 *
 * Gotcha: si start_sec + duration_sec > duración real del audio, FFmpeg
 * simplemente corta al final sin error — comportamiento deseado. */
pub async fn cut_to_mp3(
    input_path: &Path,
    output_path: &Path,
    start_sec: f64,
    duration_sec: f64,
    binaries: Option<&FFmpegBinaries>,
) -> Result<(), AudioError> {
    ensure_input_exists(input_path)?;

    let binaries = binaries.cloned().unwrap_or_else(FFmpegBinaries::detect);
    let ffmpeg_path = binaries.ffmpeg().ok_or(AudioError::FfmpegNotFound)?;

    let start_str = format!("{start_sec:.6}");
    let duration_str = format!("{duration_sec:.6}");

    // -ss antes de -i = fast seek; los extra_args se insertan DESPUÉS de -i
    // Para eso usamos run_ffmpeg_with_input_seek que pasa -ss al principio
    run_ffmpeg_seek(
        ffmpeg_path,
        input_path,
        output_path,
        &start_str,
        &duration_str,
    )
    .await
}

async fn run_ffmpeg_seek(
    ffmpeg_path: &Path,
    input_path: &Path,
    output_path: &Path,
    start_sec: &str,
    duration_sec: &str,
) -> Result<(), AudioError> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let args = vec![
        "-y".to_owned(),
        "-ss".to_owned(),
        start_sec.to_owned(),
        "-i".to_owned(),
        input_path.to_string_lossy().into_owned(),
        "-t".to_owned(),
        duration_sec.to_owned(),
        "-codec:a".to_owned(),
        "libmp3lame".to_owned(),
        "-b:a".to_owned(),
        format!("{MP3_BITRATE_KBPS}k"),
        "-ar".to_owned(),
        "44100".to_owned(),
        output_path.to_string_lossy().into_owned(),
    ];

    let output = Command::new(ffmpeg_path).args(&args).output().await?;

    if output.status.success() && output_path.is_file() {
        return Ok(());
    }

    Err(AudioError::ExternalToolFailed {
        tool: ffmpeg_path.display().to_string(),
        status: output.status.code(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}

async fn run_ffmpeg(
    ffmpeg_path: &Path,
    input_path: &Path,
    output_path: &Path,
    extra_args: &[&str],
) -> Result<(), AudioError> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut args = vec![
        "-y".to_owned(),
        "-i".to_owned(),
        input_path.to_string_lossy().into_owned(),
    ];
    args.extend(extra_args.iter().map(|value| (*value).to_owned()));
    args.push(output_path.to_string_lossy().into_owned());

    let output = Command::new(ffmpeg_path).args(&args).output().await?;

    if output.status.success() && output_path.is_file() {
        return Ok(());
    }

    Err(AudioError::ExternalToolFailed {
        tool: ffmpeg_path.display().to_string(),
        status: output.status.code(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
    })
}
