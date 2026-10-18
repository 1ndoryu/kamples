mod support;

use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{Duration, Utc};
use serde::Deserialize;
use utoipa::IntoParams;

use self::support::{
    can_manage_article, default_my_articles_per_page, default_page, default_per_page,
    load_manageable_article_meta, normalize_category_filter, normalize_moderation_filter,
    normalize_update_request, parse_create_article_multipart, resolve_cover_url,
    CREATE_RATE_LIMIT_PER_HOUR, MAX_PER_PAGE,
};
use crate::errors::AppError;
#[allow(unused_imports)]
use crate::errors::ErrorResponse;
use crate::middleware::{CurrentUser, OptionalUser};
#[allow(unused_imports)]
use crate::models::{
    ArticleCategoriesResponse, ArticleListData, ArticleListResponse, ArticleResponse,
    CreateArticleMultipartRequestDoc, DeleteArticleData, DeleteArticleResponse,
    ToggleArticleLikeResponse, UpdateArticleRequest,
};
use crate::repositories::{ArticleRepository, CreateArticleParams};
use crate::AppState;

const MAX_ARTICLE_MULTIPART_BODY_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ArticleListQuery {
    #[serde(default = "default_page", alias = "page")]
    pub pagina: i64,
    #[serde(default = "default_per_page", alias = "per_page")]
    pub limite: i64,
    #[serde(default)]
    pub categoria: Option<String>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct MyArticlesQuery {
    #[serde(default = "default_page", alias = "page")]
    pub pagina: i64,
    #[serde(default = "default_my_articles_per_page", alias = "per_page")]
    pub limite: i64,
    #[serde(default)]
    pub moderacion_estado: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/articulos",
    tag = "articles",
    params(ArticleListQuery),
    responses(
        (status = 200, description = "Listado público de artículos", body = ArticleListResponse),
        (status = 422, description = "Query inválida", body = ErrorResponse)
    )
)]
pub async fn list_articles(
    State(state): State<AppState>,
    viewer: OptionalUser,
    Query(query): Query<ArticleListQuery>,
) -> Result<Json<ArticleListResponse>, AppError> {
    let page = query.pagina.max(1);
    let per_page = query.limite.clamp(1, MAX_PER_PAGE);
    let offset = (page - 1) * per_page;
    let categoria = normalize_category_filter(query.categoria.as_deref());
    let viewer_id = viewer.0.as_ref().map(|user| user.user_id);

    let articulos = ArticleRepository::list_published(
        &state.pool,
        categoria.as_deref(),
        viewer_id,
        per_page,
        offset,
    )
    .await?;
    let total = ArticleRepository::count_published(&state.pool, categoria.as_deref()).await?;
    let hay_mas = offset + i64::try_from(articulos.len()).unwrap_or(i64::MAX) < total;

    Ok(Json(ArticleListResponse {
        ok: true,
        data: ArticleListData {
            articulos,
            total,
            hay_mas,
        },
    }))
}

#[utoipa::path(
    get,
    path = "/api/articulos/categorias",
    tag = "articles",
    responses((status = 200, description = "Conteo por categorías", body = ArticleCategoriesResponse))
)]
pub async fn list_categories(
    State(state): State<AppState>,
) -> Result<Json<ArticleCategoriesResponse>, AppError> {
    let categorias = ArticleRepository::count_by_category(&state.pool).await?;
    Ok(Json(ArticleCategoriesResponse {
        ok: true,
        data: categorias,
    }))
}

#[utoipa::path(
    get,
    path = "/api/articulos/{slug}",
    tag = "articles",
    params(("slug" = String, Path, description = "Slug del artículo")),
    responses(
        (status = 200, description = "Detalle del artículo", body = ArticleResponse),
        (status = 404, description = "Artículo no encontrado", body = ErrorResponse)
    )
)]
pub async fn get_article(
    State(state): State<AppState>,
    viewer: OptionalUser,
    Path(slug): Path<String>,
) -> Result<Json<ArticleResponse>, AppError> {
    let viewer_user = viewer.0;
    let article = ArticleRepository::get_by_slug(
        &state.pool,
        slug.trim(),
        viewer_user.as_ref().map(|user| user.user_id),
    )
    .await?
    .ok_or_else(|| AppError::NotFound(format!("articulo {} no encontrado", slug.trim())))?;

    if article.moderacion_estado != "aprobado"
        && !can_manage_article(viewer_user.as_ref(), article.autor_id)
    {
        return Err(AppError::NotFound(format!(
            "articulo {} no encontrado",
            slug.trim()
        )));
    }

    Ok(Json(ArticleResponse {
        ok: true,
        data: article,
    }))
}

