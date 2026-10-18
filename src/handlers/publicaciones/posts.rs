use std::sync::Arc;

use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{Datelike, Utc};
use nanoid::nanoid;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::errors::AppError;
use crate::middleware::{CurrentUser, OptionalUser};
use crate::repositories::{ModerationRepository, PostDetail, PostRepository, UserRepository};
use crate::AppState;

const MAX_POST_CONTENT: usize = 5_000;
const MAX_POST_IMAGES: usize = 10;
const MAX_ATTACHED_SAMPLES: usize = 16;
const MAX_POST_IMAGE_BYTES: usize = 5 * 1024 * 1024;
const MAX_POST_IMAGE_MULTIPART: usize = MAX_POST_IMAGE_BYTES + 256 * 1024;

/* [174A-67] Este handler separa publicaciones/reposts del resto de sociales para mantener
 * rutas y validaciones del dominio en un módulo acotado.
 * Gotcha: comentarios y multimedia conversacional quedan fuera; 174A-68 cubre ese siguiente corte.
 * Pendiente: integrar notificaciones/recuento de comentarios cuando exista ese dominio. */

mod crud;
mod dto;
mod repost_imagen;
mod soporte;

pub use crud::*;
pub use dto::*;
pub use repost_imagen::*;
use soporte::*;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/publicaciones", post(create_post).get(list_posts))
        .route(
            "/publicaciones/imagenes",
            post(upload_post_image).layer(axum::extract::DefaultBodyLimit::max(MAX_POST_IMAGE_MULTIPART)),
        )
        .route(
            "/publicaciones/:id",
            get(get_post).put(update_post).delete(delete_post),
        )
        .route(
            "/publicaciones/:id/repost",
            post(repost_post).delete(unrepost_post),
        )
}
