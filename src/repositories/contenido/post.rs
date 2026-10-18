use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;

use crate::errors::AppError;

pub struct PostRepository;

pub struct PostListParams<'a> {
    pub viewer_id: i32,
    pub only_following: bool,
    pub sort_popular: bool,
    pub author_id: Option<i32>,
    pub blocked_ids: &'a [i32],
    pub limit: i64,
    pub offset: i64,
}

/* [174A-67] La proyección SQL de publicaciones vive aquí para que GET detalle y listado
 * compartan el mismo shape enriquecido sin duplicar joins ni reglas de visibilidad.
 * Gotcha: `sqlx::query_as!` exige una fila plana; por eso `PostRow` conserva flags calculadas por SQL.
 * Pendiente: sumar comentarios agregados cuando 174A-68 porte ese repositorio. */

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug)]
struct PostRow {
    post_id: i32,
    post_autor_id: i32,
    tipo: String,
    contenido: String,
    imagenes: Vec<String>,
    samples_adjuntos: Vec<i32>,
    repost_id: Option<i32>,
    moderacion_estado: String,
    total_likes: i32,
    total_comentarios: i32,
    total_reposts: i32,
    created_at: chrono::DateTime<chrono::Utc>,
    author_id: i32,
    author_username: String,
    author_display_name: Option<String>,
    author_avatar_url: Option<String>,
    author_verified: bool,
    siguiendo_autor: bool,
    mi_reaccion: Option<String>,
    yo_ya_repostee: bool,
    original_id: Option<i32>,
    original_author_id: Option<i32>,
    original_contenido: String,
    original_imagenes: Vec<String>,
    original_samples_adjuntos: Vec<i32>,
    original_total_likes: i32,
    original_total_comentarios: i32,
    original_total_reposts: i32,
    original_created_at: Option<chrono::DateTime<chrono::Utc>>,
    original_author_username: Option<String>,
    original_author_display_name: Option<String>,
    original_author_avatar_url: Option<String>,
    original_author_verified: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PostAuthorSummary {
    pub id: i32,
    pub username: String,
    pub nombre_visible: Option<String>,
    pub avatar_url: Option<String>,
    pub verificado: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RepostedPostSummary {
    pub id: i32,
    pub contenido: String,
    pub imagenes: Vec<String>,
    pub samples_adjuntos: Vec<i32>,
    pub total_likes: i32,
    pub total_comentarios: i32,
    pub total_reposts: i32,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub autor: PostAuthorSummary,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PostDetail {
    pub id: i32,
    pub autor_id: i32,
    pub tipo: String,
    pub contenido: String,
    pub imagenes: Vec<String>,
    pub samples_adjuntos: Vec<i32>,
    pub repost_id: Option<i32>,
    pub moderacion_estado: String,
    pub total_likes: i32,
    pub total_comentarios: i32,
    pub total_reposts: i32,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub autor: PostAuthorSummary,
    pub mi_reaccion: Option<String>,
    pub siguiendo_autor: bool,
    pub yo_ya_repostee: bool,
    pub repost_original: Option<RepostedPostSummary>,
}

impl PostRepository {
    pub async fn create(
        pool: &PgPool,
        autor_id: i32,
        contenido: &str,
        imagenes: &[String],
        samples_adjuntos: &[i32],
    ) -> Result<i32, AppError> {
        let id = sqlx::query_scalar!(
            r#"INSERT INTO publicaciones (
                    autor_id, tipo, contenido, imagenes, samples_adjuntos, moderacion_estado
               ) VALUES ($1, 'social', $2, $3, $4, 'aprobado')
               RETURNING id AS "id!""#,
            autor_id,
            contenido,
            imagenes,
            samples_adjuntos,
        )
        .fetch_one(pool)
        .await?;
        Ok(id)
    }

    pub async fn update_moderation_status(
        pool: &PgPool,
        post_id: i32,
        estado: &str,
        detalle: serde_json::Value,
        razon: &str,
    ) -> Result<bool, AppError> {
        let updated = sqlx::query!(
            r#"UPDATE publicaciones
               SET moderacion_estado = $2,
                   moderacion_detalle = $3,
                   moderacion_razon = $4
               WHERE id = $1
                 AND eliminado_en IS NULL"#,
            post_id,
            estado,
            detalle,
            razon
        )
        .execute(pool)
        .await?
        .rows_affected();
        Ok(updated > 0)
    }

    pub async fn update(
        pool: &PgPool,
        post_id: i32,
        autor_id: i32,
        contenido: &str,
        imagenes: &[String],
        samples_adjuntos: &[i32],
    ) -> Result<bool, AppError> {
        let updated = sqlx::query!(
            r#"UPDATE publicaciones
               SET contenido = $3,
                   imagenes = $4,
                   samples_adjuntos = $5,
                   moderacion_estado = 'aprobado'
               WHERE id = $1
                 AND autor_id = $2
                 AND eliminado_en IS NULL
                 AND repost_id IS NULL"#,
            post_id,
            autor_id,
            contenido,
            imagenes,
            samples_adjuntos,
        )
        .execute(pool)
        .await?
        .rows_affected();
        Ok(updated > 0)
    }

    pub async fn soft_delete(pool: &PgPool, post_id: i32, autor_id: i32) -> Result<bool, AppError> {
        let deleted = sqlx::query!(
            r#"UPDATE publicaciones
               SET eliminado_en = NOW()
               WHERE id = $1
                 AND autor_id = $2
                 AND eliminado_en IS NULL
                 AND repost_id IS NULL"#,
            post_id,
            autor_id,
        )
        .execute(pool)
        .await?
        .rows_affected();
        Ok(deleted > 0)
    }

    pub async fn create_repost(
        pool: &PgPool,
        autor_id: i32,
        original_id: i32,
    ) -> Result<(i32, bool), AppError> {
        let existing = sqlx::query_scalar!(
            r#"SELECT id AS "id!" FROM publicaciones
               WHERE autor_id = $1 AND repost_id = $2 AND eliminado_en IS NULL"#,
            autor_id,
            original_id,
        )
        .fetch_optional(pool)
        .await?;
        if let Some(id) = existing {
            return Ok((id, true));
        }

        let created = sqlx::query_scalar!(
            r#"INSERT INTO publicaciones (
                    autor_id, tipo, contenido, imagenes, samples_adjuntos, repost_id, moderacion_estado
               ) VALUES ($1, 'social', '', $3, $4, $2, 'aprobado')
               ON CONFLICT DO NOTHING
               RETURNING id AS "id!""#,
            autor_id,
            original_id,
            &Vec::<String>::new(),
            &Vec::<i32>::new(),
        )
        .fetch_optional(pool)
        .await?;

        if let Some(id) = created {
            Self::recount_reposts(pool, original_id).await?;
            return Ok((id, false));
        }

        let id = sqlx::query_scalar!(
            r#"SELECT id AS "id!" FROM publicaciones
               WHERE autor_id = $1 AND repost_id = $2 AND eliminado_en IS NULL"#,
            autor_id,
            original_id,
        )
        .fetch_one(pool)
        .await?;
        Ok((id, true))
    }

    pub async fn delete_repost(
        pool: &PgPool,
        autor_id: i32,
        original_id: i32,
    ) -> Result<bool, AppError> {
        let deleted = sqlx::query!(
            r#"DELETE FROM publicaciones
               WHERE autor_id = $1 AND repost_id = $2"#,
            autor_id,
            original_id,
        )
        .execute(pool)
        .await?
        .rows_affected();
        if deleted > 0 {
            Self::recount_reposts(pool, original_id).await?;
        }
        Ok(deleted > 0)
    }

    pub async fn fetch_meta(
        pool: &PgPool,
        post_id: i32,
    ) -> Result<Option<(i32, Option<i32>)>, AppError> {
        let row = sqlx::query!(
            r#"SELECT autor_id AS "autor_id!", repost_id
               FROM publicaciones
               WHERE id = $1 AND eliminado_en IS NULL"#,
            post_id,
        )
        .fetch_optional(pool)
        .await?;
        Ok(row.map(|row| (row.autor_id, row.repost_id)))
    }

    pub async fn all_samples_exist(pool: &PgPool, sample_ids: &[i32]) -> Result<bool, AppError> {
        if sample_ids.is_empty() {
            return Ok(true);
        }
        let count = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!"
               FROM samples
               WHERE id = ANY($1::int[]) AND eliminado_en IS NULL"#,
            sample_ids,
        )
        .fetch_one(pool)
        .await?;
        Ok(count == i64::try_from(sample_ids.len()).unwrap_or(i64::MAX))
    }

