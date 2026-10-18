use super::*;

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
pub(super) async fn parse_upload_multipart(mut multipart: Multipart) -> Result<ParsedUpload, AppError> {
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
