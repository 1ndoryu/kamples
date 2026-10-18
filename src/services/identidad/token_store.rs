/* [174A-20] Almacen de refresh tokens.
 * - Si AppState.redis es Some: persiste en Redis con TTL.
 * - Si es None: fallback en memoria (DashMap) con expiracion lazy.
 * Refresh token = string opaco (32 bytes random base64). */

use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use dashmap::DashMap;
use deadpool_redis::Pool as RedisPool;
use rand::RngCore;
use redis::AsyncCommands;

use crate::errors::AppError;

const REFRESH_TTL_SECS: u64 = 60 * 60 * 24 * 30; // 30 dias
const ACCESS_REVOKE_TTL_SECS: u64 = 60 * 60 * 24; // 24h (igual al exp del JWT)

static MEMORY_REFRESH: LazyLock<Arc<DashMap<String, (i32, Instant)>>> =
    LazyLock::new(|| Arc::new(DashMap::new()));
static MEMORY_REVOKED: LazyLock<Arc<DashMap<String, Instant>>> =
    LazyLock::new(|| Arc::new(DashMap::new()));

pub struct TokenStore;

impl TokenStore {
    /// Genera un token opaco de 32 bytes en base64-url.
    pub fn generate_token() -> String {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        URL_SAFE_NO_PAD.encode(bytes)
    }

    pub async fn save_refresh(
        redis: &Option<RedisPool>,
        token: &str,
        user_id: i32,
    ) -> Result<(), AppError> {
        if let Some(pool) = redis {
            let mut conn = pool
                .get()
                .await
                .map_err(|e| AppError::Internal(format!("Redis pool: {e}")))?;
            let key = format!("refresh:{token}");
            let _: () = conn
                .set_ex(&key, user_id, REFRESH_TTL_SECS)
                .await
                .map_err(|e| AppError::Internal(format!("Redis SETEX: {e}")))?;
        } else {
            MEMORY_REFRESH.insert(
                token.to_string(),
                (
                    user_id,
                    Instant::now() + Duration::from_secs(REFRESH_TTL_SECS),
                ),
            );
            cleanup_memory();
        }
        Ok(())
    }

    /// Consume un refresh token (lectura + borrado atomico). Retorna user_id si valido.
    pub async fn consume_refresh(redis: &Option<RedisPool>, token: &str) -> Result<i32, AppError> {
        if let Some(pool) = redis {
            let mut conn = pool
                .get()
                .await
                .map_err(|e| AppError::Internal(format!("Redis pool: {e}")))?;
            let key = format!("refresh:{token}");
            /* GETDEL es atomico: get + delete en una sola op. */
            let user_id: Option<i32> = redis::cmd("GETDEL")
                .arg(&key)
                .query_async(&mut *conn)
                .await
                .map_err(|e| AppError::Internal(format!("Redis GETDEL: {e}")))?;
            user_id.ok_or(AppError::Unauthorized)
        } else {
            cleanup_memory();
            let entry = MEMORY_REFRESH.remove(token).ok_or(AppError::Unauthorized)?;
            let (uid, exp) = entry.1;
            if Instant::now() > exp {
                return Err(AppError::Unauthorized);
            }
            Ok(uid)
        }
    }

    pub async fn revoke_refresh(redis: &Option<RedisPool>, token: &str) -> Result<(), AppError> {
        if let Some(pool) = redis {
            let mut conn = pool
                .get()
                .await
                .map_err(|e| AppError::Internal(format!("Redis pool: {e}")))?;
            let _: i64 = conn
                .del(format!("refresh:{token}"))
                .await
                .map_err(|e| AppError::Internal(format!("Redis DEL: {e}")))?;
        } else {
            MEMORY_REFRESH.remove(token);
        }
        Ok(())
    }

    /// Marca un access JWT (por su jti) como revocado hasta su expiracion natural.
    pub async fn revoke_access(redis: &Option<RedisPool>, jti: &str) -> Result<(), AppError> {
        if let Some(pool) = redis {
            let mut conn = pool
                .get()
                .await
                .map_err(|e| AppError::Internal(format!("Redis pool: {e}")))?;
            let _: () = conn
                .set_ex(format!("revoked:{jti}"), 1i32, ACCESS_REVOKE_TTL_SECS)
                .await
                .map_err(|e| AppError::Internal(format!("Redis SETEX: {e}")))?;
        } else {
            MEMORY_REVOKED.insert(
                jti.to_string(),
                Instant::now() + Duration::from_secs(ACCESS_REVOKE_TTL_SECS),
            );
        }
        Ok(())
    }

    pub async fn is_access_revoked(redis: &Option<RedisPool>, jti: &str) -> Result<bool, AppError> {
        if let Some(pool) = redis {
            let mut conn = pool
                .get()
                .await
                .map_err(|e| AppError::Internal(format!("Redis pool: {e}")))?;
            let exists: bool = conn
                .exists(format!("revoked:{jti}"))
                .await
                .map_err(|e| AppError::Internal(format!("Redis EXISTS: {e}")))?;
            Ok(exists)
        } else {
            cleanup_memory();
            Ok(MEMORY_REVOKED.get(jti).is_some_and(|e| Instant::now() < *e))
        }
    }
}

/* Limpieza lazy de entradas expiradas (solo modo memoria). */
fn cleanup_memory() {
    let now = Instant::now();
    MEMORY_REFRESH.retain(|_, (_, exp)| *exp > now);
    MEMORY_REVOKED.retain(|_, exp| *exp > now);
}
