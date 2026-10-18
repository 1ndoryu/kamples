/* [174A-51] SelectorCandidatos portado del legado Kamples.
 *
 * Pre-filtra ~1000 IDs vía 6 fuentes con index scans rápidos (O(log N))
 * para que el recomendador (174A-52) corra scoring sólo sobre candidatos
 * en lugar de O(N) sobre todo el catálogo.
 *
 * Diferencias con el legado PHP:
 *   - PHP devolvía un fragmento SQL "candidatos AS (...)" para inyectar en
 *     un CTE compuesto. Aquí devolvemos `Vec<i32>` (IDs) y dejamos que el
 *     recomendador filtre por `id = ANY($1::int[])`. Más seguro, testeable
 *     y compatible con macros `query!` validadas en compile-time.
 *   - Filtro bidireccional de bloqueos aplicado en cada fuente (legado lo
 *     dejaba al `MotorRecomendacion` posterior — aquí lo enforzamos antes).
 *
 * 6 fuentes (paridad con `SelectorCandidatos.php`):
 *   1. Trending recientes (últimos N días, score = likes·2 + repro + desc·3).
 *   2. Similares por embedding (ANN pgvector `<=>`), si hay vector de perfil.
 *   3. De creadores seguidos (más recientes por publicado_at).
 *   4. Afinidad por top-tags del usuario (overlap con `samples.tags`).
 *   5. Populares all-time (likes + reproducciones + descargas).
 *   6. No reproducidos por el usuario, aleatorios (frescura garantizada).
 *
 * El conteo de samples activos se cachea (1h) para que el recomendador decida
 * si activar este selector o saltar directo a scoring sobre todo el catálogo. */

use std::collections::HashSet;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use deadpool_redis::Pool as RedisPool;
use pgvector::Vector;
use redis::AsyncCommands;
use sqlx::PgPool;

use crate::errors::AppError;
use crate::repositories::ModerationRepository;

use super::profile::UserProfile;

const COUNT_CACHE_KEY: &str = "kamples_total_samples_activos";
const COUNT_CACHE_TTL_SECS: u64 = 3600;
const ALLOWED_TRENDING_DAYS: [i32; 5] = [7, 14, 30, 60, 90];
const MAX_USER_TAGS: usize = 10;
const TOP_TAGS_LIKES_LIMIT: i64 = 50;

static MEMORY_COUNT_CACHE: LazyLock<Arc<DashMap<String, (i64, Instant)>>> =
    LazyLock::new(|| Arc::new(DashMap::new()));

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CandidatesConfig {
    pub max_trending: i64,
    pub max_embedding: i64,
    pub max_seguidos: i64,
    pub max_tags: i64,
    pub max_populares: i64,
    pub max_nuevos: i64,
    pub dias_trending: i32,
}

impl CandidatesConfig {
    pub const fn legacy_defaults() -> Self {
        Self {
            max_trending: 300,
            max_embedding: 200,
            max_seguidos: 200,
            max_tags: 200,
            max_populares: 100,
            max_nuevos: 150,
            dias_trending: 14,
        }
    }

    /// Whitelist: si `dias_trending` no está en el set permitido, vuelve a 14.
    /// Replicar la salvaguarda del legado para evitar interpolar enteros
    /// arbitrarios en `INTERVAL` aunque vengan de configuración interna.
    #[must_use]
    pub fn safe_dias_trending(&self) -> i32 {
        if ALLOWED_TRENDING_DAYS.contains(&self.dias_trending) {
            self.dias_trending
        } else {
            14
        }
    }
}

impl Default for CandidatesConfig {
    fn default() -> Self {
        Self::legacy_defaults()
    }
}

pub struct CandidatesService;