    pub async fn get(
        pool: &PgPool,
        viewer_id: i32,
        post_id: i32,
        blocked_ids: &[i32],
    ) -> Result<Option<PostDetail>, AppError> {
        let row = sqlx::query_as!(
            PostRow,
            r#"SELECT
                    p.id AS "post_id!",
                p.autor_id AS "post_autor_id!",
                    COALESCE(p.tipo, 'social') AS "tipo!",
                COALESCE(p.contenido, '') AS "contenido!",
                    p.imagenes AS "imagenes!: Vec<String>",
                    p.samples_adjuntos AS "samples_adjuntos!: Vec<i32>",
                    p.repost_id,
                    COALESCE(p.moderacion_estado, 'aprobado') AS "moderacion_estado!",
                    COALESCE(p.total_likes, 0) AS "total_likes!",
                    COALESCE(p.total_comentarios, 0) AS "total_comentarios!",
                    COALESCE(p.total_reposts, 0) AS "total_reposts!",
                    p.created_at AS "created_at!: chrono::DateTime<chrono::Utc>",
                    u.id AS "author_id!",
                    u.username AS "author_username!",
                    u.nombre_visible AS "author_display_name",
                    u.avatar_url AS "author_avatar_url",
                    COALESCE(u.verificado, FALSE) AS "author_verified!",
                    EXISTS(SELECT 1 FROM follows f WHERE f.seguidor_id = $1 AND f.seguido_id = p.autor_id) AS "siguiendo_autor!",
                    (SELECT reaccion FROM likes WHERE usuario_id = $1 AND tipo = 'publicacion' AND target_id = p.id LIMIT 1) AS "mi_reaccion",
                    EXISTS(SELECT 1 FROM publicaciones mr WHERE mr.autor_id = $1 AND mr.repost_id = COALESCE(p.repost_id, p.id) AND mr.eliminado_en IS NULL) AS "yo_ya_repostee!",
                    op.id AS "original_id?",
                    op.autor_id AS "original_author_id?",
                    COALESCE(op.contenido, '') AS "original_contenido!",
                    COALESCE(op.imagenes, '{}'::text[]) AS "original_imagenes!: Vec<String>",
                    COALESCE(op.samples_adjuntos, '{}'::int[]) AS "original_samples_adjuntos!: Vec<i32>",
                    COALESCE(op.total_likes, 0) AS "original_total_likes!",
                    COALESCE(op.total_comentarios, 0) AS "original_total_comentarios!",
                    COALESCE(op.total_reposts, 0) AS "original_total_reposts!",
                    op.created_at AS "original_created_at?: chrono::DateTime<chrono::Utc>",
                    ou.username AS "original_author_username?",
                    ou.nombre_visible AS "original_author_display_name?",
                    ou.avatar_url AS "original_author_avatar_url?",
                    COALESCE(ou.verificado, FALSE) AS "original_author_verified!"
               FROM publicaciones p
               JOIN usuarios_ext u ON u.id = p.autor_id
               LEFT JOIN publicaciones op ON op.id = p.repost_id AND op.eliminado_en IS NULL
               LEFT JOIN usuarios_ext ou ON ou.id = op.autor_id
               WHERE p.id = $2
                 AND p.eliminado_en IS NULL
                 AND COALESCE(p.moderacion_estado, 'aprobado') <> 'rechazado'
                                 AND (
                                         p.autor_id = $1
                                         OR (
                                                 SELECT COUNT(*)
                                                 FROM reportes r
                                                 WHERE r.tipo = 'publicacion'
                                                     AND COALESCE(r.estado, 'pendiente') = 'pendiente'
                                                     AND r.target_id = p.id
                                         ) < 3
                                 )
                 AND NOT (p.autor_id = ANY($3::int[]))
                                 AND (p.repost_id IS NULL OR op.id IS NOT NULL)
                                 AND (
                                         p.repost_id IS NULL
                                         OR op.autor_id = $1
                                         OR (
                                                 SELECT COUNT(*)
                                                 FROM reportes r
                                                 WHERE r.tipo = 'publicacion'
                                                     AND COALESCE(r.estado, 'pendiente') = 'pendiente'
                                                     AND r.target_id = op.id
                                         ) < 3
                                 )"#,
            viewer_id,
            post_id,
            blocked_ids,
        )
        .fetch_optional(pool)
        .await?;
        Ok(row.map(map_post_row))
    }

