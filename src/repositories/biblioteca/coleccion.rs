/* [174A-64] ColeccionesRepository — port mínimo del legado.
 *
 * Tabla `colecciones` (BIGSERIAL id) + `coleccion_samples` (M2M con orden).
 *
 * Funcionalidad:
 * - create / update / soft_delete / fetch / list_by_user
 * - add_sample (con orden auto-incremental)
 * - remove_sample
 * - list_samples (en orden)
 * - is_owner / is_public
 * - check_parent_valid (mismo dueño + profundidad <= 2)
 *
 * NO portado:
 * - Sync changelog.
 * - Subir imagen multipart.
 * - Manejo avanzado de hijas en delete (cascade DB se encarga).
 */

use sqlx::PgPool;

use crate::repositories::SampleContextRow;
use crate::errors::AppError;

mod legacy;

pub use legacy::{LegacyColeccionParentRecord, LegacyColeccionRecord, LegacyColeccionSampleRecord};

pub struct ColeccionesRepository;

/* [296A-1] Resultado de agregar sample a coleccion.
 * Legacy PHP: UNIQUE(usuario_id, sample_id) => 1 sample = 1 coleccion.
 * Si el sample ya existia en otra coleccion, se MOVIA silenciosamente. */
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColeccionOp {
    /// Sample agregado por primera vez.
    Agregado,
    /// Sample ya existia en esta coleccion (no-op).
    YaExiste,
    /// Sample movido desde otra coleccion del mismo usuario.
    Movido { coleccion_anterior: i64 },
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, utoipa::ToSchema)]
pub struct Coleccion {
    pub id: i64,
    pub usuario_id: i32,
    pub nombre: String,
    pub descripcion: Option<String>,
    pub publica: bool,
    pub parent_id: Option<i64>,
    pub imagen_url: Option<String>,
    pub version: i32,
    pub total_samples: i32,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct ColeccionSample {
    pub sample_id: i32,
    pub orden: i32,
    #[schema(value_type = String, format = DateTime)]
    pub added_at: chrono::DateTime<chrono::Utc>,
}

/* [174A-62] Info de un sample lista para empacar en ZIP. */
#[derive(Debug, Clone)]
pub struct ColeccionSampleFile {
    pub sample_id: i32,
    pub titulo: String,
    pub storage_key: String,
    pub es_premium: bool,
    pub creador_id: i32,
}

/* [18AA-2] Cambios de un `UPDATE` de coleccion. Va en struct y no en una lista
 * de argumentos porque son cinco campos opcionales con la misma forma
 * (`Option` = no tocar, `Some(None)` = poner a NULL): pasados sueltos, el
 * llamante puede confundir el orden sin que el compilador lo note.
 * `version` no vive aqui: no es un campo a escribir, es el testigo del bloqueo
 * optimista y `update` lo recibe aparte para que se vea que es otra cosa. */
#[derive(Debug, Clone)]
pub struct CambiosColeccion<'a> {
    pub nombre: Option<&'a str>,
    #[allow(clippy::option_option)]
    pub descripcion: Option<Option<&'a str>>,
    pub publica: Option<bool>,
    #[allow(clippy::option_option)]
    pub imagen_url: Option<Option<&'a str>>,
    #[allow(clippy::option_option)]
    pub parent_id: Option<Option<i64>>,
}

