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

const MAX_PORTADA_BYTES: usize = 5 * 1024 * 1024; // 5 MB

const MAX_JSON_HASH_BODY_BYTES: usize = 8 * 1024;
const MAX_HASH_STREAM_BYTES: u64 = 256 * 1024 * 1024;
const MAX_AUDIO_UPLOAD_BYTES: usize = 50 * 1024 * 1024;
const MAX_SAMPLE_MULTIPART_BODY_BYTES: usize = 64 * 1024 * 1024;
const IDEMPOTENCY_TTL_SECS: u64 = 60 * 60;

/* [174A-28] Precheck de duplicados para uploads.
 * Acepta dos modos:
 * - `application/json` con `{ audio_hash }` ya calculado por el cliente.
 * - cualquier otro content-type: body binario, calculando SHA-256 en streaming.
 *
 * El endpoint NO crea samples ni bloquea subidas. Solo informa si ya existe un
 * sample visible con el mismo contenido para que desktop/frontend optimicen UX. */

#[utoipa::path(
    post,
    path = "/api/samples/check-duplicate",
    request_body(
        content = CheckDuplicateRequest,
        description = "Acepta JSON { audio_hash } o body binario crudo para calcular SHA-256 en streaming"
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Resultado del precheck de duplicado", body = CheckDuplicateResponse),
        (status = 400, description = "Hash inválido o body vacío", body = ErrorResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 413, description = "Body demasiado grande", body = ErrorResponse)
    )
)]
pub async fn check_duplicate(
    State(state): State<AppState>,
    user: CurrentUser,
    request: Request,
) -> Result<Json<CheckDuplicateResponse>, AppError> {
    let content_type = request
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();

    let (audio_hash, bytes_hashed) = if content_type.starts_with("application/json") {
        parse_precomputed_hash(request.into_body()).await?
    } else {
        hash_body_streaming(request.into_body()).await?
    };

    /* [224A-2] En modo dev (ALLOW_DUPLICATE_UPLOADS=true) siempre retorna
     * possible_duplicate=false — permite re-subir el mismo audio en local
     * sin bloqueos. Nunca habilitar en producción. */
    if state.allow_duplicate_uploads {
        return Ok(Json(CheckDuplicateResponse {
            audio_hash,
            possible_duplicate: false,
            sample_id: None,
            same_owner: None,
            title: None,
            message: None,
            bytes_hashed,
        }));
    }

    let duplicate =
        SampleRepository::find_duplicate_by_audio_hash(&state.pool, &audio_hash).await?;
    let response = if let Some(existing) = duplicate {
        let same_owner = existing.creador_id == user.user_id;
        CheckDuplicateResponse {
            audio_hash,
            possible_duplicate: true,
            sample_id: Some(existing.id),
            same_owner: Some(same_owner),
            title: Some(existing.titulo),
            message: Some(if same_owner {
                "Ya tienes este sample subido. Puedes reutilizar el existente.".to_string()
            } else {
                "Ya existe un sample con el mismo audio. La subida puede continuar para revisión o deduplicación posterior.".to_string()
            }),
            bytes_hashed,
        }
    } else {
        CheckDuplicateResponse {
            audio_hash,
            possible_duplicate: false,
            sample_id: None,
            same_owner: None,
            title: None,
            message: None,
            bytes_hashed,
        }
    };

    Ok(Json(response))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/samples/check-duplicate", post(check_duplicate))
        .route(
            "/samples/upload",
            post(upload).layer(DefaultBodyLimit::max(MAX_SAMPLE_MULTIPART_BODY_BYTES)),
        )
}

