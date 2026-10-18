use crate::errors::AppError;
use crate::repositories::{CommentRepository, CommentTargetKind, NotificationTargetRepository};
use crate::services::NotificationFanoutService;
use crate::AppState;

pub async fn maybe_notify_comment_creation(
    state: &AppState,
    actor_id: i32,
    target_kind: CommentTargetKind,
    target_id: i32,
    parent_id: Option<i32>,
) -> Result<(), AppError> {
    if let Some(parent_id) = parent_id {
        let parent = CommentRepository::find_context(&state.pool, parent_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("comentario padre {parent_id} no existe")))?;
        let sample_slug = match target_kind {
            CommentTargetKind::Sample => {
                NotificationTargetRepository::find_sample_meta(&state.pool, target_id)
                    .await?
                    .and_then(|meta| meta.slug)
            }
            CommentTargetKind::Publicacion
            | CommentTargetKind::Cancion
            | CommentTargetKind::Relacion
            | CommentTargetKind::Articulo => None,
        };

        NotificationFanoutService::dispatch_comment_reply(
            state,
            parent.autor_id,
            actor_id,
            parent_id,
            matches!(target_kind, CommentTargetKind::Sample).then_some(target_id),
            sample_slug.as_deref(),
        )
        .await?;
        return Ok(());
    }

    match target_kind {
        CommentTargetKind::Sample => {
            if let Some(meta) =
                NotificationTargetRepository::find_sample_meta(&state.pool, target_id).await?
            {
                NotificationFanoutService::dispatch_sample_comment(
                    state,
                    meta.creator_id,
                    actor_id,
                    target_id,
                    &meta.title,
                    meta.slug.as_deref(),
                )
                .await?;
            }
        }
        CommentTargetKind::Publicacion => {
            if let Some(meta) =
                NotificationTargetRepository::find_post_meta(&state.pool, target_id).await?
            {
                NotificationFanoutService::dispatch_post_comment(
                    state,
                    meta.author_id,
                    actor_id,
                    target_id,
                    &meta.content,
                )
                .await?;
            }
        }
        CommentTargetKind::Cancion | CommentTargetKind::Relacion | CommentTargetKind::Articulo => {}
    }

    Ok(())
}

pub async fn maybe_notify_comment_like(
    state: &AppState,
    actor_id: i32,
    comment_id: i32,
) -> Result<(), AppError> {
    let context = CommentRepository::find_context(&state.pool, comment_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("comentario {comment_id} no existe")))?;

    NotificationFanoutService::dispatch_comment_reaction(
        state,
        context.autor_id,
        actor_id,
        comment_id,
    )
    .await
}
