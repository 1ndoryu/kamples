use super::*;

pub(super) async fn collect_hidden_author_ids(
    state: &AppState,
    viewer_id: Option<i32>,
) -> Result<Vec<i32>, AppError> {
    let Some(user_id) = viewer_id else {
        return Ok(Vec::new());
    };

    let mut ids = ModerationRepository::list_blocked(&state.pool, user_id).await?;
    ids.extend(ModerationRepository::list_blockers(&state.pool, user_id).await?);
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

pub(super) fn normalize_comment_detail(
    mut detail: CommentDetail,
    public_base_url: Option<&str>,
) -> CommentDetail {
    detail.media_url = asset_to_public_url(public_base_url, detail.media_url);
    detail.autor.avatar_url = asset_to_public_url(public_base_url, detail.autor.avatar_url);
    detail
}

fn asset_to_public_url(public_base_url: Option<&str>, raw: Option<String>) -> Option<String> {
    let raw = raw?.trim().replace('\\', "/");
    if raw.is_empty() {
        return None;
    }
    if raw.starts_with("http://") || raw.starts_with("https://") {
        return Some(raw);
    }

    let path = if raw.starts_with('/') {
        raw
    } else {
        format!("/uploads/{raw}")
    };

    Some(match public_base_url {
        Some(base) => format!("{}{}", base.trim_end_matches('/'), path),
        None => path,
    })
}