impl ColeccionesRepository {
    pub async fn create(
        pool: &PgPool,
        usuario_id: i32,
        nombre: &str,
        descripcion: Option<&str>,
        publica: bool,
        parent_id: Option<i64>,
    ) -> Result<Coleccion, AppError> {
        if let Some(pid) = parent_id {
            Self::check_parent_valid(pool, usuario_id, pid).await?;
        }
        let row = sqlx::query_as!(
            Coleccion,
            r#"INSERT INTO colecciones (usuario_id, nombre, descripcion, publica, parent_id)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING id, usuario_id, nombre, descripcion, publica AS "publica!",
                         parent_id, imagen_url, version AS "version!",
                         total_samples AS "total_samples!",
                         created_at AS "created_at!", updated_at AS "updated_at!""#,
            usuario_id,
            nombre,
            descripcion,
            publica,
            parent_id,
        )
        .fetch_one(pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db)
                if db.constraint() == Some("idx_colecciones_nombre_unico") =>
            {
                AppError::Conflict(
                    "ya existe una coleccion con ese nombre en la misma ubicacion".into(),
                )
            }
            other => other.into(),
        })?;
        Ok(row)
    }

    pub async fn fetch(pool: &PgPool, id: i64) -> Result<Option<Coleccion>, AppError> {
        let row = sqlx::query_as!(
            Coleccion,
            r#"SELECT id, usuario_id, nombre, descripcion, publica AS "publica!",
                     parent_id, imagen_url, version AS "version!",
                     total_samples AS "total_samples!",
                     created_at AS "created_at!", updated_at AS "updated_at!"
               FROM colecciones WHERE id = $1 AND eliminado_en IS NULL"#,
            id
        )
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn list_by_user(pool: &PgPool, usuario_id: i32) -> Result<Vec<Coleccion>, AppError> {
        let rows = sqlx::query_as!(
            Coleccion,
            r#"SELECT id, usuario_id, nombre, descripcion, publica AS "publica!",
                     parent_id, imagen_url, version AS "version!",
                     total_samples AS "total_samples!",
                     created_at AS "created_at!", updated_at AS "updated_at!"
               FROM colecciones
               WHERE usuario_id = $1 AND eliminado_en IS NULL
               ORDER BY created_at DESC"#,
            usuario_id
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn update(
        pool: &PgPool,
        id: i64,
        cambios: CambiosColeccion<'_>,
        /* [18AA-2] Version que el cliente leyó. `None` = sin comprobacion. */
        version: Option<i32>,
    ) -> Result<bool, AppError> {
        let CambiosColeccion {
            nombre,
            descripcion,
            publica,
            imagen_url,
            parent_id,
        } = cambios;
        /* COALESCE para campos opcionales: si se pasa None, conserva valor actual. */
        let res = sqlx::query!(
            r#"UPDATE colecciones SET
                 nombre        = COALESCE($2, nombre),
                 descripcion   = CASE WHEN $3::bool THEN $4 ELSE descripcion END,
                 publica       = COALESCE($5, publica),
                 imagen_url    = CASE WHEN $6::bool THEN $7 ELSE imagen_url END,
                 parent_id     = CASE WHEN $8::bool THEN $9 ELSE parent_id END,
                 version       = version + 1,
                 updated_at    = NOW()
               WHERE id = $1 AND eliminado_en IS NULL
                 AND ($10::int IS NULL OR version = $10)"#,
            id,
            nombre,
            descripcion.is_some(),
            descripcion.flatten(),
            publica,
            imagen_url.is_some(),
            imagen_url.flatten(),
            parent_id.is_some(),
            parent_id.flatten(),
            version,
        )
        .execute(pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db)
                if db.constraint() == Some("idx_colecciones_nombre_unico") =>
            {
                AppError::Conflict(
                    "ya existe una coleccion con ese nombre en la misma ubicacion".into(),
                )
            }
            other => other.into(),
        })?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn soft_delete(pool: &PgPool, id: i64) -> Result<bool, AppError> {
        let res = sqlx::query!(
            "UPDATE colecciones SET eliminado_en = NOW() WHERE id = $1 AND eliminado_en IS NULL",
            id
        )
        .execute(pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    /* [296A-1] Port de moverAColeccion() legacy.
     * UNIQUE(usuario_id, sample_id) garantiza 1 sample = 1 coleccion por usuario.
     * Si ya existe en otra coleccion, se mueve (DELETE viejo + INSERT nuevo).
     * Si ya existe en esta coleccion, no-op. */
    pub async fn add_sample(
        pool: &PgPool,
        coleccion_id: i64,
        sample_id: i32,
        usuario_id: i32,
    ) -> Result<ColeccionOp, AppError> {
        let mut tx = pool.begin().await?;

        /* 1. Verificar si ya pertenece a ESTA coleccion. */
        let ya_en_esta = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM coleccion_samples WHERE coleccion_id = $1 AND sample_id = $2) AS "e!""#,
            coleccion_id,
            sample_id
        )
        .fetch_one(&mut *tx)
        .await?;
        if ya_en_esta {
            tx.commit().await?;
            return Ok(ColeccionOp::YaExiste);
        }

        /* 2. Buscar si pertenece a OTRA coleccion del mismo usuario. */
        let col_anterior = sqlx::query_scalar!(
            "SELECT cs.coleccion_id FROM coleccion_samples cs \
             WHERE cs.sample_id = $1 AND cs.usuario_id = $2 LIMIT 1",
            sample_id,
            usuario_id
        )
        .fetch_optional(&mut *tx)
        .await?;

        /* 3. Si existe en otra, eliminarlo de la anterior. */
        if let Some(old_col) = col_anterior {
            sqlx::query!(
                "DELETE FROM coleccion_samples WHERE coleccion_id = $1 AND sample_id = $2",
                old_col,
                sample_id
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query!(
                "UPDATE colecciones SET total_samples = GREATEST(total_samples - 1, 0), updated_at = NOW() WHERE id = $1",
                old_col
            )
            .execute(&mut *tx)
            .await?;
        }

        /* 4. Insertar en la nueva coleccion. */
        let next_orden = sqlx::query_scalar!(
            r#"SELECT COALESCE(MAX(orden), -1) + 1 AS "next_orden!" FROM coleccion_samples WHERE coleccion_id = $1"#,
            coleccion_id
        )
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query!(
            "INSERT INTO coleccion_samples (coleccion_id, sample_id, usuario_id, orden) \
             VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
            coleccion_id,
            sample_id,
            usuario_id,
            next_orden
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            "UPDATE colecciones SET total_samples = total_samples + 1, updated_at = NOW() WHERE id = $1",
            coleccion_id
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(match col_anterior {
            Some(old) => ColeccionOp::Movido {
                coleccion_anterior: old,
            },
            None => ColeccionOp::Agregado,
        })
    }

    pub async fn remove_sample(
        pool: &PgPool,
        coleccion_id: i64,
        sample_id: i32,
    ) -> Result<bool, AppError> {
        let mut tx = pool.begin().await?;
        let res = sqlx::query!(
            "DELETE FROM coleccion_samples WHERE coleccion_id = $1 AND sample_id = $2",
            coleccion_id,
            sample_id,
        )
        .execute(&mut *tx)
        .await?;
        let deleted = res.rows_affected() > 0;
        if deleted {
            sqlx::query!(
                "UPDATE colecciones SET total_samples = GREATEST(total_samples - 1, 0), \
                 updated_at = NOW() WHERE id = $1",
                coleccion_id
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(deleted)
    }

    /* [174A-65] Merge: mueve todos los samples de `source_id` a `target_id`
     * (sin duplicados, conserva ORDEN re-asignando incrementalmente al final
     * de target) y soft-deletea source. Ambas colecciones deben pertenecer al
     * mismo usuario, lo cual se valida en el handler antes de llamar.
     * Devuelve el número de samples efectivamente movidos. */
    pub async fn merge(pool: &PgPool, target_id: i64, source_id: i64) -> Result<i64, AppError> {
        if target_id == source_id {
            return Err(AppError::BadRequest(
                "source y target no pueden ser iguales".into(),
            ));
        }
        let mut tx = pool.begin().await?;
        let next_orden: i32 = sqlx::query_scalar!(
            r#"SELECT COALESCE(MAX(orden), -1) + 1 AS "n!" FROM coleccion_samples WHERE coleccion_id = $1"#,
            target_id
        )
        .fetch_one(&mut *tx)
        .await?;
        /* Inserta en target los samples del source que aún no estén en target. */
        let inserted = sqlx::query!(
            r#"INSERT INTO coleccion_samples (coleccion_id, sample_id, orden)
               SELECT $1, s.sample_id,
                      $2 + (ROW_NUMBER() OVER (ORDER BY s.orden, s.added_at) - 1)::int
               FROM coleccion_samples s
               WHERE s.coleccion_id = $3
                 AND NOT EXISTS (
                     SELECT 1 FROM coleccion_samples t
                     WHERE t.coleccion_id = $1 AND t.sample_id = s.sample_id
                 )"#,
            target_id,
            next_orden,
            source_id,
        )
        .execute(&mut *tx)
        .await?;
        let moved = i64::try_from(inserted.rows_affected()).unwrap_or(i64::MAX);
        /* Borra todos los samples de source y soft-deletea la colección. */
        sqlx::query!(
            "DELETE FROM coleccion_samples WHERE coleccion_id = $1",
            source_id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "UPDATE colecciones SET eliminado_en = NOW() WHERE id = $1 AND eliminado_en IS NULL",
            source_id
        )
        .execute(&mut *tx)
        .await?;
        if moved > 0 {
            let moved_i32 = i32::try_from(moved).unwrap_or(i32::MAX);
            sqlx::query!(
                "UPDATE colecciones SET total_samples = total_samples + $2, updated_at = NOW() WHERE id = $1",
                target_id,
                moved_i32,
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(moved)
    }

    pub async fn list_samples(
        pool: &PgPool,
        coleccion_id: i64,
    ) -> Result<Vec<ColeccionSample>, AppError> {
        let rows = sqlx::query_as!(
            ColeccionSample,
            r#"SELECT sample_id, orden, added_at AS "added_at!"
               FROM coleccion_samples
               WHERE coleccion_id = $1
               ORDER BY orden ASC, added_at ASC"#,
            coleccion_id
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /* [174A-62] Lista los samples de una colección con la metadata necesaria
     * para empacar un ZIP: id, título, key de storage (prefiere ruta_original)
     * y flags relevantes para créditos / revenue share. Filtra ya samples
     * soft-deleted y los que no tengan ninguna ruta de archivo disponible. */
    pub async fn list_samples_for_zip(
        pool: &PgPool,
        coleccion_id: i64,
    ) -> Result<Vec<ColeccionSampleFile>, AppError> {
        let rows = sqlx::query!(
            r#"SELECT s.id AS "id!", s.titulo AS "titulo!",
                      s.ruta_original, s.ruta_optimizada,
                      s.es_premium AS "es_premium!", s.creador_id AS "creador_id!"
               FROM coleccion_samples cs
               JOIN samples s ON s.id = cs.sample_id
               WHERE cs.coleccion_id = $1 AND s.eliminado_en IS NULL
               ORDER BY cs.orden ASC, cs.added_at ASC"#,
            coleccion_id,
        )
        .fetch_all(pool)
        .await?;
        let out = rows
            .into_iter()
            .filter_map(|r| {
                let key = r.ruta_original.or(r.ruta_optimizada)?;
                Some(ColeccionSampleFile {
                    sample_id: r.id,
                    titulo: r.titulo,
                    storage_key: key,
                    es_premium: r.es_premium,
                    creador_id: r.creador_id,
                })
            })
            .collect();
        Ok(out)
    }

    /* Verifica que el usuario sea dueño y la coleccion exista (no soft-deleted). */
    pub async fn is_owner(
        pool: &PgPool,
        coleccion_id: i64,
        usuario_id: i32,
    ) -> Result<bool, AppError> {
        let r = sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM colecciones
                             WHERE id = $1 AND usuario_id = $2 AND eliminado_en IS NULL) AS "e!""#,
            coleccion_id,
            usuario_id,
        )
        .fetch_one(pool)
        .await?;
        Ok(r)
    }

    /* Contexto (tags, bpm, key) de las muestras activas de una colección.
     * Movido desde handlers/colecciones.rs: la SQL queda idéntica (caché .sqlx). */
    pub async fn contexto_muestras(
        pool: &PgPool,
        coleccion_id: i64,
    ) -> Result<Vec<SampleContextRow>, AppError> {
        let rows = sqlx::query_as!(
            SampleContextRow,
            r#"
        SELECT
            COALESCE(s.tags, ARRAY[]::text[]) AS "tags!: Vec<String>",
            s.bpm AS "bpm?",
            s.key AS "music_key?"
        FROM samples s
        JOIN coleccion_samples cs ON cs.sample_id = s.id
        WHERE cs.coleccion_id = $1
          AND s.estado = 'activo'
          AND s.eliminado_en IS NULL
        "#,
            coleccion_id
        )
        .fetch_all(pool)
        .await
        .map_err(AppError::from)?;
        Ok(rows)
    }

    /* IDs de las muestras de una colección (para excluirlas de sugerencias). */
    pub async fn ids_muestras(pool: &PgPool, coleccion_id: i64) -> Result<Vec<i32>, AppError> {
        let rows = sqlx::query_scalar!(
            r#"SELECT sample_id AS "sample_id!" FROM coleccion_samples WHERE coleccion_id = $1"#,
            coleccion_id
        )
        .fetch_all(pool)
        .await
        .map_err(AppError::from)?;
        Ok(rows)
    }

    /* Valida que `parent_id` exista, pertenezca al mismo usuario, y que su propio
     * parent sea NULL (profundidad máxima 2 niveles). */
    pub async fn check_parent_valid(
        pool: &PgPool,
        usuario_id: i32,
        parent_id: i64,
    ) -> Result<(), AppError> {
        let row = sqlx::query!(
            "SELECT usuario_id, parent_id FROM colecciones \
             WHERE id = $1 AND eliminado_en IS NULL",
            parent_id
        )
        .fetch_optional(pool)
        .await?;
        match row {
            None => Err(AppError::BadRequest("parent_id no existe".into())),
            Some(r) if r.usuario_id != usuario_id => Err(AppError::Forbidden(
                "parent_id pertenece a otro usuario".into(),
            )),
            Some(r) if r.parent_id.is_some() => Err(AppError::BadRequest(
                "profundidad máxima de carpetas excedida (2 niveles)".into(),
            )),
            Some(_) => Ok(()),
        }
    }
}
