/* [274A-29+30+32+33+34] Escrituras admin de moderacion.
 * Replica AdminModeracionController.php: cambia estados reales, resuelve reportes,
 * aplica bans manuales y rechazos masivos sin tocar handlers con SQL.
 * Consultas con macros sqlx: comprobadas al compilar y cacheadas en .sqlx. */

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool};
use utoipa::ToSchema;

use crate::errors::AppError;

pub struct AdminModerationRepository;

#[derive(Debug, Clone, Serialize, ToSchema, FromRow)]
pub struct PublicacionPendiente {
    pub id: i32,
    pub autor_id: i32,
    pub tipo: String,
    pub contenido: String,
    pub imagenes: Vec<String>,
    pub samples_adjuntos: Vec<i32>,
    pub moderacion_estado: Option<String>,
    pub moderacion_detalle: Option<serde_json::Value>,
    pub moderacion_razon: Option<String>,
    pub created_at: DateTime<Utc>,
    pub autor_username: String,
    pub autor_nombre: String,
    pub autor_avatar: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema, FromRow)]
pub struct ArticuloPendiente {
    pub id: i32,
    pub titulo: String,
    pub slug: String,
    pub extracto: String,
    pub categoria: String,
    pub portada_url: Option<String>,
    pub moderacion_estado: String,
    pub moderacion_razon: Option<String>,
    pub created_at: DateTime<Utc>,
    pub autor_id: i32,
    pub autor_username: String,
    pub autor_nombre: String,
    pub autor_avatar: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema, FromRow)]
pub struct ReportePendiente {
    pub id: i32,
    pub tipo: String,
    pub target_id: i32,
    pub razon: String,
    pub detalles: Option<String>,
    pub estado: String,
    pub created_at: DateTime<Utc>,
    pub reportador_id: i32,
    pub reportador_username: String,
    pub reportado_id: Option<i32>,
}

