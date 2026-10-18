use super::*;

#[derive(Debug, Clone)]
pub(super) struct NormalizedPostPayload {
    pub(super) contenido: String,
    pub(super) imagenes: Vec<String>,
    pub(super) samples_adjuntos: Vec<i32>,
}


pub(super) async fn collect_hidden_author_ids(state: &AppState, user_id: i32) -> Result<Vec<i32>, AppError> {
    let mut ids = ModerationRepository::list_blocked(&state.pool, user_id).await?;
    ids.extend(ModerationRepository::list_blockers(&state.pool, user_id).await?);
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

pub(super) async fn ensure_owner_original_post(
    state: &AppState,
    post_id: i32,
    user_id: i32,
) -> Result<(), AppError> {
    let (owner_id, repost_id) = PostRepository::fetch_meta(&state.pool, post_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("publicacion {post_id} no existe")))?;
    if owner_id != user_id {
        return Err(AppError::Forbidden(
            "no eres autor de la publicacion".into(),
        ));
    }
    if repost_id.is_some() {
        return Err(AppError::BadRequest(
            "la publicacion es un repost; usa el endpoint /repost para quitarlo".into(),
        ));
    }
    Ok(())
}

pub(super) async fn ensure_can_delete_original_post(
    state: &AppState,
    post_id: i32,
    user: &CurrentUser,
) -> Result<i32, AppError> {
    let (owner_id, repost_id) = PostRepository::fetch_meta(&state.pool, post_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("publicacion {post_id} no existe")))?;
    if repost_id.is_some() {
        return Err(AppError::BadRequest(
            "la publicacion es un repost; usa el endpoint /repost para quitarlo".into(),
        ));
    }
    /* [274A-5] El claim `rol` del JWT puede quedar viejo si la sesión se emitió antes
     * de elevar al usuario a admin. Para autorización destructiva, consultar la BD
     * evita 403 falsos hasta que el usuario vuelva a iniciar sesión. */
    let is_admin = UserRepository::find_by_id(&state.pool, user.user_id)
        .await?
        .is_some_and(|db_user| db_user.rol == "admin");
    if owner_id != user.user_id && !is_admin {
        return Err(AppError::Forbidden(
            "no eres autor de la publicacion".into(),
        ));
    }
    Ok(owner_id)
}

pub(super) async fn ensure_samples_exist(state: &AppState, sample_ids: &[i32]) -> Result<(), AppError> {
    if !PostRepository::all_samples_exist(&state.pool, sample_ids).await? {
        return Err(AppError::BadRequest(
            "samples_adjuntos contiene IDs inexistentes o eliminados".into(),
        ));
    }
    Ok(())
}

pub(super) fn parse_filter(filter: &str) -> Result<(bool, bool), AppError> {
    match filter {
        "todos" => Ok((false, false)),
        "siguiendo" => Ok((true, false)),
        "populares" => Ok((false, true)),
        other => Err(AppError::Validation(format!("filtro inválido: {other}"))),
    }
}

pub(super) fn normalize_post_payload(
    contenido: &str,
    imagenes: Vec<String>,
    samples_adjuntos: Vec<i32>,
) -> Result<NormalizedPostPayload, AppError> {
    let contenido = contenido.trim().to_string();
    if contenido.len() > MAX_POST_CONTENT {
        return Err(AppError::Validation(format!(
            "contenido demasiado largo (max {MAX_POST_CONTENT} chars)"
        )));
    }

    let imagenes: Vec<String> = imagenes
        .into_iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .take(MAX_POST_IMAGES + 1)
        .collect();
    if imagenes.len() > MAX_POST_IMAGES {
        return Err(AppError::Validation(format!(
            "demasiadas imagenes (max {MAX_POST_IMAGES})"
        )));
    }

    let mut samples_adjuntos = samples_adjuntos;
    samples_adjuntos.sort_unstable();
    samples_adjuntos.dedup();
    if samples_adjuntos.len() > MAX_ATTACHED_SAMPLES {
        return Err(AppError::Validation(format!(
            "demasiados samples adjuntos (max {MAX_ATTACHED_SAMPLES})"
        )));
    }
    if contenido.is_empty() && imagenes.is_empty() && samples_adjuntos.is_empty() {
        return Err(AppError::Validation(
            "la publicacion debe tener contenido, imagenes o samples_adjuntos".into(),
        ));
    }

    Ok(NormalizedPostPayload {
        contenido,
        imagenes,
        samples_adjuntos,
    })
}

#[cfg(test)]
mod tests {
    use super::{normalize_post_payload, parse_filter};

    #[test]
    fn normalize_rejects_empty_payload() {
        assert!(normalize_post_payload("", Vec::new(), Vec::new()).is_err());
    }

    #[test]
    fn normalize_allows_media_only_post() {
        let payload = normalize_post_payload("", vec![" https://cdn/test.png ".into()], vec![5, 5])
            .expect("payload");
        assert_eq!(payload.imagenes, vec!["https://cdn/test.png"]);
        assert_eq!(payload.samples_adjuntos, vec![5]);
    }

    #[test]
    fn parse_filter_supports_expected_values() {
        assert_eq!(parse_filter("todos").expect("todos"), (false, false));
        assert_eq!(parse_filter("siguiendo").expect("siguiendo"), (true, false));
        assert_eq!(parse_filter("populares").expect("populares"), (false, true));
        assert!(parse_filter("otro").is_err());
    }
}
