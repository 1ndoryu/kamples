use axum::body::{to_bytes, Body};
use axum::extract::{FromRequest, Multipart, Request};
use axum::http::header::CONTENT_TYPE;
use chrono::Datelike;
use std::str::FromStr;
use uuid::Uuid;

use crate::errors::AppError;
use crate::repositories::DirectMessageKind;
use crate::AppState;

use super::{
    CreateMessageJsonRequest, MAX_AUDIO_UPLOAD_BYTES, MAX_IMAGE_UPLOAD_BYTES, MAX_JSON_BODY_BYTES,
    MAX_MESSAGE_CHARS,
};

#[derive(Debug)]
pub struct ParsedCreateMessage {
    pub contenido: String,
    pub kind: DirectMessageKind,
    pub sample_id: Option<i32>,
    pub media: Option<UploadedMedia>,
}

#[derive(Debug)]
pub struct UploadedMedia {
    pub bytes: Vec<u8>,
    pub content_type: String,
    pub original_filename: Option<String>,
    pub extension: String,
    pub kind: DirectMessageKind,
}

pub async fn parse_create_message_request(
    request: Request,
    state: &AppState,
) -> Result<ParsedCreateMessage, AppError> {
    let content_type = request
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();

    if content_type.starts_with("multipart/form-data") {
        let multipart = Multipart::from_request(request, state)
            .await
            .map_err(|error| AppError::BadRequest(format!("multipart inválido: {error}")))?;
        parse_multipart_message(multipart).await
    } else {
        parse_json_message(request.into_body()).await
    }
}

pub async fn parse_json_message(body: Body) -> Result<ParsedCreateMessage, AppError> {
    let bytes = to_bytes(body, MAX_JSON_BODY_BYTES)
        .await
        .map_err(|_| AppError::PayloadTooLarge)?;
    let payload: CreateMessageJsonRequest = serde_json::from_slice(&bytes)
        .map_err(|error| AppError::BadRequest(format!("JSON inválido: {error}")))?;
    let kind = payload
        .tipo
        .as_deref()
        .map_or(Ok(DirectMessageKind::Texto), DirectMessageKind::from_str)?;
    validate_message_shape(kind, payload.sample_id, false)?;

    Ok(ParsedCreateMessage {
        contenido: normalize_content(
            &payload.contenido,
            matches!(kind, DirectMessageKind::Sample),
        )?,
        kind,
        sample_id: payload.sample_id,
        media: None,
    })
}

pub async fn parse_multipart_message(
    mut multipart: Multipart,
) -> Result<ParsedCreateMessage, AppError> {
    let mut contenido = String::new();
    let mut declared_kind = None;
    let mut sample_id = None;
    let mut media = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| AppError::BadRequest(format!("multipart inválido: {error}")))?
    {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "contenido" => {
                contenido = field.text().await.map_err(|error| {
                    AppError::BadRequest(format!("contenido inválido: {error}"))
                })?;
            }
            "tipo" => {
                let raw = field
                    .text()
                    .await
                    .map_err(|error| AppError::BadRequest(format!("tipo inválido: {error}")))?;
                if !raw.trim().is_empty() {
                    declared_kind = Some(DirectMessageKind::from_str(raw.trim())?);
                }
            }
            "sample_id" | "sampleId" => {
                sample_id = parse_optional_i32_field(field, "sample_id").await?;
            }
            "media" => {
                if media.is_some() {
                    return Err(AppError::BadRequest(
                        "solo se permite un archivo media por mensaje".into(),
                    ));
                }

                media = Some(parse_uploaded_media(field, declared_kind).await?);
            }
            _ => {
                let _ = field.bytes().await;
            }
        }
    }

    let inferred_kind = media
        .as_ref()
        .map(|uploaded| uploaded.kind)
        .or(declared_kind)
        .unwrap_or(DirectMessageKind::Texto);
    validate_message_shape(inferred_kind, sample_id, media.is_some())?;

    Ok(ParsedCreateMessage {
        contenido: normalize_content(&contenido, inferred_kind != DirectMessageKind::Texto)?,
        kind: inferred_kind,
        sample_id,
        media,
    })
}

pub fn normalize_content(raw: &str, allow_empty: bool) -> Result<String, AppError> {
    let contenido = raw.trim().to_string();
    if contenido.is_empty() && !allow_empty {
        return Err(AppError::Validation(
            "el mensaje no puede estar vacío".into(),
        ));
    }
    if contenido.chars().count() > MAX_MESSAGE_CHARS {
        return Err(AppError::Validation(format!(
            "el mensaje excede {MAX_MESSAGE_CHARS} caracteres"
        )));
    }
    Ok(contenido)
}

