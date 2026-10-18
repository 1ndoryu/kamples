/* [174A-66] SavedCollectionsRepository — bookmarks de colecciones ajenas.
 * Tabla `colecciones_guardadas (usuario_id, coleccion_id, created_at)` con PK
 * compuesta — upsert via ON CONFLICT DO NOTHING para idempotencia. */

use sqlx::PgPool;

use crate::errors::AppError;

pub struct SavedCollectionsRepository;

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, utoipa::ToSchema)]
pub struct SavedColeccion {
    pub id: i64,
    pub usuario_id: i32,
    pub nombre: String,
    pub descripcion: Option<String>,
    pub publica: bool,
    pub imagen_url: Option<String>,
    pub total_samples: i32,
    #[schema(value_type = String, format = DateTime)]
    pub guardada_at: chrono::DateTime<chrono::Utc>,
}

impl SavedCollectionsRepository {
    /* Devuelve true si fue insertada (false si ya estaba guardada). */
    pub async fn save(pool: &PgPool, usuario_id: i32, coleccion_id: i64) -> Result<bool, AppError> {
        let res = sqlx::query!(
            "INSERT INTO colecciones_guardadas (usuario_id, coleccion_id) \
             VALUES ($1, $2) ON CONFLICT DO NOTHING",
            usuario_id,
            coleccion_id,
        )
        .execute(pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn unsave(
        pool: &PgPool,
        usuario_id: i32,
        coleccion_id: i64,
    ) -> Result<bool, AppError> {
        let res = sqlx::query!(
            "DELETE FROM colecciones_guardadas WHERE usuario_id = $1 AND coleccion_id = $2",
            usuario_id,
            coleccion_id,
        )
        .execute(pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn is_saved(
        pool: &PgPool,
        usuario_id: i32,
        coleccion_id: i64,
    ) -> Result<bool, AppError> {
        let r = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM colecciones_guardadas
                             WHERE usuario_id = $1 AND coleccion_id = $2) AS "e!""#,
            usuario_id,
            coleccion_id,
        )
        .fetch_one(pool)
        .await?;
        Ok(r)
    }

    pub async fn list_by_user(
        pool: &PgPool,
        usuario_id: i32,
        limite: i64,
        offset: i64,
    ) -> Result<Vec<SavedColeccion>, AppError> {
        let rows = sqlx::query_as!(
            SavedColeccion,
            r#"SELECT c.id AS "id!",
                      c.usuario_id AS "usuario_id!",
                      c.nombre AS "nombre!",
                      c.descripcion,
                      c.publica AS "publica!",
                      c.imagen_url,
                      c.total_samples AS "total_samples!",
                      g.created_at AS "guardada_at!"
               FROM colecciones_guardadas g
               JOIN colecciones c ON c.id = g.coleccion_id
               WHERE g.usuario_id = $1 AND c.eliminado_en IS NULL
               ORDER BY g.created_at DESC
               LIMIT $2 OFFSET $3"#,
            usuario_id,
            limite,
            offset,
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}