#[utoipa::path(
    post,
    path = "/api/samples/upload",
    request_body(
        content = UploadSampleRequestDoc,
        content_type = "multipart/form-data",
        description = "Upload multipart de audio con tags mínimos, flags y clave opcional de idempotencia"
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 201, description = "Sample subido", body = UploadSampleResponse),
        (status = 200, description = "Resultado recuperado por idempotencia", body = UploadSampleResponse),
        (status = 400, description = "Validacion multipart/MIME/tags", body = ErrorResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Cuenta no activa o limite de plan", body = ErrorResponse),
        (status = 413, description = "Archivo demasiado grande", body = ErrorResponse)
    )
)]
/* [174A-29] Upload multipart compatible con desktop/legacy.
 * Conserva las reglas críticas del flujo viejo: idempotencia por header,
 * límite de plan, mínimo 2 tags, validación por extensión + magic bytes y
 * persistencia inmediata del original para que la fase de pipeline continúe. */
pub async fn upload(
    State(state): State<AppState>,
    user: CurrentUser,
    headers: HeaderMap,
    multipart: Multipart,
) -> Result<(StatusCode, Json<UploadSampleResponse>), AppError> {
    let idempotency_key = headers
        .get("X-Idempotency-Key")
        .and_then(|value| value.to_str().ok())
        .map(IdempotencyStore::sanitize_key)
        .transpose()?;

    if let Some(key) = &idempotency_key {
        if let Some(cached) = IdempotencyStore::get_json::<UploadSampleResponse>(
            &state.redis,
            &format!("samples:upload:{}", user.user_id),
            key,
        )
        .await?
        {
            return Ok((StatusCode::OK, Json(cached)));
        }
    }

    validar_cuota_subida(&state, user.user_id).await?;

    let parsed = parse_upload_multipart(multipart).await?;
    let audio_hash = hex::encode(Sha256::digest(&parsed.audio_bytes));
    let nombres = nombres_subida(user.user_id, &parsed.titulo, parsed.formato);
    state
        .storage
        .put_bytes(&nombres.storage_key, &parsed.audio_bytes)
        .await?;

    /* [166A-6] Subir portada si se proporciono. */
    let imagen_url = match &parsed.portada_bytes {
        Some(portada_bytes) => Some(
            subir_portada(
                &state,
                user.user_id,
                &nombres.slug,
                portada_bytes,
                parsed.portada_filename.as_deref(),
            )
            .await?,
        ),
        None => None,
    };

    let metadata = metadata_subida(&parsed);
    let response = crear_sample_subido(
        &state,
        user.user_id,
        &parsed,
        &nombres,
        &audio_hash,
        imagen_url.as_deref(),
        metadata,
    )
    .await?;

    /* [246A-1] Registrar en changelog para delta sync del desktop. */
    SyncChangelogRepository::insert_entry(
        &state.pool,
        user.user_id,
        SyncChangelogTipo::SampleAdded,
        response.sample_id,
        serde_json::json!({"origen": "upload"}),
    )
    .await
    .ok();

    if let Some(key) = &idempotency_key {
        IdempotencyStore::set_json(
            &state.redis,
            &format!("samples:upload:{}", user.user_id),
            key,
            IDEMPOTENCY_TTL_SECS,
            &response,
        )
        .await?;
    }

    Ok((StatusCode::CREATED, Json(response)))
}

/* Comprueba la cuenta y el límite del plan antes de leer el multipart. */
async fn validar_cuota_subida(state: &AppState, user_id: i32) -> Result<(), AppError> {
    let profile = ProfileRepository::find_by_id(&state.pool, user_id)
        .await?
        .ok_or(AppError::NotFound("Usuario".into()))?;
    if profile.estado != "activo" {
        return Err(AppError::Forbidden(
            "La cuenta no esta activa para subir samples".into(),
        ));
    }
    let total_actual = profile.total_samples.unwrap_or(0);
    let limite_max = if matches!(profile.plan.as_str(), "pro" | "premium") {
        20_000
    } else {
        100
    };
    if total_actual >= limite_max {
        return Err(AppError::Forbidden(format!(
            "Limite de samples alcanzado para el plan {} ({limite_max})",
            profile.plan
        )));
    }
    Ok(())
}