impl AdminModerationRepository {
    pub async fn list_pending_posts(
        pool: &PgPool,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<PublicacionPendiente>, AppError> {
        let rows = sqlx::query_as!(
            PublicacionPendiente,
            r#"SELECT p.id, p.autor_id, p.tipo, p.contenido AS "contenido!",
                      COALESCE(p.imagenes, '{}')         AS "imagenes!",
                      COALESCE(p.samples_adjuntos, '{}') AS "samples_adjuntos!",
                      p.moderacion_estado, p.moderacion_detalle, p.moderacion_razon, p.created_at AS "created_at!",
                      u.username AS autor_username,
                      u.nombre_visible AS autor_nombre,
                      u.avatar_url AS autor_avatar
                 FROM publicaciones p
                 JOIN usuarios_ext u ON u.id = p.autor_id
                WHERE p.moderacion_estado IN ('pendiente', 'revision')
                  AND p.eliminado_en IS NULL
                ORDER BY p.created_at DESC
                LIMIT $1 OFFSET $2"#,
            limit,
            offset
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_pending_articles(
        pool: &PgPool,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ArticuloPendiente>, AppError> {
        let rows = sqlx::query_as!(
            ArticuloPendiente,
            r"SELECT a.id, a.titulo, a.slug, a.extracto, a.categoria, a.portada_url,
                      a.moderacion_estado, a.moderacion_razon, a.created_at,
                      a.autor_id,
                      u.username AS autor_username,
                      u.nombre_visible AS autor_nombre,
                      u.avatar_url AS autor_avatar
                 FROM articulos a
                 JOIN usuarios_ext u ON u.id = a.autor_id
                WHERE a.moderacion_estado IN ('pendiente', 'revision')
                  AND a.eliminado_en IS NULL
                ORDER BY a.created_at DESC
                LIMIT $1 OFFSET $2",
            limit,
            offset
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_pending_reports(
        pool: &PgPool,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ReportePendiente>, AppError> {
        let rows = sqlx::query_as!(
            ReportePendiente,
            r#"SELECT r.id, r.tipo, r.target_id, r.razon, r.detalles,
                      r.estado AS "estado!", r.created_at AS "created_at!",
                      r.reportador_id AS "reportador_id!",
                      u.username AS reportador_username,
                      r.reportado_id
                 FROM reportes r
                 JOIN usuarios_ext u ON u.id = r.reportador_id
                WHERE r.estado = 'pendiente'
                ORDER BY r.created_at DESC
                LIMIT $1 OFFSET $2"#,
            limit,
            offset
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn count_pending_reports(pool: &PgPool) -> Result<i64, AppError> {
        let total = sqlx::query_scalar!(
            r#"SELECT COUNT(*)::bigint AS "total!" FROM reportes WHERE estado = 'pendiente'"#
        )
        .fetch_one(pool)
        .await?;
        Ok(total)
    }

    pub async fn list_recent_moderated_posts(
        pool: &PgPool,
        dias: i32,
    ) -> Result<Vec<PublicacionPendiente>, AppError> {
        let rows = sqlx::query_as!(
            PublicacionPendiente,
            r#"SELECT p.id, p.autor_id, p.tipo, p.contenido AS "contenido!",
                      COALESCE(p.imagenes, '{}')         AS "imagenes!",
                      COALESCE(p.samples_adjuntos, '{}') AS "samples_adjuntos!",
                      p.moderacion_estado, p.moderacion_detalle, p.moderacion_razon, p.created_at AS "created_at!",
                      u.username AS autor_username,
                      u.nombre_visible AS autor_nombre,
                      u.avatar_url AS autor_avatar
                 FROM publicaciones p
                 JOIN usuarios_ext u ON u.id = p.autor_id
                WHERE p.moderacion_estado IS NOT NULL
                  AND p.moderacion_estado <> 'pendiente'
                  AND p.created_at >= NOW() - make_interval(days => $1)
                ORDER BY p.created_at DESC
                LIMIT 100"#,
            dias
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn set_post_moderation(
        pool: &PgPool,
        post_id: i32,
        estado: &str,
        razon: Option<&str>,
    ) -> Result<Option<i32>, AppError> {
        let autor_id = sqlx::query_scalar!(
            "UPDATE publicaciones
             SET moderacion_estado = $2,
                 moderacion_razon = COALESCE($3, moderacion_razon)
             WHERE id = $1 AND eliminado_en IS NULL
             RETURNING autor_id",
            post_id,
            estado,
            razon
        )
        .fetch_optional(pool)
        .await?;
        Ok(autor_id)
    }

    pub async fn set_comment_moderation(
        pool: &PgPool,
        comment_id: i32,
        estado: &str,
    ) -> Result<Option<i32>, AppError> {
        let autor_id = sqlx::query_scalar!(
            "UPDATE comentarios
             SET moderacion_estado = $2,
                 updated_at = NOW()
             WHERE id = $1
             RETURNING autor_id",
            comment_id,
            estado
        )
        .fetch_optional(pool)
        .await?;
        Ok(autor_id)
    }

    pub async fn set_article_moderation(
        pool: &PgPool,
        article_id: i32,
        estado: &str,
        razon: Option<&str>,
    ) -> Result<Option<i32>, AppError> {
        let autor_id = sqlx::query_scalar!(
            "UPDATE articulos
             SET moderacion_estado = $2::text,
                 moderacion_razon = CASE WHEN $2::text = 'rechazado' THEN COALESCE($3, 'revision_manual') ELSE moderacion_razon END,
                 publicado_en = CASE WHEN $2::text = 'aprobado' THEN COALESCE(publicado_en, NOW()) ELSE publicado_en END,
                 updated_at = NOW()
             WHERE id = $1 AND eliminado_en IS NULL
             RETURNING autor_id",
            article_id,
            estado,
            razon
        )
        .fetch_optional(pool)
        .await?;
        Ok(autor_id)
    }

    pub async fn reject_pending_posts(pool: &PgPool) -> Result<i64, AppError> {
        let result = sqlx::query!(
            "UPDATE publicaciones
             SET moderacion_estado = 'rechazado',
                 moderacion_razon = 'rechazo_masivo'
             WHERE moderacion_estado IN ('pendiente', 'revision')
               AND eliminado_en IS NULL"
        )
        .execute(pool)
        .await?;
        i64::try_from(result.rows_affected()).map_err(|_| {
            AppError::Internal("Cantidad de publicaciones afectadas fuera de rango".into())
        })
    }

    pub async fn reject_user_posts(pool: &PgPool, author_id: i32) -> Result<i64, AppError> {
        let result = sqlx::query!(
            "UPDATE publicaciones
             SET moderacion_estado = 'rechazado',
                 moderacion_razon = 'rechazo_masivo'
             WHERE autor_id = $1
               AND COALESCE(moderacion_estado, '') <> 'rechazado'
               AND eliminado_en IS NULL",
            author_id
        )
        .execute(pool)
        .await?;
        i64::try_from(result.rows_affected()).map_err(|_| {
            AppError::Internal("Cantidad de publicaciones afectadas fuera de rango".into())
        })
    }

    pub async fn resolve_report(
        pool: &PgPool,
        report_id: i32,
        estado: &str,
        admin_id: i32,
    ) -> Result<bool, AppError> {
        let result = sqlx::query!(
            "UPDATE reportes
             SET estado = $2,
                 resuelto_por = $3,
                 resuelto_at = NOW()
             WHERE id = $1
               AND COALESCE(estado, 'pendiente') = 'pendiente'",
            report_id,
            estado,
            admin_id
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn apply_manual_ban(
        pool: &PgPool,
        user_id: i32,
        ban_until: DateTime<Utc>,
        reason: &str,
    ) -> Result<bool, AppError> {
        let result = sqlx::query!(
            "UPDATE usuarios_ext
             SET baneado_hasta = $2,
                 ban_razon = $3
             WHERE id = $1
               AND estado != 'en_eliminacion'",
            user_id,
            ban_until,
            reason
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
