use axum::extract::Multipart;
use chrono::{Datelike, Utc};
use nanoid::nanoid;

use crate::errors::AppError;
use crate::middleware::CurrentUser;
use crate::models::UpdateArticleRequest;
use crate::repositories::{ArticleEmbed, ArticleMeta, ArticleRepository, UpdateArticleParams};
use crate::AppState;

const DEFAULT_PAGE: i64 = 1;
const DEFAULT_PER_PAGE: i64 = 20;
const DEFAULT_MY_ARTICLES_PER_PAGE: i64 = 20;
pub(super) const MAX_PER_PAGE: i64 = 50;
const MIN_TITLE_LEN: usize = 5;
const MAX_TITLE_LEN: usize = 300;
const MIN_CONTENT_LEN: usize = 50;
const MAX_EXCERPT_LEN: usize = 500;
const MAX_PORTADA_UPLOAD_BYTES: usize = 5 * 1024 * 1024;
const MAX_EMBEDS_LEN: usize = 8 * 1024;
pub(super) const CREATE_RATE_LIMIT_PER_HOUR: i64 = 5;

const VALID_ARTICLE_CATEGORIES: [&str; 26] = [
    "inspiracion",
    "mastering",
    "mezcla",
    "promocion-musical",
    "teoria-musical",
    "grabacion",
    "sampling",
    "diseno-sonoro",
    "herramientas",
    "ableton-live",
    "bitwig-studio",
    "cubase",
    "fl-studio",
    "garageband",
    "logic-pro",
    "pro-tools",
    "studio-one",
    "drops-gratis",
    "midi-gratis",
    "plugins-gratis",
    "presets-gratis",
    "proyectos-gratis",
    "sonidos-gratis",
    "entrevistas",
    "destacados",
    "noticias",
];

const VALID_MODERATION_STATES: [&str; 4] = ["pendiente", "revision", "aprobado", "rechazado"];

#[derive(Debug, Clone)]
pub(super) struct ParsedCreateArticlePayload {
    pub(super) titulo: String,
    pub(super) contenido: String,
    pub(super) extracto: String,
    pub(super) categoria: String,
    pub(super) portada_url: Option<String>,
    pub(super) portada: Option<ParsedImageUpload>,
    pub(super) embeds: Vec<ArticleEmbed>,
    pub(super) descarga_publica: bool,
}

#[derive(Debug, Clone)]
pub(super) struct ParsedImageUpload {
    pub(super) bytes: Vec<u8>,
    pub(super) extension: &'static str,
}

#[derive(Debug, Default)]
struct CreateArticleBuilder {
    titulo: Option<String>,
    contenido: Option<String>,
    extracto: Option<String>,
    categoria: Option<String>,
    portada_url: Option<String>,
    portada: Option<ParsedImageUpload>,
    embeds: Option<Vec<ArticleEmbed>>,
    descarga_publica: bool,
}

pub(super) fn default_page() -> i64 {
    DEFAULT_PAGE
}

pub(super) fn default_per_page() -> i64 {
    DEFAULT_PER_PAGE
}

pub(super) fn default_my_articles_per_page() -> i64 {
    DEFAULT_MY_ARTICLES_PER_PAGE
}

pub(super) fn normalize_category_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| VALID_ARTICLE_CATEGORIES.contains(value))
        .map(ToString::to_string)
}

pub(super) fn normalize_moderation_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| VALID_MODERATION_STATES.contains(value))
        .map(ToString::to_string)
}

pub(super) fn can_manage_article(user: Option<&CurrentUser>, author_id: i32) -> bool {
    user.is_some_and(|user| user.user_id == author_id || user.rol == "admin")
}

pub(super) async fn load_manageable_article_meta(
    state: &AppState,
    user: &CurrentUser,
    article_id: i32,
) -> Result<ArticleMeta, AppError> {
    let meta = ArticleRepository::find_meta_by_id(&state.pool, article_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("articulo {article_id} no encontrado")))?;

    if meta.eliminado_en.is_some() {
        return Err(AppError::NotFound(format!(
            "articulo {article_id} no encontrado"
        )));
    }
    if !can_manage_article(Some(user), meta.autor_id) {
        return Err(AppError::Forbidden(
            "Sin permisos para editar este artículo".to_string(),
        ));
    }

    Ok(meta)
}