    pub async fn list(
        pool: &PgPool,
        params: PostListParams<'_>,
    ) -> Result<Vec<PostDetail>, AppError> {
        let rows = sqlx::query_as!(
            PostRow,
            r#"SELECT
                    p.id AS "post_id!",
                p.autor_id AS "post_autor_id!",
                    COALESCE(p.tipo, 'social') AS "tipo!",
                    COALESCE(p.contenido, '') AS "contenido!",
                    p.imagenes AS "imagenes!: Vec<String>",
                    p.samples_adjuntos AS "samples_adjuntos!: Vec<i32>",
                    p.repost_id,
                    COALESCE(p.moderacion_estado, 'aprobado') AS "moderacion_estado!",
                    COALESCE(p.total_likes, 0) AS "total_likes!",
                    COALESCE(p.total_comentarios, 0) AS "total_comentarios!",
                    COALESCE(p.total_reposts, 0) AS "total_reposts!",
                    p.created_at AS "created_at!: chrono::DateTime<chrono::Utc>",
                    u.id AS "author_id!",
                    u.username AS "author_username!",
                    u.nombre_visible AS "author_display_name",
                    u.avatar_url AS "author_avatar_url",
                    COALESCE(u.verificado, FALSE) AS "author_verified!",
                    EXISTS(SELECT 1 FROM follows f WHERE f.seguidor_id = $1 AND f.seguido_id = p.autor_id) AS "siguiendo_autor!",
                    (SELECT reaccion FROM likes WHERE usuario_id = $1 AND tipo = 'publicacion' AND target_id = p.id LIMIT 1) AS "mi_reaccion",
                    EXISTS(SELECT 1 FROM publicaciones mr WHERE mr.autor_id = $1 AND mr.repost_id = COALESCE(p.repost_id, p.id) AND mr.eliminado_en IS NULL) AS "yo_ya_repostee!",
                    op.id AS "original_id?",
                    op.autor_id AS "original_author_id?",
                    COALESCE(op.contenido, '') AS "original_contenido!",
                    COALESCE(op.imagenes, '{}'::text[]) AS "original_imagenes!: Vec<String>",
                    COALESCE(op.samples_adjuntos, '{}'::int[]) AS "original_samples_adjuntos!: Vec<i32>",
                    COALESCE(op.total_likes, 0) AS "original_total_likes!",
                    COALESCE(op.total_comentarios, 0) AS "original_total_comentarios!",
                    COALESCE(op.total_reposts, 0) AS "original_total_reposts!",
                    op.created_at AS "original_created_at?: chrono::DateTime<chrono::Utc>",
                    ou.username AS "original_author_username?",
                    ou.nombre_visible AS "original_author_display_name?",
                    ou.avatar_url AS "original_author_avatar_url?",
                    COALESCE(ou.verificado, FALSE) AS "original_author_verified!"
               FROM publicaciones p
               JOIN usuarios_ext u ON u.id = p.autor_id
               LEFT JOIN publicaciones op ON op.id = p.repost_id AND op.eliminado_en IS NULL
               LEFT JOIN usuarios_ext ou ON ou.id = op.autor_id
               WHERE p.eliminado_en IS NULL
                 AND COALESCE(p.moderacion_estado, 'aprobado') <> 'rechazado'
                                 AND (
                                         p.autor_id = $1
                                         OR (
                                                 SELECT COUNT(*)
                                                 FROM reportes r
                                                 WHERE r.tipo = 'publicacion'
                                                     AND COALESCE(r.estado, 'pendiente') = 'pendiente'
                                                     AND r.target_id = p.id
                                         ) < 3
                                 )
                 AND ($2::bool = FALSE OR EXISTS(SELECT 1 FROM follows ff WHERE ff.seguidor_id = $1 AND ff.seguido_id = p.autor_id))
                 AND ($3::int IS NULL OR p.autor_id = $3)
                 AND NOT (p.autor_id = ANY($4::int[]))
                 AND (p.repost_id IS NULL OR op.id IS NOT NULL)
                                 AND (
                                         p.repost_id IS NULL
                                         OR op.autor_id = $1
                                         OR (
                                                 SELECT COUNT(*)
                                                 FROM reportes r
                                                 WHERE r.tipo = 'publicacion'
                                                     AND COALESCE(r.estado, 'pendiente') = 'pendiente'
                                                     AND r.target_id = op.id
                                         ) < 3
                                 )
               ORDER BY
                 CASE WHEN $7::bool THEN (COALESCE(p.total_likes, 0) + COALESCE(p.total_comentarios, 0) + COALESCE(p.total_reposts, 0)) ELSE 0 END DESC,
                 p.created_at DESC
               LIMIT $5 OFFSET $6"#,
            params.viewer_id,
            params.only_following,
            params.author_id,
            params.blocked_ids,
            params.limit,
            params.offset,
            params.sort_popular,
        )
        .fetch_all(pool)
        .await?;
        Ok(rows.into_iter().map(map_post_row).collect())
    }

