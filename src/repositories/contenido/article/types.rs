use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ArticleEmbed {
    pub tipo: String,
    pub id: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub descarga_publica: Option<bool>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ArticleAuthorSummary {
    pub id: i32,
    pub username: String,
    pub nombre_visible: Option<String>,
    pub avatar_url: Option<String>,
    pub verificado: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ArticleSummary {
    pub id: i32,
    pub autor_id: i32,
    pub titulo: String,
    pub slug: String,
    pub extracto: String,
    pub portada_url: Option<String>,
    pub categoria: String,
    pub total_likes: i32,
    pub total_comentarios: i32,
    pub moderacion_estado: Option<String>,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: DateTime<Utc>,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub publicado_en: Option<DateTime<Utc>>,
    pub autor: ArticleAuthorSummary,
    pub liked_por_mi: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ArticleDetail {
    pub id: i32,
    pub autor_id: i32,
    pub titulo: String,
    pub slug: String,
    pub contenido: String,
    pub extracto: String,
    pub portada_url: Option<String>,
    pub categoria: String,
    pub embeds: Vec<ArticleEmbed>,
    pub descarga_publica: bool,
    pub total_likes: i32,
    pub total_comentarios: i32,
    pub moderacion_estado: String,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: DateTime<Utc>,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub publicado_en: Option<DateTime<Utc>>,
    pub autor: ArticleAuthorSummary,
    pub liked_por_mi: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ArticleCategoryCount {
    pub categoria: String,
    pub total: i64,
}

#[derive(Debug, Clone)]
pub struct CreateArticleParams {
    pub autor_id: i32,
    pub titulo: String,
    pub slug: String,
    pub contenido: String,
    pub extracto: String,
    pub portada_url: Option<String>,
    pub categoria: String,
    pub embeds: serde_json::Value,
    pub descarga_publica: bool,
    pub moderacion_estado: String,
    pub publicado_en: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateArticleParams {
    pub titulo: Option<String>,
    pub slug: Option<String>,
    pub contenido: Option<String>,
    pub extracto: Option<String>,
    pub categoria: Option<String>,
    pub portada_url: Option<String>,
    pub embeds: Option<serde_json::Value>,
    pub descarga_publica: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ArticleMeta {
    pub id: i32,
    pub autor_id: i32,
    pub moderacion_estado: String,
    pub eliminado_en: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub(super) struct ArticleSummaryRow {
    pub(super) id: i32,
    pub(super) autor_id: i32,
    pub(super) titulo: String,
    pub(super) slug: String,
    pub(super) extracto: String,
    pub(super) portada_url: Option<String>,
    pub(super) categoria: String,
    pub(super) total_likes: i32,
    pub(super) total_comentarios: i32,
    pub(super) moderacion_estado: String,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
    pub(super) publicado_en: Option<DateTime<Utc>>,
    pub(super) author_id: i32,
    pub(super) author_username: String,
    pub(super) author_display_name: Option<String>,
    pub(super) author_avatar_url: Option<String>,
    pub(super) author_verified: bool,
    pub(super) liked_por_mi: bool,
}

#[derive(Debug)]
pub(super) struct ArticleDetailRow {
    pub(super) id: i32,
    pub(super) autor_id: i32,
    pub(super) titulo: String,
    pub(super) slug: String,
    pub(super) contenido: String,
    pub(super) extracto: String,
    pub(super) portada_url: Option<String>,
    pub(super) categoria: String,
    pub(super) embeds: serde_json::Value,
    pub(super) descarga_publica: bool,
    pub(super) total_likes: i32,
    pub(super) total_comentarios: i32,
    pub(super) moderacion_estado: String,
    pub(super) created_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
    pub(super) publicado_en: Option<DateTime<Utc>>,
    pub(super) author_id: i32,
    pub(super) author_username: String,
    pub(super) author_display_name: Option<String>,
    pub(super) author_avatar_url: Option<String>,
    pub(super) author_verified: bool,
    pub(super) liked_por_mi: bool,
}

#[derive(Debug)]
pub(super) struct ArticleMetaRow {
    pub(super) id: i32,
    pub(super) autor_id: i32,
    pub(super) moderacion_estado: String,
    pub(super) eliminado_en: Option<DateTime<Utc>>,
}

pub(super) fn map_summary_row(row: ArticleSummaryRow) -> ArticleSummary {
    ArticleSummary {
        id: row.id,
        autor_id: row.autor_id,
        titulo: row.titulo,
        slug: row.slug,
        extracto: row.extracto,
        portada_url: row.portada_url,
        categoria: row.categoria,
        total_likes: row.total_likes,
        total_comentarios: row.total_comentarios,
        moderacion_estado: Some(row.moderacion_estado),
        created_at: row.created_at,
        updated_at: row.updated_at,
        publicado_en: row.publicado_en,
        autor: ArticleAuthorSummary {
            id: row.author_id,
            username: row.author_username,
            nombre_visible: row.author_display_name,
            avatar_url: row.author_avatar_url,
            verificado: row.author_verified,
        },
        liked_por_mi: row.liked_por_mi,
    }
}

pub(super) fn map_detail_row(row: ArticleDetailRow) -> ArticleDetail {
    ArticleDetail {
        id: row.id,
        autor_id: row.autor_id,
        titulo: row.titulo,
        slug: row.slug,
        contenido: row.contenido,
        extracto: row.extracto,
        portada_url: row.portada_url,
        categoria: row.categoria,
        embeds: serde_json::from_value(row.embeds).unwrap_or_default(),
        descarga_publica: row.descarga_publica,
        total_likes: row.total_likes,
        total_comentarios: row.total_comentarios,
        moderacion_estado: row.moderacion_estado,
        created_at: row.created_at,
        updated_at: row.updated_at,
        publicado_en: row.publicado_en,
        autor: ArticleAuthorSummary {
            id: row.author_id,
            username: row.author_username,
            nombre_visible: row.author_display_name,
            avatar_url: row.author_avatar_url,
            verificado: row.author_verified,
        },
        liked_por_mi: row.liked_por_mi,
    }
}