pub(super) async fn normalize_update_request(
    state: &AppState,
    article_id: i32,
    body: &UpdateArticleRequest,
) -> Result<UpdateArticleParams, AppError> {
    let mut update = UpdateArticleParams::default();

    if let Some(titulo) = body.titulo.as_deref() {
        update.titulo = Some(validate_title(titulo)?);
        update.slug = Some(
            ArticleRepository::generate_unique_slug(&state.pool, titulo.trim(), Some(article_id))
                .await?,
        );
    }
    if let Some(contenido) = body.contenido.as_deref() {
        update.contenido = Some(validate_content(contenido)?);
    }
    if let Some(extracto) = body.extracto.as_deref() {
        update.extracto = Some(validate_excerpt(extracto)?);
    }
    if let Some(categoria) = body.categoria.as_deref() {
        update.categoria = Some(validate_category(categoria)?);
    }
    if let Some(portada_url) = body.portada_url.as_deref() {
        update.portada_url = Some(portada_url.trim().to_string());
    }
    if let Some(embeds) = body.embeds.as_deref() {
        update.embeds = Some(
            serde_json::to_value(parse_embeds(embeds)?)
                .map_err(|error| AppError::Internal(format!("serializar embeds: {error}")))?,
        );
    }
    if let Some(descarga_publica) = body.descarga_publica {
        update.descarga_publica = Some(descarga_publica);
    }

    if update.titulo.is_none()
        && update.contenido.is_none()
        && update.extracto.is_none()
        && update.categoria.is_none()
        && update.portada_url.is_none()
        && update.embeds.is_none()
        && update.descarga_publica.is_none()
    {
        return Err(AppError::BadRequest("Sin cambios válidos".to_string()));
    }

    Ok(update)
}

pub(super) async fn parse_create_article_multipart(
    mut multipart: Multipart,
) -> Result<ParsedCreateArticlePayload, AppError> {
    let mut builder = CreateArticleBuilder::default();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| AppError::BadRequest(format!("multipart invalido: {error}")))?
    {
        let name = field.name().unwrap_or_default().to_string();
        if name == "portada" {
            handle_portada_field(field, &mut builder.portada).await?;
            continue;
        }

        let value = read_text_field(field, &name).await?;
        apply_text_field(&name, &value, &mut builder)?;
    }

    Ok(ParsedCreateArticlePayload {
        titulo: validate_title(builder.titulo.as_deref().unwrap_or_default())?,
        contenido: validate_content(builder.contenido.as_deref().unwrap_or_default())?,
        extracto: validate_excerpt(builder.extracto.as_deref().unwrap_or_default())?,
        categoria: validate_category(builder.categoria.as_deref().unwrap_or("inspiracion"))?,
        portada_url: builder.portada_url,
        portada: builder.portada,
        embeds: builder.embeds.unwrap_or_default(),
        descarga_publica: builder.descarga_publica,
    })
}

pub(super) async fn resolve_cover_url(
    state: &AppState,
    user_id: i32,
    slug: &str,
    portada_url: Option<String>,
    portada: Option<ParsedImageUpload>,
) -> Result<Option<String>, AppError> {
    if portada_url.is_some() {
        return Ok(portada_url);
    }

    let Some(portada) = portada else {
        return Ok(None);
    };
    let storage_key = build_cover_storage_key(user_id, slug, portada.extension);
    state
        .storage
        .put_bytes(&storage_key, &portada.bytes)
        .await?;
    Ok(Some(build_upload_url(state, &storage_key)))
}

async fn handle_portada_field(
    field: axum::extract::multipart::Field<'_>,
    target: &mut Option<ParsedImageUpload>,
) -> Result<(), AppError> {
    if target.is_some() {
        return Err(AppError::BadRequest(
            "solo se permite una portada".to_string(),
        ));
    }
    let filename = field.file_name().map(ToString::to_string);
    let bytes = collect_multipart_bytes(field, MAX_PORTADA_UPLOAD_BYTES).await?;
    let extension = detect_image_extension(&bytes, filename.as_deref())?;
    *target = Some(ParsedImageUpload { bytes, extension });
    Ok(())
}

async fn read_text_field(
    field: axum::extract::multipart::Field<'_>,
    name: &str,
) -> Result<String, AppError> {
    field
        .text()
        .await
        .map_err(|error| AppError::BadRequest(format!("{name} invalido: {error}")))
}

fn apply_text_field(
    name: &str,
    raw_value: &str,
    builder: &mut CreateArticleBuilder,
) -> Result<(), AppError> {
    let value = raw_value.trim().to_string();
    match name {
        "titulo" => builder.titulo = Some(value),
        "contenido" => builder.contenido = Some(value),
        "extracto" => builder.extracto = Some(value),
        "categoria" => builder.categoria = Some(value),
        "portada_url" if !value.is_empty() => {
            builder.portada_url = Some(value);
        }
        "embeds" => builder.embeds = Some(parse_embeds(raw_value)?),
        "descarga_publica" => {
            builder.descarga_publica = parse_bool_field(raw_value, false)?;
        }
        _ => {}
    }
    Ok(())
}

