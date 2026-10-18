use super::*;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