#[utoipa::path(
    get,
    path = "/api/articulos/mis-articulos",
    tag = "articles",
    params(MyArticlesQuery),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Artículos del usuario autenticado", body = ArticleListResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse)
    )
)]
pub async fn list_my_articles(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<MyArticlesQuery>,
) -> Result<Json<ArticleListResponse>, AppError> {
    let page = query.pagina.max(1);
    let per_page = query.limite.clamp(1, MAX_PER_PAGE);
    let offset = (page - 1) * per_page;
    let moderacion_estado = normalize_moderation_filter(query.moderacion_estado.as_deref());
    let articulos = ArticleRepository::list_by_author(
        &state.pool,
        user.user_id,
        moderacion_estado.as_deref(),
        Some(user.user_id),
        per_page,
        offset,
    )
    .await?;
    let total =
        ArticleRepository::count_by_author(&state.pool, user.user_id, moderacion_estado.as_deref())
            .await?;
    let hay_mas = offset + i64::try_from(articulos.len()).unwrap_or(i64::MAX) < total;

    Ok(Json(ArticleListResponse {
        ok: true,
        data: ArticleListData {
            articulos,
            total,
            hay_mas,
        },
    }))
}

#[utoipa::path(
    post,
    path = "/api/articulos",
    tag = "articles",
    request_body(
        content = CreateArticleMultipartRequestDoc,
        content_type = "multipart/form-data",
        description = "Crea un artículo con portada opcional en archivo"
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 201, description = "Artículo creado", body = ArticleResponse),
        (status = 400, description = "Payload inválido", body = ErrorResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 413, description = "Archivo demasiado grande", body = ErrorResponse),
        (status = 415, description = "Tipo de imagen no soportado", body = ErrorResponse),
        (status = 429, description = "Límite de creación excedido", body = ErrorResponse)
    )
)]
pub async fn create_article(
    State(state): State<AppState>,
    user: CurrentUser,
    multipart: Multipart,
) -> Result<(StatusCode, Json<ArticleResponse>), AppError> {
    let since = Utc::now() - Duration::hours(1);
    let total_recent =
        ArticleRepository::count_recent_by_author(&state.pool, user.user_id, since).await?;
    if total_recent >= CREATE_RATE_LIMIT_PER_HOUR {
        return Err(AppError::TooManyRequests(
            "Demasiados artículos creados. Intenta más tarde.".to_string(),
        ));
    }

    let payload = parse_create_article_multipart(multipart).await?;
    let slug = ArticleRepository::generate_unique_slug(&state.pool, &payload.titulo, None).await?;
    let portada_url = resolve_cover_url(
        &state,
        user.user_id,
        &slug,
        payload.portada_url,
        payload.portada,
    )
    .await?;
    let moderacion_estado = if user.rol == "admin" {
        "aprobado".to_string()
    } else {
        "pendiente".to_string()
    };
    let publicado_en = (moderacion_estado == "aprobado").then_some(Utc::now());

    let article_id = ArticleRepository::create(
        &state.pool,
        &CreateArticleParams {
            autor_id: user.user_id,
            titulo: payload.titulo,
            slug,
            contenido: payload.contenido,
            extracto: payload.extracto,
            portada_url,
            categoria: payload.categoria,
            embeds: serde_json::to_value(payload.embeds)
                .map_err(|error| AppError::Internal(format!("serializar embeds: {error}")))?,
            descarga_publica: payload.descarga_publica,
            moderacion_estado,
            publicado_en,
        },
    )
    .await?;

    let article = ArticleRepository::get_by_id(&state.pool, article_id, Some(user.user_id))
        .await?
        .ok_or_else(|| {
            AppError::Internal(format!("articulo {article_id} recien creado no visible"))
        })?;

    Ok((
        StatusCode::CREATED,
        Json(ArticleResponse {
            ok: true,
            data: article,
        }),
    ))
}