async fn collect_multipart_bytes(
    mut field: axum::extract::multipart::Field<'_>,
    max_bytes: usize,
) -> Result<Vec<u8>, AppError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = field
        .chunk()
        .await
        .map_err(|error| AppError::BadRequest(format!("chunk multipart invalido: {error}")))?
    {
        if bytes.len().saturating_add(chunk.len()) > max_bytes {
            return Err(AppError::PayloadTooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.is_empty() {
        return Err(AppError::BadRequest("La portada esta vacia".to_string()));
    }
    Ok(bytes)
}

fn parse_bool_field(raw: &str, default: bool) -> Result<bool, AppError> {
    let value = raw.trim();
    if value.is_empty() {
        return Ok(default);
    }
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "on" | "yes" => Ok(true),
        "0" | "false" | "off" | "no" => Ok(false),
        _ => Err(AppError::BadRequest(format!("boolean invalido: {raw}"))),
    }
}

fn detect_image_extension(bytes: &[u8], filename: Option<&str>) -> Result<&'static str, AppError> {
    let extension = filename
        .and_then(|value| std::path::Path::new(value).extension())
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();

    if bytes.len() >= 3
        && bytes[0..3] == [0xFF, 0xD8, 0xFF]
        && matches!(extension.as_str(), "" | "jpg" | "jpeg")
    {
        return Ok("jpg");
    }
    if bytes.len() >= 8
        && bytes[0..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
        && matches!(extension.as_str(), "" | "png")
    {
        return Ok("png");
    }
    if bytes.len() >= 12
        && &bytes[0..4] == b"RIFF"
        && &bytes[8..12] == b"WEBP"
        && matches!(extension.as_str(), "" | "webp")
    {
        return Ok("webp");
    }

    Err(AppError::UnsupportedMediaType(
        "Formato de imagen no válido. Formatos aceptados: JPEG, PNG, WEBP".to_string(),
    ))
}

fn parse_embeds(raw: &str) -> Result<Vec<ArticleEmbed>, AppError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if trimmed.len() > MAX_EMBEDS_LEN {
        return Err(AppError::BadRequest("Embeds demasiado largos".to_string()));
    }

    let parsed: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|_| AppError::BadRequest("Formato de embeds inválido".to_string()))?;
    let Some(items) = parsed.as_array() else {
        return Err(AppError::BadRequest(
            "Formato de embeds inválido".to_string(),
        ));
    };

    let mut normalized = Vec::new();
    for item in items {
        let Some(tipo) = item.get("tipo").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let Some(id) = item.get("id").and_then(serde_json::Value::as_i64) else {
            continue;
        };
        if !(tipo == "sample" || tipo == "coleccion") || id <= 0 {
            continue;
        }
        normalized.push(ArticleEmbed {
            tipo: tipo.to_string(),
            id: i32::try_from(id).unwrap_or(i32::MAX),
            descarga_publica: item
                .get("descargaPublica")
                .and_then(serde_json::Value::as_bool)
                .or_else(|| {
                    item.get("descarga_publica")
                        .and_then(serde_json::Value::as_bool)
                }),
        });
    }

    Ok(normalized)
}

fn build_cover_storage_key(user_id: i32, slug: &str, extension: &str) -> String {
    let now = Utc::now();
    format!(
        "articulos/{}/{:04}/{:02}/{}-{}.{}",
        user_id,
        now.year(),
        now.month(),
        slug,
        nanoid!(8),
        extension,
    )
}

fn build_upload_url(state: &AppState, storage_key: &str) -> String {
    if let Some(base) = &state.public_base_url {
        format!("{}/uploads/{}", base.trim_end_matches('/'), storage_key)
    } else {
        format!("/uploads/{storage_key}")
    }
}

fn validate_title(raw: &str) -> Result<String, AppError> {
    let title = raw.trim();
    let len = title.chars().count();
    if len < MIN_TITLE_LEN {
        return Err(AppError::BadRequest(
            "El título debe tener al menos 5 caracteres".to_string(),
        ));
    }
    if len > MAX_TITLE_LEN {
        return Err(AppError::BadRequest(
            "El título no puede superar 300 caracteres".to_string(),
        ));
    }
    Ok(title.to_string())
}

fn validate_content(raw: &str) -> Result<String, AppError> {
    let content = raw.trim();
    if content.chars().count() < MIN_CONTENT_LEN {
        return Err(AppError::BadRequest(
            "El contenido debe tener al menos 50 caracteres".to_string(),
        ));
    }
    Ok(content.to_string())
}

fn validate_excerpt(raw: &str) -> Result<String, AppError> {
    let excerpt = raw.trim();
    if excerpt.is_empty() {
        return Err(AppError::BadRequest(
            "El extracto es obligatorio".to_string(),
        ));
    }
    if excerpt.chars().count() > MAX_EXCERPT_LEN {
        return Err(AppError::BadRequest(
            "El extracto no puede superar 500 caracteres".to_string(),
        ));
    }
    Ok(excerpt.to_string())
}

fn validate_category(value: &str) -> Result<String, AppError> {
    let category = value.trim();
    if VALID_ARTICLE_CATEGORIES.contains(&category) {
        Ok(category.to_string())
    } else {
        Err(AppError::BadRequest("Categoría no válida".to_string()))
    }
}