    async fn recount_reposts(pool: &PgPool, original_id: i32) -> Result<(), AppError> {
        sqlx::query!(
            r#"UPDATE publicaciones
               SET total_reposts = (
                   SELECT COUNT(*) FROM publicaciones
                   WHERE repost_id = $1 AND eliminado_en IS NULL
               )
               WHERE id = $1"#,
            original_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }
}

fn map_post_row(row: PostRow) -> PostDetail {
    let repost_original = row.original_id.map(|id| RepostedPostSummary {
        id,
        contenido: row.original_contenido,
        imagenes: row.original_imagenes,
        samples_adjuntos: row.original_samples_adjuntos,
        total_likes: row.original_total_likes,
        total_comentarios: row.original_total_comentarios,
        total_reposts: row.original_total_reposts,
        created_at: row.original_created_at.unwrap_or(row.created_at),
        autor: PostAuthorSummary {
            id: row.original_author_id.unwrap_or_default(),
            username: row.original_author_username.unwrap_or_default(),
            nombre_visible: row.original_author_display_name,
            avatar_url: row.original_author_avatar_url,
            verificado: row.original_author_verified,
        },
    });

    PostDetail {
        id: row.post_id,
        autor_id: row.post_autor_id,
        tipo: row.tipo,
        contenido: row.contenido,
        imagenes: row.imagenes,
        samples_adjuntos: row.samples_adjuntos,
        repost_id: row.repost_id,
        moderacion_estado: row.moderacion_estado,
        total_likes: row.total_likes,
        total_comentarios: row.total_comentarios,
        total_reposts: row.total_reposts,
        created_at: row.created_at,
        autor: PostAuthorSummary {
            id: row.author_id,
            username: row.author_username,
            nombre_visible: row.author_display_name,
            avatar_url: row.author_avatar_url,
            verificado: row.author_verified,
        },
        mi_reaccion: row.mi_reaccion,
        siguiendo_autor: row.siguiendo_autor,
        yo_ya_repostee: row.yo_ya_repostee,
        repost_original,
    }
}
