/* [274A-43] Repositorio admin de duplicados: mueve SQL fuera del handler y agrega
 * backfill idempotente sin violar el indice unico de samples.audio_hash.
 * Consultas con macros sqlx: comprobadas al compilar y cacheadas en .sqlx. */

use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};

pub struct AdminDuplicatesRepository;

#[derive(Debug, Clone, FromRow)]
pub struct DuplicateAdminRow {
    pub id: i32,
    pub tipo: String,
    pub estado: String,
    pub created_at: DateTime<Utc>,
    pub original_id: i32,
    pub original_titulo: String,
    pub original_subido_at: DateTime<Utc>,
    pub original_ruta_preview: Option<String>,
    pub original_ruta_waveform: Option<String>,
    pub original_creador: String,
    pub original_creador_id: i32,
    pub original_slug: Option<String>,
    pub original_hash: Option<String>,
    pub duplicado_id: i32,
    pub duplicado_titulo: String,
    pub duplicado_subido_at: DateTime<Utc>,
    pub duplicado_ruta_preview: Option<String>,
    pub duplicado_ruta_waveform: Option<String>,
    pub duplicado_creador: String,
    pub duplicado_creador_id: i32,
    pub duplicado_slug: Option<String>,
    pub duplicado_hash: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct BackfillSampleRow {
    pub id: i32,
    pub creador_id: i32,
    pub ruta_original: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ExistingHashSampleRow {
    pub id: i32,
    pub creador_id: i32,
}

impl AdminDuplicatesRepository {
    pub async fn list(
        pool: &PgPool,
        estado: &str,
        tipo: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<DuplicateAdminRow>, sqlx::Error> {
        sqlx::query_as!(
            DuplicateAdminRow,
            r#"SELECT d.id,
                      d.tipo,
                      d.estado,
                      d.created_at AS "created_at!",
                      so.id              AS original_id,
                      so.titulo          AS original_titulo,
                      so.created_at      AS "original_subido_at!",
                      so.ruta_preview    AS original_ruta_preview,
                      so.ruta_waveform   AS original_ruta_waveform,
                      uo.username        AS original_creador,
                      uo.id              AS original_creador_id,
                      so.slug            AS original_slug,
                      so.audio_hash      AS original_hash,
                      sd.id              AS duplicado_id,
                      sd.titulo          AS duplicado_titulo,
                      sd.created_at      AS "duplicado_subido_at!",
                      sd.ruta_preview    AS duplicado_ruta_preview,
                      sd.ruta_waveform   AS duplicado_ruta_waveform,
                      ud.username        AS duplicado_creador,
                      ud.id              AS duplicado_creador_id,
                      sd.slug            AS duplicado_slug,
                      sd.audio_hash      AS duplicado_hash
                 FROM duplicados_pendientes d
                 JOIN samples so       ON so.id = d.sample_original_id
                 JOIN usuarios_ext uo  ON uo.id = so.creador_id
                 JOIN samples sd       ON sd.id = d.sample_duplicado_id
                 JOIN usuarios_ext ud  ON ud.id = sd.creador_id
                WHERE d.estado = $1
                  AND ($2::text IS NULL OR d.tipo = $2)
                ORDER BY d.created_at DESC
                LIMIT $3 OFFSET $4"#,
            estado,
            tipo,
            limit,
            offset
        )
        .fetch_all(pool)
        .await
    }

    pub async fn count_pending(pool: &PgPool) -> Result<i64, sqlx::Error> {
        let total = sqlx::query_scalar!(
            r#"SELECT COUNT(*)::bigint AS "total!" FROM duplicados_pendientes WHERE estado = 'pendiente'"#
        )
        .fetch_one(pool)
        .await?;

        Ok(total)
    }

    pub async fn list_samples_without_hash(
        pool: &PgPool,
        limit: i64,
    ) -> Result<Vec<BackfillSampleRow>, sqlx::Error> {
        sqlx::query_as!(
            BackfillSampleRow,
            r"SELECT id, creador_id, ruta_original
                 FROM samples
                WHERE audio_hash IS NULL
                  AND estado = 'activo'
                  AND eliminado_en IS NULL
                ORDER BY id ASC
                LIMIT $1",
            limit
        )
        .fetch_all(pool)
        .await
    }

    pub async fn find_existing_hash_owner(
        pool: &PgPool,
        audio_hash: &str,
        excluded_sample_id: i32,
    ) -> Result<Option<ExistingHashSampleRow>, sqlx::Error> {
        sqlx::query_as!(
            ExistingHashSampleRow,
            r"SELECT id, creador_id
                 FROM samples
                WHERE audio_hash = $1
                  AND id != $2
                  AND estado != 'eliminado'
                  AND eliminado_en IS NULL
                ORDER BY CASE estado WHEN 'activo' THEN 0 WHEN 'en_supervision' THEN 1 ELSE 2 END,
                         created_at ASC
                LIMIT 1",
            audio_hash,
            excluded_sample_id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn store_unique_hash(
        pool: &PgPool,
        sample_id: i32,
        audio_hash: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            r"UPDATE samples
                 SET audio_hash = $2,
                     updated_at = NOW()
               WHERE id = $1
                 AND estado = 'activo'
                 AND eliminado_en IS NULL
                 AND NOT EXISTS (
                     SELECT 1
                       FROM samples other
                      WHERE other.audio_hash = $2
                        AND other.id != $1
                        AND other.estado != 'eliminado'
                        AND other.eliminado_en IS NULL
                 )",
            sample_id,
            audio_hash
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn record_duplicate_from_backfill(
        pool: &PgPool,
        original_sample_id: i32,
        duplicate_sample_id: i32,
        duplicate_kind: &str,
    ) -> Result<bool, sqlx::Error> {
        let mut tx = pool.begin().await?;

        let inserted = sqlx::query_scalar!(
            r"INSERT INTO duplicados_pendientes (sample_original_id, sample_duplicado_id, tipo, estado, notas)
                 VALUES ($1, $2, $3, 'pendiente', 'Detectado por backfill de audio_hash en Rust')
                 ON CONFLICT (sample_original_id, sample_duplicado_id) DO NOTHING
                 RETURNING id",
            original_sample_id,
            duplicate_sample_id,
            duplicate_kind
        )
        .fetch_optional(&mut *tx)
        .await?;

        sqlx::query!(
            r"UPDATE samples
                 SET estado = 'en_supervision',
                     updated_at = NOW()
               WHERE id = $1
                 AND estado = 'activo'
                 AND eliminado_en IS NULL",
            duplicate_sample_id
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(inserted.is_some())
    }
}
