use super::*;

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
