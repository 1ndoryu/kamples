use std::str::FromStr;

use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;

use crate::errors::AppError;

use crate::repositories::DirectMessageKind;

pub struct ConversationRepository;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ConversationParticipantSummary {
    pub id: i32,
    pub username: String,
    pub nombre_visible: String,
    pub avatar_url: Option<String>,
    pub verificado: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ConversationSummary {
    pub id: i32,
    pub participante: ConversationParticipantSummary,
    pub ultimo_mensaje: String,
    pub ultimo_mensaje_tipo: DirectMessageKind,
    #[schema(value_type = String, format = DateTime)]
    pub ultimo_mensaje_at: chrono::DateTime<chrono::Utc>,
    pub no_leidos: i32,
    pub es_mutuo: bool,
    pub aceptada: bool,
    pub en_linea: bool,
}

#[derive(Debug)]
struct ConversationSummaryRow {
    id: i32,
    aceptada: bool,
    other_id: i32,
    other_username: String,
    other_display_name: String,
    other_avatar_url: Option<String>,
    other_verified: bool,
    last_content: String,
    last_type: String,
    last_at: chrono::DateTime<chrono::Utc>,
    unread_count: i32,
    es_mutuo: bool,
}

impl ConversationRepository {
    pub async fn list_for_user(
        pool: &PgPool,
        user_id: i32,
        hidden_ids: &[i32],
    ) -> Result<Vec<ConversationSummary>, AppError> {
        let rows: Vec<ConversationSummaryRow> = sqlx::query_as!(
            ConversationSummaryRow,
            r#"SELECT
                    c.id AS "id!",
                    COALESCE(c.aceptada, FALSE) AS "aceptada!",
                    u.id AS "other_id!",
                    u.username AS "other_username!",
                    COALESCE(u.nombre_visible, u.username) AS "other_display_name!",
                    u.avatar_url AS "other_avatar_url",
                    COALESCE(u.verificado, FALSE) AS "other_verified!",
                    COALESCE(lm.contenido, '') AS "last_content!",
                    COALESCE(lm.tipo, 'texto') AS "last_type!",
                    COALESCE(lm.created_at, c.created_at) AS "last_at!: chrono::DateTime<chrono::Utc>",
                    COALESCE(nl.total, 0)::int4 AS "unread_count!",
                    (
                        EXISTS(
                            SELECT 1 FROM follows f1
                            WHERE f1.seguidor_id = $1 AND f1.seguido_id = u.id
                        )
                        AND EXISTS(
                            SELECT 1 FROM follows f2
                            WHERE f2.seguidor_id = u.id AND f2.seguido_id = $1
                        )
                    ) AS "es_mutuo!"
               FROM conversaciones c
               JOIN usuarios_ext u
                 ON u.id = CASE WHEN c.participante_1 = $1 THEN c.participante_2 ELSE c.participante_1 END
                AND u.estado = 'activo'
               LEFT JOIN LATERAL (
                    SELECT m.contenido, m.tipo, m.created_at
                    FROM mensajes m
                    WHERE m.conversacion_id = c.id
                    ORDER BY m.id DESC
                    LIMIT 1
               ) lm ON TRUE
               LEFT JOIN LATERAL (
                    SELECT COUNT(*)::int4 AS total
                    FROM mensajes m2
                    WHERE m2.conversacion_id = c.id
                      AND m2.autor_id != $1
                      AND m2.leido = FALSE
               ) nl ON TRUE
               WHERE (c.participante_1 = $1 OR c.participante_2 = $1)
                 AND NOT (u.id = ANY($2::int[]))
               ORDER BY c.ultimo_mensaje_at DESC NULLS LAST, c.id DESC"#,
            user_id,
            hidden_ids,
        )
        .fetch_all(pool)
        .await?;

        rows.into_iter().map(map_conversation_summary_row).collect()
    }

    pub async fn get_for_user(
        pool: &PgPool,
        user_id: i32,
        conversation_id: i32,
        hidden_ids: &[i32],
    ) -> Result<Option<ConversationSummary>, AppError> {
        let row: Option<ConversationSummaryRow> = sqlx::query_as!(
            ConversationSummaryRow,
            r#"SELECT
                    c.id AS "id!",
                    COALESCE(c.aceptada, FALSE) AS "aceptada!",
                    u.id AS "other_id!",
                    u.username AS "other_username!",
                    COALESCE(u.nombre_visible, u.username) AS "other_display_name!",
                    u.avatar_url AS "other_avatar_url",
                    COALESCE(u.verificado, FALSE) AS "other_verified!",
                    COALESCE(lm.contenido, '') AS "last_content!",
                    COALESCE(lm.tipo, 'texto') AS "last_type!",
                    COALESCE(lm.created_at, c.created_at) AS "last_at!: chrono::DateTime<chrono::Utc>",
                    COALESCE(nl.total, 0)::int4 AS "unread_count!",
                    (
                        EXISTS(
                            SELECT 1 FROM follows f1
                            WHERE f1.seguidor_id = $1 AND f1.seguido_id = u.id
                        )
                        AND EXISTS(
                            SELECT 1 FROM follows f2
                            WHERE f2.seguidor_id = u.id AND f2.seguido_id = $1
                        )
                    ) AS "es_mutuo!"
               FROM conversaciones c
               JOIN usuarios_ext u
                 ON u.id = CASE WHEN c.participante_1 = $1 THEN c.participante_2 ELSE c.participante_1 END
                AND u.estado = 'activo'
               LEFT JOIN LATERAL (
                    SELECT m.contenido, m.tipo, m.created_at
                    FROM mensajes m
                    WHERE m.conversacion_id = c.id
                    ORDER BY m.id DESC
                    LIMIT 1
               ) lm ON TRUE
               LEFT JOIN LATERAL (
                    SELECT COUNT(*)::int4 AS total
                    FROM mensajes m2
                    WHERE m2.conversacion_id = c.id
                      AND m2.autor_id != $1
                      AND m2.leido = FALSE
               ) nl ON TRUE
               WHERE c.id = $2
                 AND (c.participante_1 = $1 OR c.participante_2 = $1)
                 AND NOT (u.id = ANY($3::int[]))"#,
            user_id,
            conversation_id,
            hidden_ids,
        )
        .fetch_optional(pool)
        .await?;

        row.map(map_conversation_summary_row).transpose()
    }