struct NombresSubida {
    id_corto: String,
    slug: String,
    storage_key: String,
}

/* Genera id corto, slug y clave de almacenamiento del audio original. */
fn nombres_subida(user_id: i32, titulo: &str, formato: AudioUploadFormat) -> NombresSubida {
    let id_corto = generate_short_id();
    let slug_base = slugify(titulo);
    let slug = format!(
        "{}-{id_corto}",
        if slug_base.is_empty() {
            "sample"
        } else {
            slug_base.as_str()
        }
    );
    let storage_key = build_storage_key(user_id, &slug, formato.extension());
    NombresSubida {
        id_corto,
        slug,
        storage_key,
    }
}

/* Sube la portada junto al audio y devuelve su URL pública. */
async fn subir_portada(
    state: &AppState,
    user_id: i32,
    slug: &str,
    bytes: &[u8],
    filename: Option<&str>,
) -> Result<String, AppError> {
    let ext = filename
        .and_then(|name| std::path::Path::new(name).extension())
        .and_then(|ext| ext.to_str())
        .unwrap_or("jpg");
    let portada_key = format!("samples/{user_id}/portadas/{slug}.{ext}");
    state.storage.put_bytes(&portada_key, bytes).await?;
    Ok(build_upload_url(state, &portada_key))
}

/* Metadata libre del sample; conserva relacion y lado como hacia el legado. */
fn metadata_subida(parsed: &ParsedUpload) -> serde_json::Value {
    let mut metadata = serde_json::json!({
        "filename_original": parsed.original_filename,
        "mime_type": parsed.formato.mime(),
        "sync_upload": parsed.sync_upload,
        "origen_subida": parsed.origen_subida,
    });
    if let Some(relacion_id) = parsed.relacion_id {
        metadata["relacion_id"] = serde_json::json!(relacion_id);
    }
    if let Some(lado_relacion) = &parsed.lado_relacion {
        metadata["lado_extraccion"] = serde_json::json!(lado_relacion);
    }
    metadata
}

/* Persiste el sample; si falla, borra el audio ya subido para no dejar huérfanos. */
async fn crear_sample_subido(
    state: &AppState,
    user_id: i32,
    parsed: &ParsedUpload,
    nombres: &NombresSubida,
    audio_hash: &str,
    imagen_url: Option<&str>,
    metadata: serde_json::Value,
) -> Result<UploadSampleResponse, AppError> {
    let created = match SampleRepository::create_upload_sample(
        &state.pool,
        CreateUploadSampleParams {
            creador_id: user_id,
            titulo: &parsed.titulo,
            slug: &nombres.slug,
            id_corto: &nombres.id_corto,
            descripcion: &parsed.contenido,
            formato: parsed.formato.extension(),
            tamano: i64::try_from(parsed.audio_bytes.len()).unwrap_or(i64::MAX),
            tags: &parsed.tags,
            audio_hash,
            ruta_original: &nombres.storage_key,
            permitir_descarga: parsed.permitir_descarga,
            licencia_libre: parsed.licencia_libre,
            es_premium: parsed.es_premium,
            precio: parsed.precio,
            mostrar_en_comunidad: parsed.mostrar_en_comunidad,
            metadata,
            sync_upload: parsed.sync_upload,
            cancion_origen_id: parsed.cancion_origen_id,
            relacion_sampleo_id: parsed.relacion_id,
            imagen_url,
        },
    )
    .await
    {
        Ok(created) => created,
        Err(error) => {
            let _ = state.storage.delete(&nombres.storage_key).await;
            return Err(AppError::from(error));
        }
    };

    /* [166A-6] Si hay relacion_id + lado_relacion, vincular el sample a la relacion. */
    if let (Some(relacion_id), Some(lado_relacion)) =
        (parsed.relacion_id, parsed.lado_relacion.as_ref())
    {
        SampleRepository::update_relacion_sample_link(
            &state.pool,
            relacion_id,
            created.id,
            lado_relacion,
            parsed.inicio_segundos,
            parsed.tipo_elemento.as_deref(),
        )
        .await?;
    }

    Ok(UploadSampleResponse {
        ok: true,
        sample_id: created.id,
        id_corto: created.id_corto,
        slug: created.slug,
        url: build_upload_url(state, &created.ruta_original),
        estado: created.estado,
    })
}

