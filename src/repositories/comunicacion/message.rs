use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;

use crate::errors::AppError;

pub struct MessageRepository;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DirectMessageKind {
    Texto,
    Imagen,
    Audio,
    Sample,
}

impl DirectMessageKind {
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Texto => "texto",
            Self::Imagen => "imagen",
            Self::Audio => "audio",
            Self::Sample => "sample",
        }
    }
}

impl FromStr for DirectMessageKind {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "texto" => Ok(Self::Texto),
            "imagen" => Ok(Self::Imagen),
            "audio" => Ok(Self::Audio),
            "sample" => Ok(Self::Sample),
            other => Err(AppError::Validation(format!(
                "tipo de mensaje inválido: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ConversationMessage {
    pub id: i32,
    pub conversacion_id: i32,
    pub remitente_id: i32,
    pub contenido: String,
    pub tipo: DirectMessageKind,
    pub media_url: Option<String>,
    #[schema(value_type = Option<Object>)]
    pub media_metadata: Option<serde_json::Value>,
    pub leido: bool,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct SharedSampleMessage {
    pub id: i32,
    pub titulo: String,
    pub id_corto: Option<String>,
    pub slug: String,
    pub tipo: String,
    pub bpm: Option<i32>,
    pub music_key: Option<String>,
}

pub struct CreateMessageParams<'a> {
    pub conversacion_id: i32,
    pub autor_id: i32,
    pub contenido: &'a str,
    pub tipo: DirectMessageKind,
    pub media_url: Option<&'a str>,
    pub media_metadata: Option<serde_json::Value>,
}

#[derive(Debug)]
struct ConversationMessageRow {
    id: i32,
    conversacion_id: i32,
    remitente_id: i32,
    contenido: String,
    tipo: String,
    media_url: Option<String>,
    media_metadata: Option<serde_json::Value>,
    leido: bool,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl MessageRepository {
    pub async fn list_by_conversation(
        pool: &PgPool,
        conversacion_id: i32,
        limit: i64,
        before_id: Option<i32>,
    ) -> Result<Vec<ConversationMessage>, AppError> {
        let rows: Vec<ConversationMessageRow> = sqlx::query_as!(
            ConversationMessageRow,
            r#"SELECT ordered.id AS "id!",
                      ordered.conversacion_id AS "conversacion_id!",
                      ordered.autor_id AS "remitente_id!",
                      COALESCE(ordered.contenido, '') AS "contenido!",
                      COALESCE(ordered.tipo, 'texto') AS "tipo!",
                      ordered.media_url AS "media_url",
                      ordered.media_metadata AS "media_metadata?: serde_json::Value",
                      COALESCE(ordered.leido, FALSE) AS "leido!",
                      ordered.created_at AS "created_at!: chrono::DateTime<chrono::Utc>"
               FROM (
                    SELECT m.id,
                           m.conversacion_id,
                           m.autor_id,
                           m.contenido,
                           m.tipo,
                           m.media_url,
                           m.media_metadata,
                           m.leido,
                           m.created_at
                    FROM mensajes m
                    WHERE m.conversacion_id = $1
                      AND ($2::int4 IS NULL OR m.id < $2)
                    ORDER BY m.id DESC
                    LIMIT $3
               ) ordered
               ORDER BY ordered.id ASC"#,
            conversacion_id,
            before_id,
            limit,
        )
        .fetch_all(pool)
        .await?;

        rows.into_iter().map(map_message_row).collect()
    }

    pub async fn count_by_conversation(
        pool: &PgPool,
        conversacion_id: i32,
    ) -> Result<i64, AppError> {
        let total = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!"
               FROM mensajes
               WHERE conversacion_id = $1"#,
            conversacion_id,
        )
        .fetch_one(pool)
        .await?;
        Ok(total)
    }

    pub async fn has_previous(
        pool: &PgPool,
        conversacion_id: i32,
        before_id: i32,
    ) -> Result<bool, AppError> {
        let exists = sqlx::query_scalar!(
            r#"SELECT EXISTS(
                    SELECT 1
                    FROM mensajes
                    WHERE conversacion_id = $1 AND id < $2
                ) AS "exists!""#,
            conversacion_id,
            before_id,
        )
        .fetch_one(pool)
        .await?;
        Ok(exists)
    }

    pub async fn create(pool: &PgPool, params: CreateMessageParams<'_>) -> Result<i32, AppError> {
        let mut tx = pool.begin().await?;

        let message_id = sqlx::query_scalar!(
            r#"INSERT INTO mensajes (
                    conversacion_id,
                    autor_id,
                    contenido,
                    tipo,
                    media_url,
                    media_metadata
               )
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING id AS "id!""#,
            params.conversacion_id,
            params.autor_id,
            params.contenido,
            params.tipo.as_db_str(),
            params.media_url,
            params.media_metadata,
        )
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query!(
            r#"UPDATE conversaciones
               SET ultimo_mensaje_at = NOW(),
                   aceptada = TRUE
               WHERE id = $1"#,
            params.conversacion_id,
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(message_id)
    }

    pub async fn get(
        pool: &PgPool,
        message_id: i32,
    ) -> Result<Option<ConversationMessage>, AppError> {
        let row: Option<ConversationMessageRow> = sqlx::query_as!(
            ConversationMessageRow,
            r#"SELECT m.id AS "id!",
                      m.conversacion_id AS "conversacion_id!",
                      m.autor_id AS "remitente_id!",
                      COALESCE(m.contenido, '') AS "contenido!",
                      COALESCE(m.tipo, 'texto') AS "tipo!",
                      m.media_url AS "media_url",
                      m.media_metadata AS "media_metadata?: serde_json::Value",
                      COALESCE(m.leido, FALSE) AS "leido!",
                      m.created_at AS "created_at!: chrono::DateTime<chrono::Utc>"
               FROM mensajes m
               WHERE m.id = $1"#,
            message_id,
        )
        .fetch_optional(pool)
        .await?;

        row.map(map_message_row).transpose()
    }

    pub async fn mark_read(
        pool: &PgPool,
        conversacion_id: i32,
        user_id: i32,
    ) -> Result<u64, AppError> {
        let updated = sqlx::query!(
            r#"UPDATE mensajes
               SET leido = TRUE
               WHERE conversacion_id = $1
                 AND autor_id != $2
                 AND leido = FALSE"#,
            conversacion_id,
            user_id,
        )
        .execute(pool)
        .await?
        .rows_affected();
        Ok(updated)
    }

    pub async fn mark_all_read_for_user(pool: &PgPool, user_id: i32) -> Result<u64, AppError> {
        let updated = sqlx::query!(
            r#"UPDATE mensajes m
               SET leido = TRUE
               FROM conversaciones c
               WHERE m.conversacion_id = c.id
                 AND (c.participante_1 = $1 OR c.participante_2 = $1)
                 AND m.autor_id != $1
                 AND m.leido = FALSE"#,
            user_id,
        )
        .execute(pool)
        .await?
        .rows_affected();
        Ok(updated)
    }

    pub async fn find_sample_for_share(
        pool: &PgPool,
        sample_id: i32,
    ) -> Result<Option<SharedSampleMessage>, AppError> {
        let sample = sqlx::query_as!(
            SharedSampleMessage,
            r#"SELECT id,
                      titulo AS "titulo!",
                      id_corto,
                      slug AS "slug!",
                      tipo AS "tipo!",
                      bpm,
                      key AS "music_key?"
               FROM samples
               WHERE id = $1
                 AND eliminado_en IS NULL
                 AND estado = 'activo'"#,
            sample_id,
        )
        .fetch_optional(pool)
        .await?;
        Ok(sample)
    }
}

fn map_message_row(row: ConversationMessageRow) -> Result<ConversationMessage, AppError> {
    Ok(ConversationMessage {
        id: row.id,
        conversacion_id: row.conversacion_id,
        remitente_id: row.remitente_id,
        contenido: row.contenido,
        tipo: DirectMessageKind::from_str(&row.tipo)?,
        media_url: row.media_url,
        media_metadata: row.media_metadata,
        leido: row.leido,
        created_at: row.created_at,
    })
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::DirectMessageKind;

    #[test]
    fn parses_direct_message_kind() {
        assert_eq!(
            DirectMessageKind::from_str("texto").expect("texto"),
            DirectMessageKind::Texto
        );
        assert_eq!(
            DirectMessageKind::from_str("IMAGEN").expect("imagen"),
            DirectMessageKind::Imagen
        );
        assert_eq!(
            DirectMessageKind::from_str("audio").expect("audio"),
            DirectMessageKind::Audio
        );
        assert_eq!(
            DirectMessageKind::from_str("sample").expect("sample"),
            DirectMessageKind::Sample
        );
        assert!(DirectMessageKind::from_str("video").is_err());
    }
}
