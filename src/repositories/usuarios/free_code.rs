use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::errors::AppError;

/* [174A-84] FreeCodeRepository — port de CodigoGratisRepository.php.
 *
 * Tablas:
 * - `codigos_descarga_gratis`: códigos activos para sample/colección.
 * - `codigos_gratis_usos`: reclamos idempotentes por usuario.
 *
 * Reglas portadas:
 * - generate: crea código hex único y guarda `nombre_item` para UX.
 * - verify: busca códigos activos aunque estén expirados.
 * - claim: ON CONFLICT DO NOTHING (idempotente).
 * - compensation: si expiró, acredita bonus una sola vez en transacción.
 * - invalidate: desactiva todos los códigos activos de un tipo+target.
 */

#[derive(Debug, Clone)]
pub struct CreateFreeCodeInput<'a> {
    pub codigo: &'a str,
    pub tipo: &'a str,
    pub target_id: i64,
    pub creado_por_id: i32,
    pub nombre_item: &'a str,
}

#[derive(Debug, Clone)]
pub struct FreeCodeRecord {
    pub id: i64,
    pub codigo: String,
    pub tipo: String,
    pub target_id: i64,
    pub nombre_item: String,
    pub expires_at: DateTime<Utc>,
}

pub struct FreeCodeRepository;

impl FreeCodeRepository {
    pub async fn create(pool: &PgPool, input: &CreateFreeCodeInput<'_>) -> Result<i64, AppError> {
        let id = sqlx::query_scalar!(
            r#"
            INSERT INTO codigos_descarga_gratis (codigo, tipo, target_id, creado_por_id, nombre_item)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id AS "id!"
            "#,
            input.codigo,
            input.tipo,
            input.target_id,
            i64::from(input.creado_por_id),
            input.nombre_item,
        )
        .fetch_one(pool)
        .await?;

        Ok(id)
    }

    pub async fn find_active(
        pool: &PgPool,
        codigo: &str,
    ) -> Result<Option<FreeCodeRecord>, AppError> {
        let row = sqlx::query_as!(
            FreeCodeRecord,
            r#"
            SELECT
                id AS "id!",
                codigo AS "codigo!",
                tipo AS "tipo!",
                target_id AS "target_id!",
                nombre_item AS "nombre_item!",
                expires_at AS "expires_at!"
            FROM codigos_descarga_gratis
            WHERE codigo = $1
              AND activo = TRUE
            LIMIT 1
            "#,
            codigo,
        )
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    pub async fn register_claim(
        pool: &PgPool,
        codigo_id: i64,
        usuario_id: i32,
    ) -> Result<bool, AppError> {
        let inserted = sqlx::query_scalar!(
            r#"
            INSERT INTO codigos_gratis_usos (codigo_id, usuario_id)
            VALUES ($1, $2)
            ON CONFLICT (codigo_id, usuario_id) DO NOTHING
            RETURNING id AS "id!"
            "#,
            codigo_id,
            i64::from(usuario_id),
        )
        .fetch_optional(pool)
        .await?
        .is_some();

        Ok(inserted)
    }

    pub async fn compensate_expired_claim(
        pool: &PgPool,
        codigo_id: i64,
        usuario_id: i32,
        creditos_bonus: i32,
    ) -> Result<bool, AppError> {
        let mut tx = pool.begin().await?;

        let inserted = sqlx::query_scalar!(
            r#"
            INSERT INTO codigos_gratis_usos (codigo_id, usuario_id, expirado)
            VALUES ($1, $2, TRUE)
            ON CONFLICT (codigo_id, usuario_id) DO NOTHING
            RETURNING id AS "id!"
            "#,
            codigo_id,
            i64::from(usuario_id),
        )
        .fetch_optional(&mut *tx)
        .await?
        .is_some();

        if inserted {
            sqlx::query!(
                r#"
                UPDATE usuarios_ext
                SET creditos_bonus = creditos_bonus + $2
                WHERE id = $1
                "#,
                usuario_id,
                creditos_bonus,
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok(inserted)
    }

    pub async fn can_user_download(
        pool: &PgPool,
        codigo: &str,
        tipo: &str,
        target_id: i64,
        usuario_id: i32,
    ) -> Result<bool, AppError> {
        let allowed = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM codigos_descarga_gratis cdg
                INNER JOIN codigos_gratis_usos cgu ON cgu.codigo_id = cdg.id
                WHERE cdg.codigo = $1
                  AND cdg.tipo = $2
                  AND cdg.target_id = $3
                  AND cgu.usuario_id = $4
                  AND cgu.expirado = FALSE
                  AND cdg.activo = TRUE
                  AND cdg.expires_at > NOW()
            ) AS "allowed!"
            "#,
            codigo,
            tipo,
            target_id,
            i64::from(usuario_id),
        )
        .fetch_one(pool)
        .await?;

        Ok(allowed)
    }

    pub async fn invalidate_target(
        pool: &PgPool,
        tipo: &str,
        target_id: i64,
    ) -> Result<i64, AppError> {
        let result = sqlx::query!(
            r#"
            UPDATE codigos_descarga_gratis
            SET activo = FALSE
            WHERE tipo = $1
              AND target_id = $2
              AND activo = TRUE
            "#,
            tipo,
            target_id,
        )
        .execute(pool)
        .await?;

        Ok(i64::try_from(result.rows_affected()).unwrap_or(i64::MAX))
    }
}