async fn parse_precomputed_hash(body: Body) -> Result<(String, u64), AppError> {
    let bytes = to_bytes(body, MAX_JSON_HASH_BODY_BYTES)
        .await
        .map_err(|_| AppError::PayloadTooLarge)?;
    let payload: CheckDuplicateRequest = serde_json::from_slice(&bytes)
        .map_err(|error| AppError::BadRequest(format!("JSON invalido: {error}")))?;
    Ok((normalize_sha256_hex(&payload.audio_hash)?, 0))
}

async fn hash_body_streaming(body: Body) -> Result<(String, u64), AppError> {
    let mut stream = body.into_data_stream();
    let mut hasher = Sha256::new();
    let mut bytes_hashed = 0_u64;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result
            .map_err(|error| AppError::BadRequest(format!("body invalido: {error}")))?;
        bytes_hashed = bytes_hashed
            .checked_add(u64::try_from(chunk.len()).unwrap_or(u64::MAX))
            .ok_or(AppError::PayloadTooLarge)?;
        if bytes_hashed > MAX_HASH_STREAM_BYTES {
            return Err(AppError::PayloadTooLarge);
        }
        hasher.update(&chunk);
    }

    if bytes_hashed == 0 {
        return Err(AppError::BadRequest(
            "body vacio: envia audio binario o JSON con audio_hash".into(),
        ));
    }

    Ok((hex::encode(hasher.finalize()), bytes_hashed))
}

