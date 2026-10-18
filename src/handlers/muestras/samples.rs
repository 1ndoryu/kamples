use axum::body::{to_bytes, Body};
use axum::extract::{DefaultBodyLimit, Multipart, Request, State};
use axum::http::{header::CONTENT_TYPE, HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use chrono::Datelike;
use futures::StreamExt;
use sha2::{Digest, Sha256};
use slug::slugify;

use crate::errors::AppError;
#[allow(unused_imports)]
use crate::errors::ErrorResponse;
use crate::middleware::CurrentUser;
use crate::models::SyncChangelogTipo;
#[allow(unused_imports)]
use crate::models::UploadSampleRequestDoc;
use crate::models::{CheckDuplicateRequest, CheckDuplicateResponse, UploadSampleResponse};
use crate::repositories::{
    CreateUploadSampleParams, ProfileRepository, SampleRepository, SyncChangelogRepository,
};
use crate::services::IdempotencyStore;
use crate::AppState;

mod duplicados;
mod subida;
mod multipart;
mod formatos;

pub use duplicados::*;
pub use subida::*;
use multipart::*;
use formatos::*;

const MAX_PORTADA_BYTES: usize = 5 * 1024 * 1024; // 5 MB

const MAX_JSON_HASH_BODY_BYTES: usize = 8 * 1024;
const MAX_HASH_STREAM_BYTES: u64 = 256 * 1024 * 1024;
const MAX_AUDIO_UPLOAD_BYTES: usize = 50 * 1024 * 1024;
const MAX_SAMPLE_MULTIPART_BODY_BYTES: usize = 64 * 1024 * 1024;
const IDEMPOTENCY_TTL_SECS: u64 = 60 * 60;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/samples/check-duplicate", post(check_duplicate))
        .route(
            "/samples/upload",
            post(upload).layer(DefaultBodyLimit::max(MAX_SAMPLE_MULTIPART_BODY_BYTES)),
        )
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub(super) struct ParsedUpload {
    original_filename: String,
    titulo: String,
    contenido: String,
    tags: Vec<String>,
    permitir_descarga: bool,
    licencia_libre: bool,
    es_premium: bool,
    mostrar_en_comunidad: bool,
    sync_upload: bool,
    origen_subida: Option<String>,
    precio: Option<f64>,
    formato: AudioUploadFormat,
    audio_bytes: Vec<u8>,
    /* [166A-6] Campos relacionales y portada para el flujo "subir sample de esta cancion". */
    cancion_origen_id: Option<i32>,
    relacion_id: Option<i32>,
    lado_relacion: Option<String>,
    inicio_segundos: Option<i32>,
    tipo_elemento: Option<String>,
    portada_bytes: Option<Vec<u8>>,
    portada_filename: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum AudioUploadFormat {
    Wav,
    Mp3,
    Flac,
    Aiff,
    Ogg,
}

impl AudioUploadFormat {
    fn extension(self) -> &'static str {
        match self {
            Self::Wav => "wav",
            Self::Mp3 => "mp3",
            Self::Flac => "flac",
            Self::Aiff => "aiff",
            Self::Ogg => "ogg",
        }
    }

    fn mime(self) -> &'static str {
        match self {
            Self::Wav => "audio/wav",
            Self::Mp3 => "audio/mpeg",
            Self::Flac => "audio/flac",
            Self::Aiff => "audio/aiff",
            Self::Ogg => "audio/ogg",
        }
    }
}
