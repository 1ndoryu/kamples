/* [174A-30] Fallback con Symphonia: duración, formato y waveform cuando
 * ffprobe/ffmpeg no están disponibles. Es CPU-bound: los llamadores lo
 * ejecutan en spawn_blocking. */

use std::fs::File;
use std::io::ErrorKind;
use std::path::Path;
use std::time::Duration;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use super::{normalize_audio_format, AudioError};

#[derive(Debug)]
pub(super) struct SymphoniaProbe {
    pub(super) sample_rate_hz: u32,
    pub(super) channels: u16,
    pub(super) duration_seconds: f32,
}

struct AudioReader {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    sample_rate_hz: u32,
    channels: usize,
    estimated_frames: Option<u64>,
}

pub(super) fn probe_with_symphonia(
    input_path: &Path,
    format_hint: Option<&str>,
) -> Result<SymphoniaProbe, AudioError> {
    let mut reader = open_audio_reader(input_path, format_hint)?;
    let total_frames = reader
        .estimated_frames
        .unwrap_or(count_frames(&mut reader)?);

    let duration_seconds = if total_frames > 0 {
        duration_from_frames(total_frames, reader.sample_rate_hz).as_secs_f32()
    } else {
        0.0
    };

    Ok(SymphoniaProbe {
        sample_rate_hz: reader.sample_rate_hz,
        channels: u16::try_from(reader.channels).unwrap_or(u16::MAX),
        duration_seconds,
    })
}

pub(super) fn waveform_peaks_with_symphonia(
    input_path: &Path,
    format_hint: Option<&str>,
    bars: usize,
) -> Result<Vec<f32>, AudioError> {
    let mut reader = open_audio_reader(input_path, format_hint)?;
    let total_frames = reader
        .estimated_frames
        .unwrap_or(count_frames(&mut reader)?);

    if total_frames == 0 || bars == 0 {
        return Ok(vec![0.0; bars]);
    }

    let mut reader = open_audio_reader(input_path, format_hint)?;
    let frames_per_bar = total_frames.div_ceil(u64::try_from(bars).unwrap_or(1));
    let mut peaks = vec![0.0_f32; bars];
    let mut frame_index = 0_u64;
    let mut sample_buffer: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match reader.format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                break
            }
            Err(SymphoniaError::ResetRequired) => {
                return Err(AudioError::Symphonia(
                    "Symphonia pidió reset del decoder".to_owned(),
                ));
            }
            Err(error) => return Err(error.into()),
        };

        if packet.track_id() != reader.track_id {
            continue;
        }

        match reader.decoder.decode(&packet) {
            Ok(decoded) => {
                let buffer = sample_buffer.get_or_insert_with(|| {
                    SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec())
                });

                if buffer.capacity() < decoded.capacity() {
                    *buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
                }

                buffer.copy_interleaved_ref(decoded);

                for frame in buffer.samples().chunks(reader.channels) {
                    let amplitude = frame
                        .iter()
                        .fold(0.0_f32, |max_value, sample| max_value.max(sample.abs()));
                    let bucket = usize::try_from(frame_index / frames_per_bar)
                        .unwrap_or(bars - 1)
                        .min(bars - 1);
                    peaks[bucket] = peaks[bucket].max(amplitude.min(1.0_f32));
                    frame_index += 1;
                }
            }
            Err(SymphoniaError::DecodeError(error)) => {
                tracing::warn!(path = %input_path.display(), error, "packet corrupto al generar waveform con Symphonia");
            }
            Err(SymphoniaError::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                break
            }
            Err(error) => return Err(error.into()),
        }
    }

    Ok(peaks)
}

fn duration_from_frames(total_frames: u64, sample_rate_hz: u32) -> Duration {
    let sample_rate_hz = u64::from(sample_rate_hz.max(1));
    let seconds = total_frames / sample_rate_hz;
    let remainder_frames = total_frames % sample_rate_hz;
    let nanos = (u128::from(remainder_frames) * 1_000_000_000_u128) / u128::from(sample_rate_hz);

    Duration::new(seconds, u32::try_from(nanos).unwrap_or(u32::MAX))
}

fn open_audio_reader(
    input_path: &Path,
    format_hint: Option<&str>,
) -> Result<AudioReader, AudioError> {
    let file = File::open(input_path)?;
    let source = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());
    let mut hint = Hint::new();

    if let Some(extension) = input_path.extension().and_then(|value| value.to_str()) {
        hint.with_extension(extension);
    }
    if let Some(format_hint) = format_hint.and_then(normalize_audio_format) {
        hint.with_extension(&format_hint);
    }

    let probed = symphonia::default::get_probe().format(
        &hint,
        source,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;
    let format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| AudioError::MissingDefaultTrack(input_path.to_path_buf()))?;
    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let sample_rate_hz = codec_params
        .sample_rate
        .ok_or_else(|| AudioError::MissingSampleRate(input_path.to_path_buf()))?;
    let channels = codec_params
        .channels
        .map(symphonia::core::audio::Channels::count)
        .ok_or_else(|| AudioError::MissingChannels(input_path.to_path_buf()))?;
    let estimated_frames = codec_params.n_frames;
    let decoder =
        symphonia::default::get_codecs().make(&codec_params, &DecoderOptions::default())?;

    Ok(AudioReader {
        format,
        decoder,
        track_id,
        sample_rate_hz,
        channels,
        estimated_frames,
    })
}

fn count_frames(reader: &mut AudioReader) -> Result<u64, AudioError> {
    let mut total_frames = 0_u64;

    loop {
        let packet = match reader.format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                break
            }
            Err(SymphoniaError::ResetRequired) => {
                return Err(AudioError::Symphonia(
                    "Symphonia pidió reset del decoder".to_owned(),
                ));
            }
            Err(error) => return Err(error.into()),
        };

        if packet.track_id() != reader.track_id {
            continue;
        }

        match reader.decoder.decode(&packet) {
            Ok(decoded) => {
                total_frames += decoded.frames() as u64;
            }
            Err(SymphoniaError::DecodeError(error)) => {
                tracing::warn!(error, "packet corrupto al contar frames con Symphonia");
            }
            Err(SymphoniaError::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof => {
                break
            }
            Err(error) => return Err(error.into()),
        }
    }

    Ok(total_frames)
}
