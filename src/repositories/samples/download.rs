use sqlx::PgPool;

use crate::errors::AppError;

/* [174A-61] DownloadRepository — port mínimo de DescargasRepository.php.
 *
 * Tabla `descargas(id, usuario_id, sample_id, calidad, tamano_bytes, created_at)`.
 *
 * Soporta:
 * - register: insertar fila + incrementar contadores en `samples` y `usuarios_ext`.
 * - already_downloaded: chequea si el usuario descargó este sample antes
 *   (re-descargas no consumen crédito).
 * - count_today: cuenta descargas del usuario en las últimas 24h (para límites).
 *
 * NO portado:
 * - Anti-abuso por IP (RateLimiter::excedeLimiteDescargasIP).
 * - Códigos de descarga gratis (CodigoGratisRepository).
 * - Compras individuales (TransaccionesRepository::haComprado).
 */

pub struct DownloadRepository;

#[derive(Debug, Clone, Copy)]
pub struct SampleDownloadInfo {
    pub creador_id: i32,
    pub permitir_descarga: bool,
    pub es_premium: bool,
    pub precio: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct UserDownloadAllowance {
    pub plan: String,
    pub bonus_credits: i32,
}

impl DownloadRepository {
    pub async fn fetch_sample_info(
        pool: &PgPool,
        sample_id: i32,
    ) -> Result<Option<SampleDownloadInfo>, AppError> {
        let row = sqlx::query!(
            "SELECT creador_id, permitir_descarga, es_premium, precio::float8 AS precio \
             FROM samples WHERE id = $1 AND eliminado_en IS NULL",
            sample_id
        )
        .fetch_optional(pool)
        .await?;
        Ok(row.map(|r| SampleDownloadInfo {
            creador_id: r.creador_id,
            permitir_descarga: r.permitir_descarga.unwrap_or(true),
            es_premium: r.es_premium.unwrap_or(false),
            precio: r.precio,
        }))
    }

    pub async fn already_downloaded(
        pool: &PgPool,
        user_id: i32,
        sample_id: i32,
    ) -> Result<bool, AppError> {
        let r = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM descargas WHERE usuario_id = $1 AND sample_id = $2) AS \"e!\"",
            user_id,
            sample_id,
        )
        .fetch_one(pool)
        .await?;
        Ok(r)
    }

    pub async fn count_today(pool: &PgPool, user_id: i32) -> Result<i64, AppError> {
        let r = sqlx::query_scalar!(
            "SELECT COUNT(*) AS \"c!\" FROM descargas \
             WHERE usuario_id = $1 AND created_at >= NOW() - INTERVAL '24 hours'",
            user_id,
        )
        .fetch_one(pool)
        .await?;
        Ok(r)
    }

    pub async fn register(
        pool: &PgPool,
        user_id: i32,
        sample_id: i32,
        calidad: &str,
    ) -> Result<(), AppError> {
        let mut tx = pool.begin().await?;
        sqlx::query!(
            "INSERT INTO descargas (usuario_id, sample_id, calidad) VALUES ($1, $2, $3)",
            user_id,
            sample_id,
            calidad,
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "UPDATE samples SET total_descargas = total_descargas + 1 WHERE id = $1",
            sample_id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "UPDATE usuarios_ext SET total_descargas = total_descargas + 1 WHERE id = $1",
            user_id
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn user_plan(pool: &PgPool, user_id: i32) -> Result<String, AppError> {
        let plan = sqlx::query_scalar!("SELECT plan FROM usuarios_ext WHERE id = $1", user_id)
            .fetch_optional(pool)
            .await?
            .unwrap_or_else(|| "free".to_string());
        Ok(plan)
    }

    pub async fn user_download_allowance(
        pool: &PgPool,
        user_id: i32,
    ) -> Result<UserDownloadAllowance, AppError> {
        let row = sqlx::query!(
            "SELECT plan, creditos_bonus FROM usuarios_ext WHERE id = $1",
            user_id
        )
        .fetch_optional(pool)
        .await?;

        Ok(row.map_or_else(
            || UserDownloadAllowance {
                plan: "free".to_string(),
                bonus_credits: 0,
            },
            |record| UserDownloadAllowance {
                plan: record.plan,
                bonus_credits: record.creditos_bonus,
            },
        ))
    }

    /* [174A-63] Devuelve datos de archivo para servir vía stream firmado HMAC.
     * Prefiere `ruta_original`; si no existe usa `ruta_optimizada`. */
    pub async fn fetch_file_info(
        pool: &PgPool,
        sample_id: i32,
    ) -> Result<Option<SampleFileInfo>, AppError> {
        let row = sqlx::query!(
            "SELECT titulo, ruta_original, ruta_optimizada \
             FROM samples WHERE id = $1 AND eliminado_en IS NULL",
            sample_id
        )
        .fetch_optional(pool)
        .await?;
        Ok(row.and_then(|r| {
            let key = r.ruta_original.or(r.ruta_optimizada)?;
            Some(SampleFileInfo {
                titulo: r.titulo,
                storage_key: key,
            })
        }))
    }
}

#[derive(Debug, Clone)]
pub struct SampleFileInfo {
    pub titulo: String,
    pub storage_key: String,
}