fn validate_message_shape(
    kind: DirectMessageKind,
    sample_id: Option<i32>,
    has_media: bool,
) -> Result<(), AppError> {
    match kind {
        DirectMessageKind::Texto => {
            if sample_id.is_some() || has_media {
                return Err(AppError::Validation(
                    "un mensaje de texto no admite sample_id ni media".into(),
                ));
            }
        }
        DirectMessageKind::Imagen | DirectMessageKind::Audio => {
            if !has_media {
                return Err(AppError::Validation(
                    "los mensajes de imagen/audio requieren media adjunta".into(),
                ));
            }
            if sample_id.is_some() {
                return Err(AppError::Validation(
                    "sample_id no aplica a mensajes con media".into(),
                ));
            }
        }
        DirectMessageKind::Sample => {
            if has_media {
                return Err(AppError::Validation(
                    "los mensajes de sample no aceptan archivo media".into(),
                ));
            }
            if sample_id.is_none() {
                return Err(AppError::Validation(
                    "sample_id es obligatorio cuando tipo=sample".into(),
                ));
            }
        }
    }

    Ok(())
}

async fn parse_optional_i32_field(
    field: axum::extract::multipart::Field<'_>,
    field_name: &str,
) -> Result<Option<i32>, AppError> {
    let raw = field
        .text()
        .await
        .map_err(|error| AppError::BadRequest(format!("{field_name} inválido: {error}")))?;
    if raw.trim().is_empty() {
        return Ok(None);
    }

    Ok(Some(raw.trim().parse::<i32>().map_err(|_| {
        AppError::BadRequest(format!("{field_name} debe ser entero"))
    })?))
}

async fn parse_uploaded_media(
    field: axum::extract::multipart::Field<'_>,
    declared_kind: Option<DirectMessageKind>,
) -> Result<UploadedMedia, AppError> {
    let content_type = field
        .content_type()
        .map_or_else(|| "application/octet-stream".to_string(), str::to_owned);
    let original_filename = field.file_name().map(str::to_owned);
    let (detected_kind, extension) =
        detect_media_kind(&content_type, original_filename.as_deref())?;
    let max_bytes = match detected_kind {
        DirectMessageKind::Imagen => MAX_IMAGE_UPLOAD_BYTES,
        DirectMessageKind::Audio => MAX_AUDIO_UPLOAD_BYTES,
        DirectMessageKind::Texto | DirectMessageKind::Sample => {
            return Err(AppError::UnsupportedMediaType(
                "solo se admiten imagenes y audio en media".into(),
            ));
        }
    };
    let bytes = field
        .bytes()
        .await
        .map_err(|error| AppError::BadRequest(format!("media inválida: {error}")))?;
    if bytes.len() > max_bytes {
        return Err(AppError::PayloadTooLarge);
    }
    if let Some(kind) = declared_kind {
        if kind != detected_kind {
            return Err(AppError::Validation(
                "tipo no coincide con el archivo adjunto".into(),
            ));
        }
    }

    Ok(UploadedMedia {
        bytes: bytes.to_vec(),
        content_type,
        original_filename,
        extension,
        kind: detected_kind,
    })
}

pub fn detect_media_kind(
    content_type: &str,
    original_filename: Option<&str>,
) -> Result<(DirectMessageKind, String), AppError> {
    let normalized = content_type.trim().to_ascii_lowercase();
    let extension = infer_extension(&normalized, original_filename).ok_or_else(|| {
        AppError::UnsupportedMediaType(format!("No se pudo inferir extensión para {content_type}"))
    })?;

    if normalized.starts_with("image/") {
        return Ok((DirectMessageKind::Imagen, extension));
    }
    if normalized.starts_with("audio/") {
        return Ok((DirectMessageKind::Audio, extension));
    }

    Err(AppError::UnsupportedMediaType(
        "solo se admiten imágenes y audio en mensajes".into(),
    ))
}

fn infer_extension(content_type: &str, original_filename: Option<&str>) -> Option<String> {
    let from_name = original_filename
        .and_then(|name| {
            std::path::Path::new(name)
                .extension()
                .and_then(|ext| ext.to_str())
        })
        .map(|ext| ext.trim().to_ascii_lowercase())
        .filter(|ext| !ext.is_empty());
    if from_name.is_some() {
        return from_name;
    }

    mime_guess::get_mime_extensions_str(content_type)
        .and_then(|extensions| extensions.first().copied())
        .map(str::to_string)
}

pub fn build_message_storage_key(user_id: i32, extension: &str) -> String {
    let now = chrono::Utc::now();
    format!(
        "messages/{user_id}/{:04}/{:02}/{}.{}",
        now.year(),
        now.month(),
        Uuid::new_v4(),
        extension
    )
}

#[cfg(test)]
mod tests {
    use super::{detect_media_kind, normalize_content};

    #[test]
    fn normalize_rejects_empty_without_media() {
        assert!(normalize_content("   ", false).is_err());
        assert_eq!(normalize_content("   ", true).expect("allow empty"), "");
    }

    #[test]
    fn detects_audio_and_image_media() {
        let (image_kind, image_ext) =
            detect_media_kind("image/png", Some("cover.png")).expect("image");
        assert_eq!(image_kind.as_db_str(), "imagen");
        assert_eq!(image_ext, "png");

        let (audio_kind, audio_ext) =
            detect_media_kind("audio/mpeg", Some("take.mp3")).expect("audio");
        assert_eq!(audio_kind.as_db_str(), "audio");
        assert_eq!(audio_ext, "mp3");
    }
}
