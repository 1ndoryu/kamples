use serde::Serialize;
use std::str::FromStr;
use utoipa::ToSchema;

use crate::errors::AppError;

mod escritura;
mod lectura;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentTargetKind {
    Sample,
    Publicacion,
    Cancion,
    Relacion,
    Articulo,
}

impl CommentTargetKind {
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Sample => "sample",
            Self::Publicacion => "publicacion",
            Self::Cancion => "cancion",
            Self::Relacion => "relacion",
            Self::Articulo => "articulo",
        }
    }
}

impl FromStr for CommentTargetKind {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "sample" => Ok(Self::Sample),
            "publicacion" => Ok(Self::Publicacion),
            "cancion" => Ok(Self::Cancion),
            "relacion" => Ok(Self::Relacion),
            "articulo" => Ok(Self::Articulo),
            other => Err(AppError::Validation(format!(
                "tipo de comentario inválido: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentContentKind {
    Texto,
    Imagen,
    Audio,
}

impl CommentContentKind {
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Texto => "texto",
            Self::Imagen => "imagen",
            Self::Audio => "audio",
        }
    }
}

impl FromStr for CommentContentKind {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "texto" => Ok(Self::Texto),
            "imagen" => Ok(Self::Imagen),
            "audio" => Ok(Self::Audio),
            other => Err(AppError::Validation(format!(
                "tipo_contenido inválido: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CreateCommentParams<'a> {
    pub autor_id: i32,
    pub target_kind: CommentTargetKind,
    pub target_id: i32,
    pub contenido: &'a str,
    pub content_kind: CommentContentKind,
    pub media_url: Option<&'a str>,
    pub media_metadata: Option<serde_json::Value>,
    pub parent_id: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct CommentContext {
    pub id: i32,
    pub autor_id: i32,
    pub target_kind: CommentTargetKind,
    pub target_id: i32,
    pub parent_id: Option<i32>,
    pub media_url: Option<String>,
    pub content_kind: CommentContentKind,
}

#[derive(Debug)]
struct CommentContextRow {
    id: i32,
    autor_id: i32,
    tipo: String,
    target_id: i32,
    parent_id: Option<i32>,
    media_url: Option<String>,
    tipo_contenido: String,
}

#[derive(Debug)]
struct CommentRow {
    id: i32,
    autor_id: i32,
    tipo: String,
    target_id: i32,
    contenido: String,
    tipo_contenido: String,
    media_url: Option<String>,
    media_metadata: Option<serde_json::Value>,
    parent_id: Option<i32>,
    total_likes: i32,
    total_respuestas: i32,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: Option<chrono::DateTime<chrono::Utc>>,
    author_id: i32,
    author_username: String,
    author_display_name: Option<String>,
    author_avatar_url: Option<String>,
    author_verified: bool,
    mi_reaccion: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CommentAuthorSummary {
    pub id: i32,
    pub username: String,
    pub nombre_visible: Option<String>,
    pub avatar_url: Option<String>,
    pub verificado: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CommentDetail {
    pub id: i32,
    pub autor_id: i32,
    pub tipo: String,
    pub target_id: i32,
    pub contenido: String,
    pub tipo_contenido: String,
    pub media_url: Option<String>,
    #[schema(value_type = Object)]
    pub media_metadata: Option<serde_json::Value>,
    pub parent_id: Option<i32>,
    pub total_likes: i32,
    pub total_respuestas: i32,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub liked: bool,
    pub mi_reaccion: Option<String>,
    pub autor: CommentAuthorSummary,
}

pub struct CommentRepository;

/* [174A-68] Comentarios polimórficos en un repositorio dedicado para mantener
 * list/get/create/delete con el mismo shape SQL validado en compilación.
 * Gotcha: los contadores viven en tablas distintas según `tipo`, por eso el recount usa ramas explícitas.
 * Pendiente: moderación IA/notificaciones se integrarán en tareas posteriores. */

impl TryFrom<CommentContextRow> for CommentContext {
    type Error = AppError;

    fn try_from(row: CommentContextRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            autor_id: row.autor_id,
            target_kind: CommentTargetKind::from_str(&row.tipo)?,
            target_id: row.target_id,
            parent_id: row.parent_id,
            media_url: row.media_url,
            content_kind: CommentContentKind::from_str(&row.tipo_contenido)?,
        })
    }
}

fn map_comment_row(row: CommentRow) -> CommentDetail {
    CommentDetail {
        id: row.id,
        autor_id: row.autor_id,
        tipo: row.tipo,
        target_id: row.target_id,
        contenido: row.contenido,
        tipo_contenido: row.tipo_contenido,
        media_url: row.media_url,
        media_metadata: row.media_metadata,
        parent_id: row.parent_id,
        total_likes: row.total_likes,
        total_respuestas: row.total_respuestas,
        created_at: row.created_at,
        updated_at: row.updated_at,
        liked: row.mi_reaccion.is_some(),
        mi_reaccion: row.mi_reaccion,
        autor: CommentAuthorSummary {
            id: row.author_id,
            username: row.author_username,
            nombre_visible: row.author_display_name,
            avatar_url: row.author_avatar_url,
            verificado: row.author_verified,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{CommentContentKind, CommentTargetKind};
    use std::str::FromStr;

    #[test]
    fn parses_comment_target_kind() {
        assert_eq!(
            CommentTargetKind::from_str("sample").expect("sample"),
            CommentTargetKind::Sample
        );
        assert_eq!(
            CommentTargetKind::from_str("articulo").expect("articulo"),
            CommentTargetKind::Articulo
        );
        assert!(CommentTargetKind::from_str("otro").is_err());
    }

    #[test]
    fn parses_comment_content_kind() {
        assert_eq!(
            CommentContentKind::from_str("texto").expect("texto"),
            CommentContentKind::Texto
        );
        assert_eq!(
            CommentContentKind::from_str("audio").expect("audio"),
            CommentContentKind::Audio
        );
        assert!(CommentContentKind::from_str("video").is_err());
    }
}
