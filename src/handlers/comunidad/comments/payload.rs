use axum::body::{to_bytes, Body};
use axum::extract::{FromRequest, Multipart, Request};
use axum::http::header::CONTENT_TYPE;
use chrono::Datelike;
use std::str::FromStr;
use uuid::Uuid;

use crate::errors::AppError;
use crate::repositories::CommentContentKind;
use crate::AppState;

use super::{
    CreateCommentJsonRequest, MAX_AUDIO_UPLOAD_BYTES, MAX_COMMENT_CHARS, MAX_IMAGE_UPLOAD_BYTES,
    MAX_JSON_BODY_BYTES,
};

#[derive(Debug)]
pub struct ParsedCreateComment {
    pub contenido: String,
    pub parent_id: Option<i32>,
    pub media: Option<UploadedMedia>,
}

#[derive(Debug)]
pub struct UploadedMedia {
    pub bytes: Vec<u8>,
    pub content_type: String,
    pub original_filename: Option<String>,
    pub extension: String,
    pub kind: CommentContentKind,
}

pub async fn parse_create_comment_request(
    request: Request,
    state: &AppState,
) -> Result<ParsedCreateComment, AppError> {
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
        parse_multipart_comment(multipart).await
    } else {
        parse_json_comment(request.into_body()).await
    }
}

pub async fn parse_json_comment(body: Body) -> Result<ParsedCreateComment, AppError> {
    let bytes = to_bytes(body, MAX_JSON_BODY_BYTES)
        .await
        .map_err(|_| AppError::PayloadTooLarge)?;
    let payload: CreateCommentJsonRequest = serde_json::from_slice(&bytes)
        .map_err(|error| AppError::BadRequest(format!("JSON inválido: {error}")))?;
    Ok(ParsedCreateComment {
        contenido: normalize_content(&payload.contenido, false)?,
        parent_id: payload.parent_id,
        media: None,
    })
}

pub async fn parse_multipart_comment(
    mut multipart: Multipart,
) -> Result<ParsedCreateComment, AppError> {
    let mut contenido = String::new();
    let mut parent_id = None;
    let mut declared_content_kind = None;
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
            "parent_id" | "parentId" => {
                let raw = field.text().await.map_err(|error| {
                    AppError::BadRequest(format!("parent_id inválido: {error}"))
                })?;
                if !raw.trim().is_empty() {
                    parent_id =
                        Some(raw.trim().parse::<i32>().map_err(|_| {
                            AppError::BadRequest("parent_id debe ser entero".into())
                        })?);
                }
            }
            "tipo_contenido" | "tipoContenido" => {
                let raw = field.text().await.map_err(|error| {
                    AppError::BadRequest(format!("tipo_contenido inválido: {error}"))
                })?;
                if !raw.trim().is_empty() {
                    declared_content_kind = Some(CommentContentKind::from_str(raw.trim())?);
                }
            }
            "media" => {
                if media.is_some() {
                    return Err(AppError::BadRequest(
                        "solo se permite un archivo media por comentario".into(),
                    ));
                }
                let content_type = field
                    .content_type()
                    .map_or_else(|| "application/octet-stream".to_string(), str::to_owned);
                let original_filename = field.file_name().map(str::to_owned);
                let (detected_kind, extension) =
                    detect_media_kind(&content_type, original_filename.as_deref())?;
                let max_bytes = match detected_kind {
                    CommentContentKind::Imagen => MAX_IMAGE_UPLOAD_BYTES,
                    CommentContentKind::Audio => MAX_AUDIO_UPLOAD_BYTES,
                    CommentContentKind::Texto => unreachable!("texto no puede venir con media"),
                };
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|error| AppError::BadRequest(format!("media inválida: {error}")))?;
                if bytes.len() > max_bytes {
                    return Err(AppError::PayloadTooLarge);
                }
                if let Some(kind) = declared_content_kind {
                    if kind != detected_kind {
                        return Err(AppError::Validation(
                            "tipo_contenido no coincide con el archivo adjunto".into(),
                        ));
                    }
                }

                media = Some(UploadedMedia {
                    bytes: bytes.to_vec(),
                    content_type,
                    original_filename,
                    extension,
                    kind: detected_kind,
                });
            }
            _ => {
                let _ = field.bytes().await;
            }
        }
    }

    let has_media = media.is_some();
    if !has_media && declared_content_kind.is_some_and(|kind| kind != CommentContentKind::Texto) {
        return Err(AppError::Validation(
            "tipo_contenido audio/imagen requiere un archivo media".into(),
        ));
    }

    Ok(ParsedCreateComment {
        contenido: normalize_content(&contenido, has_media)?,
        parent_id,
        media,
    })
}

pub fn normalize_content(raw: &str, allow_empty: bool) -> Result<String, AppError> {
    let contenido = raw.trim().to_string();
    if contenido.is_empty() && !allow_empty {
        return Err(AppError::Validation(
            "el comentario no puede estar vacío".into(),
        ));
    }
    if contenido.chars().count() > MAX_COMMENT_CHARS {
        return Err(AppError::Validation(format!(
            "el comentario excede {MAX_COMMENT_CHARS} caracteres"
        )));
    }
    Ok(contenido)
}

pub fn detect_media_kind(
    content_type: &str,
    original_filename: Option<&str>,
) -> Result<(CommentContentKind, String), AppError> {
    let normalized = content_type.trim().to_ascii_lowercase();
    let extension = infer_extension(&normalized, original_filename).ok_or_else(|| {
        AppError::UnsupportedMediaType(format!("No se pudo inferir extensión para {content_type}"))
    })?;

    if normalized.starts_with("image/") {
        return Ok((CommentContentKind::Imagen, extension));
    }
    if normalized.starts_with("audio/") {
        return Ok((CommentContentKind::Audio, extension));
    }

    Err(AppError::UnsupportedMediaType(
        "solo se admiten imágenes y audio en comentarios".into(),
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

pub fn build_comment_storage_key(user_id: i32, extension: &str) -> String {
    let now = chrono::Utc::now();
    format!(
        "comments/{user_id}/{:04}/{:02}/{}.{}",
        now.year(),
        now.month(),
        Uuid::new_v4(),
        extension
    )
}

pub fn extract_storage_key(raw: &str) -> Option<String> {
    let normalized = raw.trim().replace('\\', "/");
    if normalized.is_empty() {
        return None;
    }
    if let Some(index) = normalized.find("/uploads/") {
        return Some(normalized[(index + "/uploads/".len())..].to_string());
    }
    if normalized.starts_with('/') {
        return normalized.strip_prefix("/").map(str::to_string);
    }
    Some(normalized)
}

#[cfg(test)]
mod tests {
    use super::{detect_media_kind, extract_storage_key, normalize_content};

    #[test]
    fn normalize_rejects_empty_without_media() {
        assert!(normalize_content("   ", false).is_err());
        assert_eq!(
            normalize_content("   ", true).expect("empty with media"),
            ""
        );
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

    #[test]
    fn extracts_storage_key_from_public_url() {
        let key = extract_storage_key("https://example.com/uploads/comments/1/2026/04/test.png")
            .expect("storage key");
        assert_eq!(key, "comments/1/2026/04/test.png");
    }
}