    pub async fn find_between_users(
        pool: &PgPool,
        user_a: i32,
        user_b: i32,
    ) -> Result<Option<i32>, AppError> {
        let (participant_1, participant_2) = canonicalize_pair(user_a, user_b);
        let conversation_id = sqlx::query_scalar!(
            r#"SELECT id AS "id!"
               FROM conversaciones
               WHERE participante_1 = $1 AND participante_2 = $2
               LIMIT 1"#,
            participant_1,
            participant_2,
        )
        .fetch_optional(pool)
        .await?;
        Ok(conversation_id)
    }

    pub async fn create(pool: &PgPool, user_a: i32, user_b: i32) -> Result<i32, AppError> {
        let (participant_1, participant_2) = canonicalize_pair(user_a, user_b);
        let conversation_id = sqlx::query_scalar!(
            r#"INSERT INTO conversaciones (participante_1, participante_2)
               VALUES ($1, $2)
               ON CONFLICT (participante_1, participante_2)
               DO UPDATE SET participante_1 = EXCLUDED.participante_1
               RETURNING id AS "id!""#,
            participant_1,
            participant_2,
        )
        .fetch_one(pool)
        .await?;
        Ok(conversation_id)
    }

    pub async fn verify_participation(
        pool: &PgPool,
        conversation_id: i32,
        user_id: i32,
    ) -> Result<bool, AppError> {
        let exists = sqlx::query_scalar!(
            r#"SELECT EXISTS(
                    SELECT 1
                    FROM conversaciones
                    WHERE id = $1
                      AND (participante_1 = $2 OR participante_2 = $2)
                ) AS "exists!""#,
            conversation_id,
            user_id,
        )
        .fetch_one(pool)
        .await?;
        Ok(exists)
    }

    pub async fn other_participant_id(
        pool: &PgPool,
        conversation_id: i32,
        user_id: i32,
    ) -> Result<Option<i32>, AppError> {
        let other_id = sqlx::query_scalar!(
            r#"SELECT CASE WHEN participante_1 = $2 THEN participante_2 ELSE participante_1 END AS "other_id!"
               FROM conversaciones
               WHERE id = $1
                 AND (participante_1 = $2 OR participante_2 = $2)"#,
            conversation_id,
            user_id,
        )
        .fetch_optional(pool)
        .await?;
        Ok(other_id)
    }

    pub async fn mark_accepted(pool: &PgPool, conversation_id: i32) -> Result<(), AppError> {
        sqlx::query!(
            r#"UPDATE conversaciones
               SET aceptada = TRUE
               WHERE id = $1
                 AND aceptada = FALSE"#,
            conversation_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn touch_last_message(pool: &PgPool, conversation_id: i32) -> Result<(), AppError> {
        sqlx::query!(
            r#"UPDATE conversaciones
               SET ultimo_mensaje_at = NOW()
               WHERE id = $1"#,
            conversation_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }
}

fn canonicalize_pair(user_a: i32, user_b: i32) -> (i32, i32) {
    if user_a <= user_b {
        (user_a, user_b)
    } else {
        (user_b, user_a)
    }
}

fn map_conversation_summary_row(
    row: ConversationSummaryRow,
) -> Result<ConversationSummary, AppError> {
    let last_type = DirectMessageKind::from_str(&row.last_type)?;
    let ultimo_mensaje = match last_type {
        DirectMessageKind::Texto => row.last_content,
        DirectMessageKind::Imagen => "[Imagen]".to_string(),
        DirectMessageKind::Audio => "[Audio]".to_string(),
        DirectMessageKind::Sample => {
            if row.last_content.trim().is_empty() {
                "[Sample]".to_string()
            } else {
                format!("[Sample] {}", row.last_content)
            }
        }
    };

    Ok(ConversationSummary {
        id: row.id,
        participante: ConversationParticipantSummary {
            id: row.other_id,
            username: row.other_username,
            nombre_visible: row.other_display_name,
            avatar_url: row.other_avatar_url,
            verificado: row.other_verified,
        },
        ultimo_mensaje,
        ultimo_mensaje_tipo: last_type,
        ultimo_mensaje_at: row.last_at,
        no_leidos: row.unread_count,
        es_mutuo: row.es_mutuo,
        aceptada: row.aceptada,
        en_linea: false,
    })
}