#[utoipa::path(
    put,
    path = "/api/articulos/{id}",
    tag = "articles",
    params(("id" = i32, Path, description = "ID del artículo a actualizar")),
    request_body = UpdateArticleRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Artículo actualizado", body = ArticleResponse),
        (status = 400, description = "Payload inválido", body = ErrorResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Sin permisos", body = ErrorResponse),
        (status = 404, description = "Artículo no encontrado", body = ErrorResponse)
    )
)]
pub async fn update_article(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(article_id): Path<i32>,
    Json(body): Json<UpdateArticleRequest>,
) -> Result<Json<ArticleResponse>, AppError> {
    let meta = load_manageable_article_meta(&state, &user, article_id).await?;
    let update = normalize_update_request(&state, article_id, &body).await?;
    let updated = ArticleRepository::update(&state.pool, meta.id, &update).await?;
    if !updated {
        return Err(AppError::Conflict(format!(
            "no se pudo actualizar el articulo {article_id}"
        )));
    }

    let article = ArticleRepository::get_by_id(&state.pool, article_id, Some(user.user_id))
        .await?
        .ok_or_else(|| AppError::NotFound(format!("articulo {article_id} no encontrado")))?;

    Ok(Json(ArticleResponse {
        ok: true,
        data: article,
    }))
}

#[utoipa::path(
    delete,
    path = "/api/articulos/{id}",
    tag = "articles",
    params(("id" = i32, Path, description = "ID del artículo a eliminar")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Artículo eliminado", body = DeleteArticleResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 403, description = "Sin permisos", body = ErrorResponse),
        (status = 404, description = "Artículo no encontrado", body = ErrorResponse)
    )
)]
pub async fn delete_article(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(article_id): Path<i32>,
) -> Result<Json<DeleteArticleResponse>, AppError> {
    let meta = load_manageable_article_meta(&state, &user, article_id).await?;
    let deleted = ArticleRepository::soft_delete(&state.pool, meta.id).await?;
    if !deleted {
        return Err(AppError::NotFound(format!(
            "articulo {article_id} no encontrado"
        )));
    }

    Ok(Json(DeleteArticleResponse {
        ok: true,
        data: DeleteArticleData { eliminado: true },
    }))
}

#[utoipa::path(
    post,
    path = "/api/articulos/{id}/like",
    tag = "articles",
    params(("id" = i32, Path, description = "ID del artículo a likear")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Like alternado", body = ToggleArticleLikeResponse),
        (status = 401, description = "No autenticado", body = ErrorResponse),
        (status = 404, description = "Artículo no encontrado", body = ErrorResponse)
    )
)]
pub async fn toggle_like_article(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(article_id): Path<i32>,
) -> Result<Json<ToggleArticleLikeResponse>, AppError> {
    let meta = ArticleRepository::find_meta_by_id(&state.pool, article_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("articulo {article_id} no encontrado")))?;
    if meta.eliminado_en.is_some() {
        return Err(AppError::NotFound(format!(
            "articulo {article_id} no encontrado"
        )));
    }

    let (liked, total) =
        ArticleRepository::toggle_like(&state.pool, user.user_id, article_id).await?;
    Ok(Json(ToggleArticleLikeResponse {
        ok: true,
        liked,
        total,
    }))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/articulos",
            get(list_articles)
                .post(create_article)
                .layer(DefaultBodyLimit::max(MAX_ARTICLE_MULTIPART_BODY_BYTES)),
        )
        .route("/articulos/categorias", get(list_categories))
        .route("/articulos/mis-articulos", get(list_my_articles))
        .route(
            "/articulos/:article_ref",
            get(get_article).put(update_article).delete(delete_article),
        )
        .route("/articulos/:article_ref/like", post(toggle_like_article))
}
