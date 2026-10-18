/* [174A-52] Cache del MotorRecomendacion: Redis con fallback a memoria
 * (DashMap con TTL), claves del feed y lock del warm async. */

use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use deadpool_redis::Pool as RedisPool;
use redis::AsyncCommands;

use crate::errors::AppError;

use super::contratos::{RankedSample, RecommenderConfig};

pub(super) const CACHE_PREFIX_FRESH: &str = "kamples_feed_";
pub(super) const CACHE_PREFIX_STALE: &str = "kamples_feed_stale_";
pub(super) const WARM_LOCK_PREFIX: &str = "kamples_warm_feed_";

pub(super) static MEMORY_CACHE: LazyLock<Arc<DashMap<String, (Vec<RankedSample>, Instant)>>> =
    LazyLock::new(|| Arc::new(DashMap::new()));
pub(super) static MEMORY_LOCKS: LazyLock<Arc<DashMap<String, Instant>>> =
    LazyLock::new(|| Arc::new(DashMap::new()));

pub(super) fn fresh_cache_key(user_id: i32, limit: usize, offset: usize) -> String {
    format!("{CACHE_PREFIX_FRESH}{user_id}_{limit}_{offset}")
}

pub(super) fn stale_cache_key(user_id: i32, limit: usize, offset: usize) -> String {
    format!("{CACHE_PREFIX_STALE}{user_id}_{limit}_{offset}")
}

pub(super) fn warm_lock_key(user_id: i32, limit: usize, offset: usize) -> String {
    format!("{WARM_LOCK_PREFIX}{user_id}_{limit}_{offset}")
}

pub(super) fn fresh_ttl(config: &RecommenderConfig, page_offset: usize) -> u64 {
    if page_offset == 0 {
        config.fresh_ttl_p0
    } else {
        config.fresh_ttl_pn
    }
}

pub(super) async fn cache_get(
    redis: &Option<RedisPool>,
    key: &str,
) -> Result<Option<Vec<RankedSample>>, AppError> {
    if let Some(pool) = redis {
        let mut conn = pool
            .get()
            .await
            .map_err(|e| AppError::Internal(format!("Redis pool: {e}")))?;
        let raw: Option<String> = conn
            .get(key)
            .await
            .map_err(|e| AppError::Internal(format!("Redis GET: {e}")))?;
        match raw {
            None => Ok(None),
            Some(s) => serde_json::from_str(&s)
                .map(Some)
                .map_err(|e| AppError::Internal(format!("Cache JSON parse: {e}"))),
        }
    } else {
        cleanup_memory();
        Ok(MEMORY_CACHE
            .get(key)
            .filter(|entry| Instant::now() < entry.value().1)
            .map(|entry| entry.value().0.clone()))
    }
}

pub(super) async fn cache_set(
    redis: &Option<RedisPool>,
    key: &str,
    value: &[RankedSample],
    ttl_secs: u64,
) -> Result<(), AppError> {
    if value.is_empty() {
        return Ok(());
    }
    if let Some(pool) = redis {
        let mut conn = pool
            .get()
            .await
            .map_err(|e| AppError::Internal(format!("Redis pool: {e}")))?;
        let json = serde_json::to_string(value)
            .map_err(|e| AppError::Internal(format!("Cache JSON serialize: {e}")))?;
        let _: () = conn
            .set_ex(key, json, ttl_secs)
            .await
            .map_err(|e| AppError::Internal(format!("Redis SETEX: {e}")))?;
    } else {
        MEMORY_CACHE.insert(
            key.to_owned(),
            (
                value.to_vec(),
                Instant::now() + Duration::from_secs(ttl_secs),
            ),
        );
        cleanup_memory();
    }
    Ok(())
}

pub(super) fn cleanup_memory() {
    let now = Instant::now();
    MEMORY_CACHE.retain(|_, (_, expires_at)| *expires_at > now);
    MEMORY_LOCKS.retain(|_, expires_at| *expires_at > now);
}

/// Lock distribuido para warm async. Devuelve `true` si el lock se adquirió.
/// Implementación: SETNX + EX en Redis, o DashMap con TTL en memoria.
pub(super) async fn try_acquire_lock(
    redis: &Option<RedisPool>,
    key: &str,
    ttl_secs: u64,
) -> Result<bool, AppError> {
    if let Some(pool) = redis {
        let mut conn = pool
            .get()
            .await
            .map_err(|e| AppError::Internal(format!("Redis pool: {e}")))?;
        let result: Option<String> = redis::cmd("SET")
            .arg(key)
            .arg("1")
            .arg("NX")
            .arg("EX")
            .arg(ttl_secs)
            .query_async(&mut *conn)
            .await
            .map_err(|e| AppError::Internal(format!("Redis SETNX: {e}")))?;
        Ok(result.is_some())
    } else {
        cleanup_memory();
        let now = Instant::now();
        if let Some(entry) = MEMORY_LOCKS.get(key) {
            if *entry.value() > now {
                return Ok(false);
            }
        }
        MEMORY_LOCKS.insert(key.to_owned(), now + Duration::from_secs(ttl_secs));
        Ok(true)
    }
}

pub(super) async fn release_lock(redis: &Option<RedisPool>, key: &str) {
    if let Some(pool) = redis {
        if let Ok(mut conn) = pool.get().await {
            let _: Result<i32, _> = conn.del(key).await;
        }
    } else {
        MEMORY_LOCKS.remove(key);
    }
}
