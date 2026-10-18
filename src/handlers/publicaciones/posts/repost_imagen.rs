use super::*;

#[utoipa::path(
    post, path = "/api/publicaciones/{id}/repost", tag = "posts",
    params(("id" = i32, Path, description = "ID de la publicación original a repostear")),
    security(("bearer_auth" = [])),
    responses((status = 200, body = RepostResponse), (status = 400, description = "No se puede repostear"))
)]
pub async fn repost_post(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i32>,
) -> Result<Json<RepostResponse>, AppError> {
    let (owner_id, repost_id) = PostRepository::fetch_meta(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("publicacion {id} no existe")))?;
    if owner_id == user.user_id {
        return Err(AppError::BadRequest(
            "no puedes repostear tu propia publicacion".into(),
        ));
    }
    if repost_id.is_some() {
        return Err(AppError::BadRequest(
            "no se puede repostear un repost".into(),
        ));
    }
    let (repost_id, already_exists) =
        PostRepository::create_repost(&state.pool, user.user_id, id).await?;
    let hidden = collect_hidden_author_ids(&state, user.user_id).await?;
    let post = PostRepository::get(&state.pool, user.user_id, repost_id, &hidden)
        .await?
        .ok_or_else(|| AppError::Internal(format!("repost {repost_id} no visible")))?;
    Ok(Json(RepostResponse {
        ok: true,
        already_exists,
        post,
    }))
}

#[utoipa::path(
    delete, path = "/api/publicaciones/{id}/repost", tag = "posts",
    params(("id" = i32, Path, description = "ID de la publicación original cuyo repost se quiere quitar")),
    security(("bearer_auth" = [])),
    responses((status = 200, body = OkResponse))
)]
pub async fn unrepost_post(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i32>,
) -> Result<Json<OkResponse>, AppError> {
    let removed = PostRepository::delete_repost(&state.pool, user.user_id, id).await?;
    if !removed {
        return Err(AppError::NotFound(format!(
            "no existe repost propio para la publicacion {id}"
        )));
    }
    Ok(Json(OkResponse { ok: true }))
}

#[derive(Serialize, ToSchema)]
pub(super) struct UploadImageResponse {
    url: String,
}

/* POST /publicaciones/imagenes — sube una imagen para adjuntar a una publicación.
 * Acepta multipart/form-data con campo "imagen" (JPEG, PNG, WEBP, máx 5 MB).
 * Devuelve la URL pública del archivo subido. */
#[utoipa::path(
    post,
    path = "/api/publicaciones/imagenes",
    tag = "posts",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, body = UploadImageResponse),
        (status = 400, description = "Imagen inválida o faltante"),
        (status = 413, description = "Imagen demasiado grande"),
    )
)]
pub(super) async fn upload_post_image(
    State(state): State<AppState>,
    user: CurrentUser,
    mut multipart: Multipart,
) -> Result<Json<UploadImageResponse>, AppError> {
    let mut bytes: Option<Vec<u8>> = None;
    let mut extension: Option<&'static str> = None;

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("multipart inválido: {e}")))?
    {
        let name = field.name().unwrap_or_default().to_string();
        if name != "imagen" {
            continue;
        }
        let filename = field.file_name().map(ToString::to_string);
        let mut acc = Vec::new();
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|e| AppError::BadRequest(format!("chunk inválido: {e}")))?
        {
            if acc.len().saturating_add(chunk.len()) > MAX_POST_IMAGE_BYTES {
                return Err(AppError::BadRequest(format!(
                    "Imagen excede {} MB",
                    MAX_POST_IMAGE_BYTES / (1024 * 1024)
                )));
            }
            acc.extend_from_slice(&chunk);
        }
        extension = Some(detect_image_extension(&acc, filename.as_deref())?);
        bytes = Some(acc);
        break;
    }

    let Some(image_bytes) = bytes else {
        return Err(AppError::BadRequest("Campo 'imagen' faltante".into()));
    };
    let ext = extension.unwrap_or("jpg");

    let now = Utc::now();
    let storage_key = format!(
        "posts/{}/{:04}/{:02}/{}.{}",
        user.user_id,
        now.year(),
        now.month(),
        nanoid!(10),
        ext,
    );
    state.storage.put_bytes(&storage_key, &image_bytes).await?;

    let url = if let Some(base) = &state.public_base_url {
        format!("{}/uploads/{}", base.trim_end_matches('/'), storage_key)
    } else {
        format!("/uploads/{storage_key}")
    };

    Ok(Json(UploadImageResponse { url }))
}

fn detect_image_extension(bytes: &[u8], filename: Option<&str>) -> Result<&'static str, AppError> {
    let ext = filename
        .and_then(|v| std::path::Path::new(v).extension())
        .and_then(|v| v.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();

    if bytes.len() >= 3
        && bytes[0..3] == [0xFF, 0xD8, 0xFF]
        && matches!(ext.as_str(), "" | "jpg" | "jpeg")
    {
        return Ok("jpg");
    }
    if bytes.len() >= 8
        && bytes[0..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
        && matches!(ext.as_str(), "" | "png")
    {
        return Ok("png");
    }
    if bytes.len() >= 12
        && &bytes[0..4] == b"RIFF"
        && &bytes[8..12] == b"WEBP"
        && matches!(ext.as_str(), "" | "webp")
    {
        return Ok("webp");
    }

    Err(AppError::UnsupportedMediaType(
        "Formato de imagen no válido. Formatos aceptados: JPEG, PNG, WEBP".into(),
    ))
}