impl CandidatesService {
    /// Cuenta samples en estado `activo`, cacheado 1h. El recomendador usa
    /// este valor para decidir si activar el selector (cuando el catálogo es
    /// grande) o aplicar scoring sobre todo el set (catálogo pequeño).
    pub async fn count_active(pool: &PgPool, redis: &Option<RedisPool>) -> Result<i64, AppError> {
        if let Some(value) = count_cache_get(redis).await? {
            return Ok(value);
        }
        let row =
            sqlx::query!(r#"SELECT COUNT(*) AS "total!" FROM samples WHERE estado = 'activo'"#)
                .fetch_one(pool)
                .await?;
        count_cache_set(redis, row.total).await?;
        Ok(row.total)
    }

    /// Invalidación explícita del conteo. Llamar al publicar o eliminar.
    pub async fn invalidate_count(redis: &Option<RedisPool>) -> Result<(), AppError> {
        if let Some(pool) = redis {
            let mut conn = pool
                .get()
                .await
                .map_err(|error| AppError::Internal(format!("Redis pool: {error}")))?;
            let _: () = conn
                .del(COUNT_CACHE_KEY)
                .await
                .map_err(|error| AppError::Internal(format!("Redis DEL: {error}")))?;
        } else {
            MEMORY_COUNT_CACHE.remove(COUNT_CACHE_KEY);
        }
        Ok(())
    }

    /// Selecciona los IDs candidatos uniendo las 6 fuentes y aplicando filtro
    /// bidireccional de bloqueos. El orden interno no es relevante: el scoring
    /// posterior reordenará. La deduplicación se hace por `HashSet`.
    pub async fn select(
        pool: &PgPool,
        user_id: i32,
        profile: &UserProfile,
        profile_vector: Option<&Vector>,
        config: &CandidatesConfig,
    ) -> Result<Vec<i32>, AppError> {
        let blocked = collect_blocked_user_ids(pool, user_id).await?;
        let mut candidates: HashSet<i32> = HashSet::new();

        merge(
            &mut candidates,
            fuente_trending(pool, config, &blocked).await?,
        );
        if let Some(vector) = profile_vector {
            merge(
                &mut candidates,
                fuente_embedding(pool, vector, config, &blocked).await?,
            );
        }
        merge(
            &mut candidates,
            fuente_seguidos(pool, user_id, config, &blocked).await?,
        );

        let top_tags = top_user_tags(pool, user_id, profile).await?;
        if !top_tags.is_empty() {
            merge(
                &mut candidates,
                fuente_tags(pool, &top_tags, config, &blocked).await?,
            );
        }

        merge(
            &mut candidates,
            fuente_populares(pool, config, &blocked).await?,
        );
        merge(
            &mut candidates,
            fuente_no_reproducidos(pool, user_id, config, &blocked).await?,
        );

        Ok(candidates.into_iter().collect())
    }
}

fn merge(set: &mut HashSet<i32>, ids: Vec<i32>) {
    set.extend(ids);
}

async fn collect_blocked_user_ids(pool: &PgPool, user_id: i32) -> Result<Vec<i32>, AppError> {
    /* Filtrado bidireccional: a quién bloqueó el usuario + quiénes lo
     * bloquearon a él. Ambas direcciones se excluyen del feed. */
    let mut blocked = ModerationRepository::list_blocked(pool, user_id).await?;
    let blockers = ModerationRepository::list_blockers(pool, user_id).await?;
    blocked.extend(blockers);
    blocked.sort_unstable();
    blocked.dedup();
    Ok(blocked)
}

async fn fuente_trending(
    pool: &PgPool,
    config: &CandidatesConfig,
    blocked: &[i32],
) -> Result<Vec<i32>, AppError> {
    /* Whitelist forzada en `safe_dias_trending`. Aún así pasamos el valor
     * como parámetro `INTERVAL '$N days'` vía interpolación segura: el set
     * sólo admite [7, 14, 30, 60, 90] y se aplica en una rama match abajo. */
    let dias = config.safe_dias_trending();
    /* El intervalo sale de un match exhaustivo sobre constantes, nunca de input
     * externo. Se liga como texto y se convierte con `::text::interval`, porque
     * un bind directo no vale para INTERVAL y la macro exige SQL literal. */
    let intervalo = match dias {
        7 => "7 days",
        30 => "30 days",
        60 => "60 days",
        90 => "90 days",
        _ => "14 days",
    };
    trending_query(pool, intervalo, config.max_trending, blocked).await
}

async fn trending_query(
    pool: &PgPool,
    intervalo: &str,
    limit: i64,
    blocked: &[i32],
) -> Result<Vec<i32>, AppError> {
    let ids = sqlx::query_scalar!(
        r#"SELECT s.id AS "id!"
             FROM samples s
            WHERE s.estado = 'activo'
              AND s.publicado_at > NOW() - $1::text::interval
              AND s.creador_id <> ALL($2::int[])
            ORDER BY (s.total_likes * 2 + s.total_reproducciones + s.total_descargas * 3) DESC
            LIMIT $3"#,
        intervalo,
        blocked,
        limit
    )
    .fetch_all(pool)
    .await?;
    Ok(ids)
}

async fn fuente_embedding(
    pool: &PgPool,
    vector: &Vector,
    config: &CandidatesConfig,
    blocked: &[i32],
) -> Result<Vec<i32>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT s.id AS "id!"
          FROM samples s
         WHERE s.estado = 'activo'
           AND s.embedding IS NOT NULL
           AND s.creador_id <> ALL($1::int[])
         ORDER BY s.embedding <=> $2
         LIMIT $3
        "#,
        blocked,
        vector as &Vector,
        config.max_embedding,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| row.id).collect())
}

async fn fuente_seguidos(
    pool: &PgPool,
    user_id: i32,
    config: &CandidatesConfig,
    blocked: &[i32],
) -> Result<Vec<i32>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT s.id AS "id!"
          FROM samples s
         WHERE s.estado = 'activo'
           AND s.creador_id IN (SELECT seguido_id FROM follows WHERE seguidor_id = $1)
           AND s.creador_id <> ALL($2::int[])
         ORDER BY s.publicado_at DESC NULLS LAST
         LIMIT $3
        "#,
        user_id,
        blocked,
        config.max_seguidos,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| row.id).collect())
}

