/* [174A-52] Servicio del MotorRecomendacion: feed con cache
 * stale-while-revalidate, cálculo bulk/página, warm async e invalidación. */

use deadpool_redis::Pool as RedisPool;
use redis::AsyncCommands;
use sqlx::PgPool;
use tracing::{debug, warn};

use crate::algorithm::candidates::CandidatesService;
use crate::algorithm::profile::ProfileService;
use crate::errors::AppError;

use super::cache::{
    cache_get, cache_set, fresh_cache_key, fresh_ttl, release_lock, stale_cache_key,
    try_acquire_lock, warm_lock_key, CACHE_PREFIX_FRESH, MEMORY_CACHE,
};
use super::consultas::fetch_enriched;
use super::contratos::{RankedSample, RecommenderConfig};
use super::fallback::fallback_recientes;
use super::puntuacion::{apply_diversity, score_row};

const PAGINAS_BULK: usize = 3;

/* [166A-X] Umbral de samples activos para activar cache distribuido.
 * Por debajo (entorno local/pruebas) se salta Redis para que los
 * samples nuevos aparezcan inmediatamente sin refresh manual. */
const MIN_CACHE_SAMPLES: i64 = 200;

pub struct RecommenderService;

impl RecommenderService {
    /// Punto de entrada del feed personalizado. Política:
    /// 1. Mira cache fresh → si hit, devuelve.
    /// 2. Mira cache stale → si hit, devuelve + spawn warm async.
    /// 3. Computa síncronamente (`compute_feed_bulk`) y devuelve.
    pub async fn feed(
        pool: PgPool,
        redis: Option<RedisPool>,
        user_id: i32,
        limit: usize,
        offset: usize,
        config: &RecommenderConfig,
    ) -> Result<Vec<RankedSample>, AppError> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        /* [174A-57] Profiling fino para usuario QA. No-op para el resto. */
        crate::services::algo_timing::ALGO_TIMING.start(user_id);

        /* [166A-X] Con pocos samples (local/pruebas) saltamos el cache
         * para que el sample nuevo aparezca sin refresh manual. */
        let total_active = CandidatesService::count_active(&pool, &redis).await?;
        let usar_cache = total_active >= MIN_CACHE_SAMPLES;

        if usar_cache {
            let cache_key = fresh_cache_key(user_id, limit, offset);
            if let Some(hit) = cache_get(&redis, &cache_key).await? {
                debug!(target: "kamples.feed", "cache hit fresh: {}", cache_key);
                crate::services::algo_timing::ALGO_TIMING.mark(user_id, "cache_fresh_hit");
                crate::services::algo_timing::ALGO_TIMING.save(
                    user_id,
                    serde_json::json!({"source": "cache_fresh", "items": hit.len()}),
                );
                return Ok(hit);
            }
            crate::services::algo_timing::ALGO_TIMING.mark(user_id, "cache_fresh_miss");

            let stale_key = stale_cache_key(user_id, limit, offset);
            if let Some(stale) = cache_get(&redis, &stale_key).await? {
                debug!(target: "kamples.feed", "cache hit stale: {}", stale_key);
                crate::services::algo_timing::ALGO_TIMING.mark(user_id, "cache_stale_hit");
                spawn_warm(pool.clone(), redis.clone(), user_id, limit, offset, *config);
                crate::services::algo_timing::ALGO_TIMING.save(
                    user_id,
                    serde_json::json!({"source": "cache_stale", "items": stale.len()}),
                );
                return Ok(stale);
            }
            crate::services::algo_timing::ALGO_TIMING.mark(user_id, "cache_stale_miss");
        }

        let items =
            compute_and_cache(&pool, &redis, user_id, limit, offset, config, usar_cache).await?;
        crate::services::algo_timing::ALGO_TIMING.mark(user_id, "compute_and_cache");
        crate::services::algo_timing::ALGO_TIMING.save(
            user_id,
            serde_json::json!({"source": "compute", "items": items.len(), "limit": limit, "offset": offset}),
        );
        Ok(items)
    }
    /// Invalida el cache fresco del feed de un usuario (todas las páginas).
    /// El stale se conserva para servir respuesta instantánea mientras
    /// recalcula. Llamar al dar like, descarga, publicar nuevo sample.
    pub async fn invalidate_user_feed(
        redis: &Option<RedisPool>,
        user_id: i32,
    ) -> Result<(), AppError> {
        let prefix = format!("{CACHE_PREFIX_FRESH}{user_id}_");
        if let Some(pool) = redis {
            let mut conn = pool
                .get()
                .await
                .map_err(|e| AppError::Internal(format!("Redis pool: {e}")))?;
            let keys: Vec<String> = conn
                .keys(format!("{prefix}*"))
                .await
                .map_err(|e| AppError::Internal(format!("Redis KEYS: {e}")))?;
            if !keys.is_empty() {
                let _: () = conn
                    .del(keys)
                    .await
                    .map_err(|e| AppError::Internal(format!("Redis DEL: {e}")))?;
            }
        } else {
            MEMORY_CACHE.retain(|k, _| !k.starts_with(&prefix));
        }
        Ok(())
    }

    /// Variante "global" (todos los usuarios). Útil al publicar sample nuevo
    /// para invalidar todos los feeds frescos. El stale se mantiene.
    pub async fn invalidate_global(redis: &Option<RedisPool>) -> Result<(), AppError> {
        if let Some(pool) = redis {
            let mut conn = pool
                .get()
                .await
                .map_err(|e| AppError::Internal(format!("Redis pool: {e}")))?;
            let keys: Vec<String> = conn
                .keys(format!("{CACHE_PREFIX_FRESH}*"))
                .await
                .map_err(|e| AppError::Internal(format!("Redis KEYS: {e}")))?;
            if !keys.is_empty() {
                let _: () = conn
                    .del(keys)
                    .await
                    .map_err(|e| AppError::Internal(format!("Redis DEL: {e}")))?;
            }
        } else {
            MEMORY_CACHE.retain(|k, _| !k.starts_with(CACHE_PREFIX_FRESH));
        }
        Ok(())
    }
}

