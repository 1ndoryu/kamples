mod notifications;
mod payload;

use axum::extract::{DefaultBodyLimit, Path, Query, Request, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use utoipa::{IntoParams, ToSchema};

use crate::algorithm::InteractionKind;
use crate::errors::AppError;
use crate::handlers::likes::LikeResponse;
use crate::handlers::social::OkResponse;
use crate::middleware::{CurrentUser, OptionalUser};
use crate::repositories::{
    CommentContentKind, CommentDetail, CommentRepository, CommentTargetKind, CreateCommentParams,
    LikeKind, LikeRepository, ModerationRepository, ProfileRepository, Reaction,
};
use crate::AppState;
use tracing::warn;

use self::{
    notifications::{maybe_notify_comment_creation, maybe_notify_comment_like},
    payload::{
        build_comment_storage_key, extract_storage_key, normalize_content,
        parse_create_comment_request,
    },
};

mod dto;
mod lectura;
mod escritura;
mod reacciones;
mod soporte;

pub use dto::*;
pub use lectura::*;
pub use escritura::*;
pub use reacciones::*;
use soporte::*;

const MAX_COMMENT_CHARS: usize = 2_000;
const MAX_JSON_BODY_BYTES: usize = 64 * 1024;
const MAX_IMAGE_UPLOAD_BYTES: usize = 8 * 1024 * 1024;
const MAX_AUDIO_UPLOAD_BYTES: usize = 24 * 1024 * 1024;
const MAX_COMMENT_MULTIPART_BODY_BYTES: usize = MAX_AUDIO_UPLOAD_BYTES + (4 * 1024 * 1024);
const DEFAULT_PAGE: i64 = 1;
const DEFAULT_PER_PAGE: i64 = 20;
const MAX_PER_PAGE: i64 = 100;
const DEFAULT_REPLIES_LIMIT: i64 = 100;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/comentarios/:tipo/:target_id",
            get(list_comments)
                .post(create_comment)
                .layer(DefaultBodyLimit::max(MAX_COMMENT_MULTIPART_BODY_BYTES)),
        )
        .route(
            "/comentarios/:comment_id",
            axum::routing::put(update_comment).delete(delete_comment),
        )
        .route("/comentarios/:comment_id/respuestas", get(list_replies))
        .route(
            "/comentarios/:comment_id/like",
            post(like_comment).delete(unlike_comment),
        )
}
