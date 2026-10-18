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

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreatePostRequest {
    #[serde(default)]
    pub contenido: String,
    #[serde(default)]
    pub imagenes: Vec<String>,
    #[serde(default, alias = "samplesAdjuntos")]
    pub samples_adjuntos: Vec<i32>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdatePostRequest {
    #[serde(default)]
    pub contenido: String,
    #[serde(default)]
    pub imagenes: Vec<String>,
    #[serde(default, alias = "samplesAdjuntos")]
    pub samples_adjuntos: Vec<i32>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct PostListQuery {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
    #[serde(default = "default_filter")]
    pub filtro: String,
    pub author_id: Option<i32>,
    /* [254A-6] El frontend legacy (apiSocial.listarPublicacionesUsuario) envia
     * `?autor=<username>` en vez de `?author_id=<int>`. Aceptamos ambos para no
     * acoplar el backend al wpJsonStub: si llega autor, lo resolvemos al id real. */
    pub autor: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PostListResponse {
    pub items: Vec<PostDetail>,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PostMutationResponse {
    pub ok: bool,
    pub post: PostDetail,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RepostResponse {
    pub ok: bool,
    pub already_exists: bool,
    pub post: PostDetail,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OkResponse {
    pub ok: bool,
}

#[derive(Debug, Clone)]
struct NormalizedPostPayload {
    contenido: String,
    imagenes: Vec<String>,
    samples_adjuntos: Vec<i32>,
}

fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    20
}
fn default_filter() -> String {
    "todos".to_string()
}

#[utoipa::path(
    post, path = "/api/publicaciones", tag = "posts",
    request_body = CreatePostRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 201, body = PostMutationResponse),
        (status = 400, description = "Payload inválido"),
    )
)]
pub async fn create_post(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<CreatePostRequest>,
) -> Result<(StatusCode, Json<PostMutationResponse>), AppError> {
    let payload = normalize_post_payload(&body.contenido, body.imagenes, body.samples_adjuntos)?;
    ensure_samples_exist(&state, &payload.samples_adjuntos).await?;
    let id = PostRepository::create(
        &state.pool,
        user.user_id,
        &payload.contenido,
        &payload.imagenes,
        &payload.samples_adjuntos,
    )
    .await?;
    let hidden = collect_hidden_author_ids(&state, user.user_id).await?;
    let post = PostRepository::get(&state.pool, user.user_id, id, &hidden)
        .await?
        .ok_or_else(|| AppError::Internal(format!("post {id} recién creado no visible")))?;

    /* Moderación asíncrona: texto + imágenes con Groq Vision (capas 1-3 del legacy).
     * El post se crea como 'aprobado'; si la IA lo rechaza, se actualiza a
     * 'rechazado' o 'revision' en background sin bloquear la respuesta 201. */
    if let Some(moderation) = &state.moderation {
        let moderation = Arc::clone(moderation);
        let pool = state.pool.clone();
        let contenido = payload.contenido.clone();
        let imagenes = payload.imagenes.clone();
        tokio::spawn(async move {
            let request = crate::services::ModerationRequest {
                entity_kind: crate::services::ModerationEntityKind::Publication,
                entity_id: Some(i64::from(id)),
                title: String::new(),
                body: contenido,
                tags: Vec::new(),
                primary_folder: None,
                secondary_folder: None,
                image_urls: imagenes,
                extra_context: None,
            };
            let result = moderation.moderate(&request).await;
            let (estado, razon) = match result.decision.verdict {
                crate::services::ModerationVerdict::Approved => return,
                crate::services::ModerationVerdict::Review => {
                    ("revision", result.decision.summary.clone())
                }
                crate::services::ModerationVerdict::Rejected => {
                    ("rechazado", result.decision.summary.clone())
                }
            };
            let detalle = serde_json::json!({
                "category": result.decision.category.as_str(),
                "reason_code": &result.decision.reason_code,
                "ai": result.ai_assessment.as_ref().map(|a| serde_json::json!({
                    "provider": a.provider.as_str(),
                    "model": &a.model,
                    "confidence": a.confidence,
                })),
            });
            if let Err(err) = PostRepository::update_moderation_status(
                &pool, id, estado, detalle, &razon,
            )
            .await
            {
                tracing::error!(post_id = id, error = %err, "Failed to update moderation status");
            } else {
                tracing::info!(post_id = id, estado, "Post moderation complete");
            }
        });
    }

    Ok((
        StatusCode::CREATED,
        Json(PostMutationResponse { ok: true, post }),
    ))
}

#[utoipa::path(
    get, path = "/api/publicaciones", tag = "posts",
    params(PostListQuery),
    responses((status = 200, body = PostListResponse))
)]
pub async fn list_posts(
    State(state): State<AppState>,
    user: OptionalUser,
    Query(query): Query<PostListQuery>,
) -> Result<Json<PostListResponse>, AppError> {
    /* [204A-1] Feed público: guests ven posts sin reacciones personalizadas.
     * viewer_id=0 garantiza que los subqueries SQL (siguiendo, mi_reaccion, etc.)
     * devuelvan false/null para guests — ningún user tiene id=0 en la BD. */
    let viewer_id = user.0.as_ref().map_or(0, |u| u.user_id);
    let (only_following, sort_popular) = parse_filter(&query.filtro)?;
    /* only_following requiere auth — si no hay usuario, ignorar el filtro */
    let only_following = only_following && user.0.is_some();
    let page = query.page.max(1);
    let per_page = query.per_page.clamp(1, 100);
    let offset = (page - 1) * per_page;
    let hidden = if let Some(ref u) = user.0 {
        collect_hidden_author_ids(&state, u.user_id).await?
    } else {
        vec![]
    };
    /* [254A-6] author_id directo tiene precedencia; si solo viene `autor=username`,
     * lo traducimos. Username inexistente => lista vacia (no error) para no romper
     * la UX del perfil cuando alguien navega a un usuario borrado. */
    let author_id = match query.author_id {
        Some(id) => Some(id),
        None => match query
            .autor
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(username) => {
                match crate::repositories::UserRepository::find_by_username(&state.pool, username)
                    .await?
                {
                    Some(u) => Some(u.id),
                    None => {
                        return Ok(Json(PostListResponse {
                            items: vec![],
                            page,
                            per_page,
                        }));
                    }
                }
            }
            None => None,
        },
    };
    let items = PostRepository::list(
        &state.pool,
        crate::repositories::PostListParams {
            viewer_id,
            only_following,
            sort_popular,
            author_id,
            blocked_ids: &hidden,
            limit: per_page,
            offset,
        },
    )
    .await?;
    Ok(Json(PostListResponse {
        items,
        page,
        per_page,
    }))
}