fn normalize_sha256_hex(input: &str) -> Result<String, AppError> {
    let normalized = input.trim().to_ascii_lowercase();
    if normalized.len() != 64 {
        return Err(AppError::BadRequest(
            "audio_hash debe tener 64 caracteres hex".into(),
        ));
    }
    hex::decode(&normalized)
        .map_err(|_| AppError::BadRequest("audio_hash no es SHA-256 hex valido".into()))?;
    Ok(normalized)
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
struct ParsedUpload {
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
enum AudioUploadFormat {
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

/* Campos del multipart tal como llegan; los obligatorios quedan en None hasta validarse. */
#[allow(clippy::struct_excessive_bools)]
#[derive(Default)]
struct CamposSubida {
    original_filename: Option<String>,
    titulo: Option<String>,
    contenido: String,
    tags: Vec<String>,
    permitir_descarga: bool,
    licencia_libre: bool,
    es_premium: bool,
    mostrar_en_comunidad: bool,
    sync_upload: bool,
    origen_subida: Option<String>,
    precio: Option<f64>,
    formato: Option<AudioUploadFormat>,
    audio_bytes: Option<Vec<u8>>,
    /* [166A-6] Campos relacionales y portada. */
    cancion_origen_id: Option<i32>,
    relacion_id: Option<i32>,
    lado_relacion: Option<String>,
    inicio_segundos: Option<i32>,
    tipo_elemento: Option<String>,
    portada_bytes: Option<Vec<u8>>,
    portada_filename: Option<String>,
}

/* Lee el multipart completo y devuelve la subida validada. */
async fn parse_upload_multipart(mut multipart: Multipart) -> Result<ParsedUpload, AppError> {
    let mut campos = CamposSubida {
        permitir_descarga: true,
        mostrar_en_comunidad: true,
        ..CamposSubida::default()
    };
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| AppError::BadRequest(format!("multipart invalido: {error}")))?
    {
        let name = field.name().unwrap_or_default().to_string();
        leer_campo(&name, field, &mut campos).await?;
    }
    finalizar_subida(campos)
}

/* Reparte cada campo entre los dos lectores según su nombre. */
async fn leer_campo(
    name: &str,
    field: axum::extract::multipart::Field<'_>,
    campos: &mut CamposSubida,
) -> Result<(), AppError> {
    match name {
        "audio" | "titulo" | "contenido" | "descripcion" | "tags" | "permitir_descarga"
        | "licencia_libre" | "es_premium" | "mostrar_en_comunidad" | "sync_upload"
        | "origen_subida" => leer_campo_principal(name, field, campos).await,
        _ => leer_campo_relacion(name, field, campos).await,
    }
}

/* Campos de contenido y flags: audio, titulo, tags y booleanos. */
async fn leer_campo_principal(
    name: &str,
    field: axum::extract::multipart::Field<'_>,
    campos: &mut CamposSubida,
) -> Result<(), AppError> {
    match name {
        "audio" => {
            if campos.audio_bytes.is_some() {
                return Err(AppError::BadRequest(
                    "solo se permite un archivo audio".into(),
                ));
            }
            campos.original_filename = field.file_name().map(ToString::to_string);
            let bytes = collect_multipart_bytes(field, MAX_AUDIO_UPLOAD_BYTES).await?;
            campos.formato = Some(detect_audio_format(
                &bytes,
                campos.original_filename.as_deref(),
            )?);
            campos.audio_bytes = Some(bytes);
        }
        "titulo" => {
            campos.titulo = Some(field.text().await.unwrap_or_default().trim().to_string());
        }
        "contenido" | "descripcion" => {
            campos.contenido = field.text().await.unwrap_or_default().trim().to_string();
        }
        "tags" => campos.tags = normalize_tags(&field.text().await.unwrap_or_default()),
        "permitir_descarga" => {
            campos.permitir_descarga =
                parse_bool_field(&field.text().await.unwrap_or_default(), true)?;
        }
        "licencia_libre" => {
            campos.licencia_libre = parse_bool_field(&field.text().await.unwrap_or_default(), false)?;
        }
        "es_premium" => {
            campos.es_premium = parse_bool_field(&field.text().await.unwrap_or_default(), false)?;
        }
        "mostrar_en_comunidad" => {
            campos.mostrar_en_comunidad =
                parse_bool_field(&field.text().await.unwrap_or_default(), true)?;
        }
        "sync_upload" => {
            campos.sync_upload = parse_bool_field(&field.text().await.unwrap_or_default(), false)?;
        }
        "origen_subida" => {
            let value = field.text().await.unwrap_or_default().trim().to_string();
            if !value.is_empty() {
                campos.origen_subida = Some(value);
            }
        }
        _ => {}
    }
    Ok(())
}

/* Campos de precio, relacion con canciones y portada. */
async fn leer_campo_relacion(
    name: &str,
    field: axum::extract::multipart::Field<'_>,
    campos: &mut CamposSubida,
) -> Result<(), AppError> {
    match name {
        "precio" => {
            let raw = field.text().await.unwrap_or_default().trim().to_string();
            if !raw.is_empty() {
                let parsed = raw
                    .parse::<f64>()
                    .map_err(|_| AppError::BadRequest("precio invalido".into()))?;
                if !(0.0..=9999.0).contains(&parsed) {
                    return Err(AppError::BadRequest(
                        "precio fuera de rango valido (0-9999)".into(),
                    ));
                }
                campos.precio = Some(parsed);
            }
        }
        "cancion_origen_id" => {
            let raw = field.text().await.unwrap_or_default().trim().to_string();
            if !raw.is_empty() {
                campos.cancion_origen_id = Some(
                    raw.parse::<i32>()
                        .map_err(|_| AppError::BadRequest("cancion_origen_id invalido".into()))?,
                );
            }
        }
        "relacion_id" => {
            let raw = field.text().await.unwrap_or_default().trim().to_string();
            if !raw.is_empty() {
                campos.relacion_id = Some(
                    raw.parse::<i32>()
                        .map_err(|_| AppError::BadRequest("relacion_id invalido".into()))?,
                );
            }
        }
        "lado_relacion" => {
            let value = field.text().await.unwrap_or_default().trim().to_string();
            if !value.is_empty() {
                if value != "fuente" && value != "destino" {
                    return Err(AppError::BadRequest(
                        "lado_relacion debe ser 'fuente' o 'destino'".into(),
                    ));
                }
                campos.lado_relacion = Some(value);
            }
        }
        "inicio_segundos" => {
            let raw = field.text().await.unwrap_or_default().trim().to_string();
            if !raw.is_empty() {
                let segundos = raw
                    .parse::<i32>()
                    .map_err(|_| AppError::BadRequest("inicio_segundos invalido".into()))?;
                if segundos < 0 {
                    return Err(AppError::BadRequest(
                        "inicio_segundos no puede ser negativo".into(),
                    ));
                }
                campos.inicio_segundos = Some(segundos);
            }
        }
        "tipo_elemento" => {
            let value = field.text().await.unwrap_or_default().trim().to_string();
            if !value.is_empty() {
                campos.tipo_elemento = Some(value);
            }
        }
        "portada" => {
            if campos.portada_bytes.is_some() {
                return Err(AppError::BadRequest(
                    "solo se permite una imagen de portada".into(),
                ));
            }
            campos.portada_filename = field.file_name().map(ToString::to_string);
            let bytes = collect_multipart_bytes(field, MAX_PORTADA_BYTES).await?;
            if !bytes.is_empty() {
                campos.portada_bytes = Some(bytes);
            }
        }
        _ => {}
    }
    Ok(())
}

/* Valida los obligatorios y arma la subida lista para persistir. */
fn finalizar_subida(campos: CamposSubida) -> Result<ParsedUpload, AppError> {
    let audio_bytes = campos.audio_bytes.ok_or(AppError::BadRequest(
        "No se recibio archivo de audio".into(),
    ))?;
    let formato = campos.formato.ok_or(AppError::BadRequest(
        "No se pudo detectar el formato de audio".into(),
    ))?;
    if campos.tags.len() < 2 {
        return Err(AppError::BadRequest(
            "Se requieren al menos 2 tags para subir un sample".into(),
        ));
    }

    let fallback_title = campos
        .original_filename
        .as_deref()
        .and_then(|filename| std::path::Path::new(filename).file_stem())
        .and_then(|stem| stem.to_str())
        .unwrap_or("sample")
        .to_string();
    let titulo = campos
        .titulo
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback_title);

    Ok(ParsedUpload {
        original_filename: campos
            .original_filename
            .unwrap_or_else(|| format!("upload.{}", formato.extension())),
        titulo,
        contenido: campos.contenido,
        tags: campos.tags,
        permitir_descarga: campos.permitir_descarga,
        licencia_libre: campos.licencia_libre,
        es_premium: campos.es_premium,
        mostrar_en_comunidad: campos.mostrar_en_comunidad,
        sync_upload: campos.sync_upload,
        origen_subida: campos.origen_subida,
        precio: campos.precio,
        formato,
        audio_bytes,
        /* [166A-6] Campos relacionales y portada. */
        cancion_origen_id: campos.cancion_origen_id,
        relacion_id: campos.relacion_id,
        lado_relacion: campos.lado_relacion,
        inicio_segundos: campos.inicio_segundos,
        tipo_elemento: campos.tipo_elemento,
        portada_bytes: campos.portada_bytes,
        portada_filename: campos.portada_filename,
    })
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
        return Err(AppError::BadRequest(
            "El archivo de audio esta vacio".into(),
        ));
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

fn normalize_tags(raw: &str) -> Vec<String> {
    let candidates = if raw.trim_start().starts_with('[') {
        serde_json::from_str::<Vec<String>>(raw).unwrap_or_default()
    } else {
        raw.split(',').map(ToString::to_string).collect()
    };

    let mut normalized = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for tag in candidates {
        let normalized_tag = tag.trim().to_ascii_lowercase();
        if normalized_tag.is_empty() || !seen.insert(normalized_tag.clone()) {
            continue;
        }
        normalized.push(normalized_tag);
    }
    normalized
}

fn detect_audio_format(
    bytes: &[u8],
    filename: Option<&str>,
) -> Result<AudioUploadFormat, AppError> {
    let extension = filename
        .and_then(|value| std::path::Path::new(value).extension())
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();

    let allowed_extension = matches!(
        extension.as_str(),
        "wav" | "mp3" | "flac" | "aiff" | "aif" | "ogg"
    );
    if !allowed_extension {
        return Err(AppError::UnsupportedMediaType(
            "Formato de audio no valido. Formatos aceptados: WAV, MP3, FLAC, AIFF, OGG".into(),
        ));
    }

    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        return Ok(AudioUploadFormat::Wav);
    }
    if bytes.len() >= 4 && &bytes[0..4] == b"fLaC" {
        return Ok(AudioUploadFormat::Flac);
    }
    if bytes.len() >= 12
        && &bytes[0..4] == b"FORM"
        && (&bytes[8..12] == b"AIFF" || &bytes[8..12] == b"AIFC")
    {
        return Ok(AudioUploadFormat::Aiff);
    }
    if bytes.len() >= 4 && &bytes[0..4] == b"OggS" {
        return Ok(AudioUploadFormat::Ogg);
    }
    if bytes.len() >= 3 && &bytes[0..3] == b"ID3" {
        return Ok(AudioUploadFormat::Mp3);
    }
    if bytes.len() >= 2 && bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0 {
        return Ok(AudioUploadFormat::Mp3);
    }

    Err(AppError::UnsupportedMediaType(
        "El contenido del archivo no coincide con un formato de audio valido".into(),
    ))
}

fn generate_short_id() -> String {
    const ALPHABET: [char; 36] = [
        'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r',
        's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
    ];
    nanoid::nanoid!(8, &ALPHABET)
}

fn build_storage_key(user_id: i32, slug: &str, extension: &str) -> String {
    let now = chrono::Utc::now();
    format!(
        "samples/{}/{:04}/{:02}/{}.{}",
        user_id,
        now.year(),
        now.month(),
        slug,
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[tokio::test]
    async fn hashes_binary_body_in_streaming_mode() {
        let (hash, bytes_hashed) = hash_body_streaming(Body::from("abc")).await.unwrap();
        assert_eq!(bytes_hashed, 3);
        assert_eq!(
            hash,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[tokio::test]
    async fn accepts_precomputed_hash_json() {
        let (hash, bytes_hashed) = parse_precomputed_hash(Body::from(
            r#"{"audio_hash":"BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"}"#,
        ))
        .await
        .unwrap();
        assert_eq!(bytes_hashed, 0);
        assert_eq!(
            hash,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn rejects_invalid_hash_hex() {
        assert!(normalize_sha256_hex("1234").is_err());
        assert!(normalize_sha256_hex(
            "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"
        )
        .is_err());
    }

    #[test]
    fn normalizes_tags_like_legacy() {
        let tags = normalize_tags("  Trap,drill,TRAP , boom bap ");
        assert_eq!(tags, vec!["trap", "drill", "boom bap"]);
    }

    #[test]
    fn builds_storage_key_under_samples_prefix() {
        let key = build_storage_key(9, "mi-sample-abc123", "wav");
        let now = chrono::Utc::now();
        assert!(key.starts_with(&format!("samples/9/{:04}/{:02}/", now.year(), now.month())));
        assert!(std::path::Path::new(&key)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("wav")));
    }
}