async fn compute_and_cache(
    pool: &PgPool,
    redis: &Option<RedisPool>,
    user_id: i32,
    limit: usize,
    offset: usize,
    config: &RecommenderConfig,
    guardar_cache: bool,
) -> Result<Vec<RankedSample>, AppError> {
    let max_bulk_offset = limit * (PAGINAS_BULK - 1);
    let usar_bulk = offset <= max_bulk_offset;

    let pages = if usar_bulk {
        compute_feed_bulk(pool, redis, user_id, limit, config).await?
    } else {
        let page = compute_feed_page(pool, redis, user_id, limit, offset, config).await?;
        vec![(offset, page)]
    };

    let mut response = Vec::new();
    for (page_offset, page) in pages {
        if page_offset == offset {
            response.clone_from(&page);
        }
        if guardar_cache {
            cache_set(
                redis,
                &fresh_cache_key(user_id, limit, page_offset),
                &page,
                fresh_ttl(config, page_offset),
            )
            .await?;
            cache_set(
                redis,
                &stale_cache_key(user_id, limit, page_offset),
                &page,
                config.stale_ttl,
            )
            .await?;
        }
    }
    Ok(response)
}

/// Bulk: calcula `PAGINAS_BULK * limit` candidatos rankeados en una sola
/// pasada y los splittea en páginas. Replica `PAGINAS_BULK = 3` del legado.
async fn compute_feed_bulk(
    pool: &PgPool,
    redis: &Option<RedisPool>,
    user_id: i32,
    limit: usize,
    config: &RecommenderConfig,
) -> Result<Vec<(usize, Vec<RankedSample>)>, AppError> {
    let target = limit * PAGINAS_BULK;
    let ranked = score_pipeline(pool, redis, user_id, target * 2, config).await?;
    let final_ranked = apply_diversity(ranked, config);
    let trimmed: Vec<RankedSample> = final_ranked.into_iter().take(target).collect();

    let mut out = Vec::with_capacity(PAGINAS_BULK);
    for page in 0..PAGINAS_BULK {
        let start = page * limit;
        if start >= trimmed.len() {
            out.push((start, Vec::new()));
            continue;
        }
        let end = (start + limit).min(trimmed.len());
        out.push((start, trimmed[start..end].to_vec()));
    }
    Ok(out)
}

/// Página individual (offset > rango bulk). Calcula `limit*2` para diversidad
/// y trimea al límite solicitado, replicando la rama non-bulk del legado.
async fn compute_feed_page(
    pool: &PgPool,
    redis: &Option<RedisPool>,
    user_id: i32,
    limit: usize,
    offset: usize,
    config: &RecommenderConfig,
) -> Result<Vec<RankedSample>, AppError> {
    let target = (limit + offset) * 2;
    let ranked = score_pipeline(pool, redis, user_id, target, config).await?;
    let final_ranked = apply_diversity(ranked, config);
    Ok(final_ranked.into_iter().skip(offset).take(limit).collect())
}

async fn score_pipeline(
    pool: &PgPool,
    redis: &Option<RedisPool>,
    user_id: i32,
    target: usize,
    config: &RecommenderConfig,
) -> Result<Vec<RankedSample>, AppError> {
    let profile = ProfileService::build(pool, redis, user_id).await?;

    let total_active = CandidatesService::count_active(pool, redis).await?;
    let usar_candidatos = total_active > config.umbral_candidatos;

    let candidate_ids: Option<Vec<i32>> = if usar_candidatos {
        Some(CandidatesService::select(pool, user_id, &profile, None, &config.candidates).await?)
    } else {
        None
    };

    /* Si el usuario es cold-start, atajo: feed por trending recientes
     * (replica `feedNuevoUsuario` del legado, sin scoring complejo). */
    if profile.is_cold_start() {
        return fallback_recientes(pool, user_id, target, candidate_ids.as_deref()).await;
    }

    let rows = fetch_enriched(pool, user_id, candidate_ids.as_deref(), target * 3).await?;
    if rows.is_empty() {
        return fallback_recientes(pool, user_id, target, candidate_ids.as_deref()).await;
    }

    let signal_cfg = config.signal;
    let mut ranked: Vec<RankedSample> = rows
        .into_iter()
        .map(|row| score_row(row, &profile, signal_cfg))
        .collect();

    /* Orden primario: es_nuevo DESC (no reproducidos primero), luego score. */
    ranked.sort_by(|a, b| {
        b.es_nuevo.cmp(&a.es_nuevo).then(
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    Ok(ranked)
}

fn spawn_warm(
    pool: PgPool,
    redis: Option<RedisPool>,
    user_id: i32,
    limit: usize,
    offset: usize,
    config: RecommenderConfig,
) {
    tokio::spawn(async move {
        let lock_key = warm_lock_key(user_id, limit, offset);
        match try_acquire_lock(&redis, &lock_key, config.warm_lock_ttl).await {
            Ok(true) => {}
            Ok(false) => return,
            Err(error) => {
                warn!(target: "kamples.feed", "warm lock error: {error}");
                return;
            }
        }
        /* Warm async siempre guarda en cache (ya supero el umbral). */
        let result = compute_and_cache(&pool, &redis, user_id, limit, offset, &config, true).await;
        if let Err(error) = result {
            warn!(target: "kamples.feed", "warm async fallo user={user_id} limit={limit} offset={offset}: {error}");
        }
        release_lock(&redis, &lock_key).await;
    });
}