async fn fuente_tags(
    pool: &PgPool,
    tags: &[String],
    config: &CandidatesConfig,
    blocked: &[i32],
) -> Result<Vec<i32>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT s.id AS "id!"
          FROM samples s
         WHERE s.estado = 'activo'
           AND s.tags && $1::text[]
           AND s.creador_id <> ALL($2::int[])
         ORDER BY s.publicado_at DESC NULLS LAST
         LIMIT $3
        "#,
        tags,
        blocked,
        config.max_tags,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| row.id).collect())
}

async fn fuente_populares(
    pool: &PgPool,
    config: &CandidatesConfig,
    blocked: &[i32],
) -> Result<Vec<i32>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT s.id AS "id!"
          FROM samples s
         WHERE s.estado = 'activo'
           AND s.creador_id <> ALL($1::int[])
         ORDER BY (s.total_likes + s.total_reproducciones + s.total_descargas) DESC
         LIMIT $2
        "#,
        blocked,
        config.max_populares,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| row.id).collect())
}

async fn fuente_no_reproducidos(
    pool: &PgPool,
    user_id: i32,
    config: &CandidatesConfig,
    blocked: &[i32],
) -> Result<Vec<i32>, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT s.id AS "id!"
          FROM samples s
         WHERE s.estado = 'activo'
           AND s.creador_id <> ALL($2::int[])
           AND NOT EXISTS (
               SELECT 1 FROM reproducciones r
                WHERE r.usuario_id = $1
                  AND r.sample_id = s.id
           )
         ORDER BY RANDOM()
         LIMIT $3
        "#,
        user_id,
        blocked,
        config.max_nuevos,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|row| row.id).collect())
}

async fn top_user_tags(
    pool: &PgPool,
    user_id: i32,
    profile: &UserProfile,
) -> Result<Vec<String>, AppError> {
    /* Combina géneros declarados del onboarding (siempre primero) con los
     * tags de los samples más likeados del usuario. Lowercase + dedup + max 10
     * (paridad con `obtenerTopTagsUsuario` legacy). */
    let mut tags: Vec<String> = profile
        .declared_genres
        .iter()
        .map(|raw| raw.trim().to_lowercase())
        .filter(|tag| !tag.is_empty())
        .collect();

    let rows = sqlx::query!(
        r#"
        SELECT DISTINCT LOWER(t) AS "tag!"
          FROM (
              SELECT UNNEST(s.tags) AS t
                FROM likes l
                JOIN samples s ON l.target_id = s.id
               WHERE l.usuario_id = $1
                 AND l.tipo = 'sample'
                 AND l.reaccion IN ('like', 'encanta')
               LIMIT $2
          ) sub
         WHERE t IS NOT NULL AND TRIM(t) <> ''
        "#,
        user_id,
        TOP_TAGS_LIKES_LIMIT,
    )
    .fetch_all(pool)
    .await?;

    for row in rows {
        tags.push(row.tag);
    }

    let mut seen: HashSet<String> = HashSet::new();
    tags.retain(|tag| seen.insert(tag.clone()));
    tags.truncate(MAX_USER_TAGS);
    Ok(tags)
}

async fn count_cache_get(redis: &Option<RedisPool>) -> Result<Option<i64>, AppError> {
    if let Some(pool) = redis {
        let mut conn = pool
            .get()
            .await
            .map_err(|error| AppError::Internal(format!("Redis pool: {error}")))?;
        let value: Option<i64> = conn
            .get(COUNT_CACHE_KEY)
            .await
            .map_err(|error| AppError::Internal(format!("Redis GET: {error}")))?;
        Ok(value)
    } else {
        cleanup_count_memory();
        Ok(MEMORY_COUNT_CACHE
            .get(COUNT_CACHE_KEY)
            .filter(|entry| Instant::now() < entry.value().1)
            .map(|entry| entry.value().0))
    }
}

async fn count_cache_set(redis: &Option<RedisPool>, value: i64) -> Result<(), AppError> {
    if let Some(pool) = redis {
        let mut conn = pool
            .get()
            .await
            .map_err(|error| AppError::Internal(format!("Redis pool: {error}")))?;
        let _: () = conn
            .set_ex(COUNT_CACHE_KEY, value, COUNT_CACHE_TTL_SECS)
            .await
            .map_err(|error| AppError::Internal(format!("Redis SETEX: {error}")))?;
    } else {
        MEMORY_COUNT_CACHE.insert(
            COUNT_CACHE_KEY.to_owned(),
            (
                value,
                Instant::now() + Duration::from_secs(COUNT_CACHE_TTL_SECS),
            ),
        );
        cleanup_count_memory();
    }
    Ok(())
}

fn cleanup_count_memory() {
    let now = Instant::now();
    MEMORY_COUNT_CACHE.retain(|_, (_, expires_at)| *expires_at > now);
}

#[cfg(test)]
mod tests;