#[utoipa::path(
    get, path = "/api/publicaciones/{id}", tag = "posts",
    params(("id" = i32, Path, description = "ID de la publicación")),
    security(("bearer_auth" = [])),
    responses((status = 200, body = PostMutationResponse), (status = 404, description = "No encontrada"))
)]
pub async fn get_post(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i32>,
) -> Result<Json<PostMutationResponse>, AppError> {
    let hidden = collect_hidden_author_ids(&state, user.user_id).await?;
    let post = PostRepository::get(&state.pool, user.user_id, id, &hidden)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("publicacion {id} no existe")))?;
    Ok(Json(PostMutationResponse { ok: true, post }))
}

#[utoipa::path(
    put, path = "/api/publicaciones/{id}", tag = "posts",
    params(("id" = i32, Path, description = "ID de la publicación a editar")),
    request_body = UpdatePostRequest,
    security(("bearer_auth" = [])),
    responses((status = 200, body = PostMutationResponse), (status = 403, description = "No autorizado"))
)]
pub async fn update_post(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i32>,
    Json(body): Json<UpdatePostRequest>,
) -> Result<Json<PostMutationResponse>, AppError> {
    ensure_owner_original_post(&state, id, user.user_id).await?;
    let payload = normalize_post_payload(&body.contenido, body.imagenes, body.samples_adjuntos)?;
    ensure_samples_exist(&state, &payload.samples_adjuntos).await?;
    let updated = PostRepository::update(
        &state.pool,
        id,
        user.user_id,
        &payload.contenido,
        &payload.imagenes,
        &payload.samples_adjuntos,
    )
    .await?;
    if !updated {
        return Err(AppError::Conflict(format!(
            "no se pudo actualizar la publicacion {id}"
        )));
    }
    let hidden = collect_hidden_author_ids(&state, user.user_id).await?;
    let post = PostRepository::get(&state.pool, user.user_id, id, &hidden)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("publicacion {id} no existe")))?;
    Ok(Json(PostMutationResponse { ok: true, post }))
}

#[utoipa::path(
    delete, path = "/api/publicaciones/{id}", tag = "posts",
    params(("id" = i32, Path, description = "ID de la publicación a eliminar")),
    security(("bearer_auth" = [])),
    responses((status = 200, body = OkResponse), (status = 403, description = "No autorizado"))
)]
pub async fn delete_post(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i32>,
) -> Result<Json<OkResponse>, AppError> {
    let owner_id = ensure_can_delete_original_post(&state, id, &user).await?;
    let deleted = PostRepository::soft_delete(&state.pool, id, owner_id).await?;
    if !deleted {
        return Err(AppError::Conflict(format!(
            "no se pudo eliminar la publicacion {id}"
        )));
    }
    Ok(Json(OkResponse { ok: true }))
}

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
struct UploadImageResponse {
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
async fn upload_post_image(
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

async fn collect_hidden_author_ids(state: &AppState, user_id: i32) -> Result<Vec<i32>, AppError> {
    let mut ids = ModerationRepository::list_blocked(&state.pool, user_id).await?;
    ids.extend(ModerationRepository::list_blockers(&state.pool, user_id).await?);
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

async fn ensure_owner_original_post(
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

async fn ensure_can_delete_original_post(
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

async fn ensure_samples_exist(state: &AppState, sample_ids: &[i32]) -> Result<(), AppError> {
    if !PostRepository::all_samples_exist(&state.pool, sample_ids).await? {
        return Err(AppError::BadRequest(
            "samples_adjuntos contiene IDs inexistentes o eliminados".into(),
        ));
    }
    Ok(())
}

fn parse_filter(filter: &str) -> Result<(bool, bool), AppError> {
    match filter {
        "todos" => Ok((false, false)),
        "siguiendo" => Ok((true, false)),
        "populares" => Ok((false, true)),
        other => Err(AppError::Validation(format!("filtro inválido: {other}"))),
    }
}

fn normalize_post_payload(
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
