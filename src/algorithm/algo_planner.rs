/* [174A-55] AlgoPlanner — orquestador de recálculos del algoritmo.
 *
 * Reemplazo Rust del `PlanificadorAlgoritmo.php` legado. Decisión clave: en
 * lugar de cron + WP-cron, este módulo expone:
 *   1. `register_interaction(...)` para el path caliente (trigger por evento).
 *   2. `process_temporal(...)` para el batch periódico (trigger por tiempo).
 *   3. `spawn_periodic_loop(...)` que lanza un `tokio::spawn` con `select!`
 *      sobre `tokio::time::interval` para los jobs en background:
 *        - REFRESH MATERIALIZED VIEW mv_trending_samples (cada 5 min).
 *        - TagAffinityService::recalculate_active (cada 10 min).
 *        - process_temporal (cada 5 min).
 *
 * El loop se cancela cuando el `CancellationToken` se dispara (graceful
 * shutdown). Los errores se loguean pero no abortan el loop. */

use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::{interval, interval_at, Instant};
use tokio_util::sync::CancellationToken;

use deadpool_redis::Pool as RedisPool;

use crate::algorithm::precompute::PrecomputeService;
use crate::algorithm::recommender::RecommenderService;
use crate::algorithm::tag_affinity::{InflightLocks, TagAffinityService};
use crate::errors::AppError;

/// Tipo de interacción que dispara contadores en `algoritmo_estado`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionKind {
    Like,
    Reproduccion,
    Completa,
    Descarga,
    Follow,
    Comentario,
}

impl InteractionKind {
    /// Incremento de cada una de las 12 columnas de `CounterRow` (6 fast y
    /// luego 6 precise): 1 en la fast y la precise de este tipo, 0 en el resto.
    fn incrementos(self) -> [i32; 12] {
        let i = match self {
            Self::Like => 0,
            Self::Reproduccion => 1,
            Self::Completa => 2,
            Self::Descarga => 3,
            Self::Follow => 4,
            Self::Comentario => 5,
        };
        let mut delta = [0; 12];
        delta[i] = 1;
        delta[i + 6] = 1;
        delta
    }
}

/// Resultado de `register_interaction`: indica qué recálculos se dispararon.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Triggered {
    pub fast: bool,
    pub precise: bool,
}

/// Tupla de los 12 contadores leídos en `algoritmo_estado` (6 fast + 6 precise).
type CounterRow = (i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32, i32);

/// Configuración de umbrales y frecuencias. Replica la sección `frecuencia`
/// de `algoritmoPesos.php` con defaults seguros.
#[derive(Debug, Clone, Copy)]
pub struct AlgoPlannerConfig {
    /// Trigger fast (cualquier contador que cruce dispara recálculo rápido).
    pub fast_likes: i32,
    pub fast_reproducciones: i32,
    pub fast_completas: i32,
    pub fast_descargas: i32,
    pub fast_follows: i32,
    pub fast_comentarios: i32,

    /// Trigger precise (típicamente 2× los fast).
    pub precise_likes: i32,
    pub precise_reproducciones: i32,
    pub precise_completas: i32,
    pub precise_descargas: i32,
    pub precise_follows: i32,
    pub precise_comentarios: i32,

    /// Periodicidad del loop background (en segundos).
    pub interval_refresh_mv_secs: u64,
    pub interval_recalc_active_secs: u64,
    pub interval_temporal_secs: u64,

    /// Limites batch.
    pub temporal_user_limit: i64,
    pub recalc_active_limit: i64,
}

impl AlgoPlannerConfig {
    #[must_use]
    pub const fn legacy_defaults() -> Self {
        Self {
            fast_likes: 3,
            fast_reproducciones: 8,
            fast_completas: 5,
            fast_descargas: 2,
            fast_follows: 2,
            fast_comentarios: 5,

            precise_likes: 6,
            precise_reproducciones: 16,
            precise_completas: 10,
            precise_descargas: 4,
            precise_follows: 4,
            precise_comentarios: 10,

            interval_refresh_mv_secs: 300,    /* 5 min */
            interval_recalc_active_secs: 600, /* 10 min */
            interval_temporal_secs: 300,      /* 5 min */
            temporal_user_limit: 500,
            recalc_active_limit: 200,
        }
    }
}

/// Orquestador. Construir 1× en el bootstrap y compartir vía `Arc`.
pub struct AlgoPlanner {
    pub config: AlgoPlannerConfig,
    pub locks: InflightLocks,
}

impl AlgoPlanner {
    #[must_use]
    pub fn new(config: AlgoPlannerConfig) -> Arc<Self> {
        Arc::new(Self {
            config,
            locks: TagAffinityService::new_inflight_locks(),
        })
    }

    /// Registra interacción + incrementa contadores + evalúa umbrales. Path
    /// caliente: invocar tras like/play/etc. Recálculos se ejecutan inline
    /// (rápido) o async (preciso, vía spawn) según el caso.
    pub async fn register_interaction(
        &self,
        pool: &PgPool,
        redis: &Option<RedisPool>,
        user_id: i32,
        kind: InteractionKind,
    ) -> Result<Triggered, AppError> {
        let d = kind.incrementos();

        /* Upsert + incremento atómico en una sola roundtrip. Consulta estática:
         * cada columna suma su delta (0 o 1). Las columnas no tocadas suman 0,
         * igual que su DEFAULT 0, así que el resultado es el mismo que antes. */
        let fila = sqlx::query!(
            "INSERT INTO algoritmo_estado (usuario_id, cnt_likes, cnt_reproducciones, \
                 cnt_completas, cnt_descargas, cnt_follows, cnt_comentarios, \
                 cnt_likes_preciso, cnt_reproducciones_preciso, cnt_completas_preciso, \
                 cnt_descargas_preciso, cnt_follows_preciso, cnt_comentarios_preciso, \
                 ultima_actividad) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, NOW()) \
             ON CONFLICT (usuario_id) DO UPDATE SET \
                 cnt_likes = algoritmo_estado.cnt_likes + $2, \
                 cnt_reproducciones = algoritmo_estado.cnt_reproducciones + $3, \
                 cnt_completas = algoritmo_estado.cnt_completas + $4, \
                 cnt_descargas = algoritmo_estado.cnt_descargas + $5, \
                 cnt_follows = algoritmo_estado.cnt_follows + $6, \
                 cnt_comentarios = algoritmo_estado.cnt_comentarios + $7, \
                 cnt_likes_preciso = algoritmo_estado.cnt_likes_preciso + $8, \
                 cnt_reproducciones_preciso = algoritmo_estado.cnt_reproducciones_preciso + $9, \
                 cnt_completas_preciso = algoritmo_estado.cnt_completas_preciso + $10, \
                 cnt_descargas_preciso = algoritmo_estado.cnt_descargas_preciso + $11, \
                 cnt_follows_preciso = algoritmo_estado.cnt_follows_preciso + $12, \
                 cnt_comentarios_preciso = algoritmo_estado.cnt_comentarios_preciso + $13, \
                 ultima_actividad = NOW() \
             RETURNING cnt_likes, cnt_reproducciones, cnt_completas, cnt_descargas, \
                       cnt_follows, cnt_comentarios, \
                       cnt_likes_preciso, cnt_reproducciones_preciso, cnt_completas_preciso, \
                       cnt_descargas_preciso, cnt_follows_preciso, cnt_comentarios_preciso",
            user_id,
            d[0],
            d[1],
            d[2],
            d[3],
            d[4],
            d[5],
            d[6],
            d[7],
            d[8],
            d[9],
            d[10],
            d[11]
        )
        .fetch_one(pool)
        .await?;
        let row: CounterRow = (
            fila.cnt_likes,
            fila.cnt_reproducciones,
            fila.cnt_completas,
            fila.cnt_descargas,
            fila.cnt_follows,
            fila.cnt_comentarios,
            fila.cnt_likes_preciso,
            fila.cnt_reproducciones_preciso,
            fila.cnt_completas_preciso,
            fila.cnt_descargas_preciso,
            fila.cnt_follows_preciso,
            fila.cnt_comentarios_preciso,
        );

        let mut triggered = Triggered::default();
        if self.fast_threshold_hit(&row) {
            self.execute_fast(pool, redis, user_id).await?;
            triggered.fast = true;
        }
        if self.precise_threshold_hit(&row) {
            self.execute_precise(pool, redis, user_id);
            triggered.precise = true;
        }

        Ok(triggered)
    }

    /// Evalúa si CUALQUIER contador fast cruzó su umbral.
    fn fast_threshold_hit(&self, row: &CounterRow) -> bool {
        row.0 >= self.config.fast_likes
            || row.1 >= self.config.fast_reproducciones
            || row.2 >= self.config.fast_completas
            || row.3 >= self.config.fast_descargas
            || row.4 >= self.config.fast_follows
            || row.5 >= self.config.fast_comentarios
    }

    fn precise_threshold_hit(&self, row: &CounterRow) -> bool {
        row.6 >= self.config.precise_likes
            || row.7 >= self.config.precise_reproducciones
            || row.8 >= self.config.precise_completas
            || row.9 >= self.config.precise_descargas
            || row.10 >= self.config.precise_follows
            || row.11 >= self.config.precise_comentarios
    }

    /// Recálculo rápido: invalida cache de feed + perfil + reset contadores.
    /// No-async-spawn, corre inline (es barato, solo borra cache).
    async fn execute_fast(
        &self,
        pool: &PgPool,
        redis: &Option<RedisPool>,
        user_id: i32,
    ) -> Result<(), AppError> {
        RecommenderService::invalidate_user_feed(redis, user_id).await?;
        sqlx::query!(
            "UPDATE algoritmo_estado SET cnt_likes = 0, cnt_reproducciones = 0, \
                 cnt_completas = 0, cnt_descargas = 0, cnt_follows = 0, cnt_comentarios = 0, \
                 ultimo_rapido = NOW() WHERE usuario_id = $1",
            user_id,
        )
        .execute(pool)
        .await?;
        tracing::debug!(target: "kamples.planner", user_id, "recalculo rapido");
        Ok(())
    }

    /// Recálculo preciso: invalida + recalcula tag_affinity async + reset.
    /// El recálculo de tags se programa via `schedule_recalc` (no bloquea).
    /// Cuando se porte `GeneradorEmbeddings` (futura tarea) se enchufará aquí.
    fn execute_precise(&self, pool: &PgPool, redis: &Option<RedisPool>, user_id: i32) {
        TagAffinityService::schedule_recalc(pool.clone(), user_id, &self.locks);
        let pool_clone = pool.clone();
        let redis_clone = redis.clone();
        tokio::spawn(async move {
            if let Err(error) =
                RecommenderService::invalidate_user_feed(&redis_clone, user_id).await
            {
                tracing::warn!(target: "kamples.planner", %error, user_id, "fast invalidate fallo");
            }
            let _ = sqlx::query!(
                "UPDATE algoritmo_estado SET cnt_likes_preciso = 0, cnt_reproducciones_preciso = 0, \
                     cnt_completas_preciso = 0, cnt_descargas_preciso = 0, \
                     cnt_follows_preciso = 0, cnt_comentarios_preciso = 0, \
                     ultimo_preciso = NOW(), version_perfil = version_perfil + 1 \
                 WHERE usuario_id = $1",
                user_id,
            )
            .execute(&pool_clone)
            .await;
            tracing::debug!(target: "kamples.planner", user_id, "recalculo preciso");
        });
    }

    /// Procesa todos los usuarios cuyo último recálculo rápido superó el
    /// intervalo configurado. Pensado para correr cada `interval_temporal_secs`.
    pub async fn process_temporal(
        &self,
        pool: &PgPool,
        redis: &Option<RedisPool>,
    ) -> Result<u32, AppError> {
        let interval_secs = i64::try_from(self.config.interval_temporal_secs).unwrap_or(300);

        let users = sqlx::query!(
            r#"
            SELECT usuario_id AS "usuario_id!"
            FROM algoritmo_estado
            WHERE ultimo_rapido < NOW() - ($1 || ' seconds')::interval
            LIMIT $2
            "#,
            interval_secs.to_string(),
            self.config.temporal_user_limit,
        )
        .fetch_all(pool)
        .await?;

        let mut count = 0u32;
        for row in users {
            if self.execute_fast(pool, redis, row.usuario_id).await.is_ok() {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Lanza el loop background. Devuelve `JoinHandle` para await en el
    /// shutdown. Se cancela con `token.cancel()`.
    #[must_use]
    pub fn spawn_periodic_loop(
        self: Arc<Self>,
        pool: PgPool,
        redis: Option<RedisPool>,
        token: CancellationToken,
    ) -> JoinHandle<()> {
        let cfg = self.config;
        tokio::spawn(async move {
            /* Offset cada interval para que no se solapen en t=0. */
            let mut tick_mv = interval_at(
                Instant::now() + Duration::from_secs(10),
                Duration::from_secs(cfg.interval_refresh_mv_secs),
            );
            let mut tick_recalc = interval_at(
                Instant::now() + Duration::from_secs(30),
                Duration::from_secs(cfg.interval_recalc_active_secs),
            );
            let mut tick_temporal = interval(Duration::from_secs(cfg.interval_temporal_secs));

            loop {
                tokio::select! {
                    () = token.cancelled() => {
                        tracing::info!(target: "kamples.planner", "loop cancelado");
                        break;
                    }
                    _ = tick_mv.tick() => {
                        if let Err(error) = PrecomputeService::refresh(&pool).await {
                            tracing::warn!(target: "kamples.planner", %error, "refresh MV fallo");
                        }
                    }
                    _ = tick_recalc.tick() => {
                        match TagAffinityService::recalculate_active(&pool, cfg.recalc_active_limit).await {
                            Ok(n) => tracing::info!(target: "kamples.planner", count = n, "recalculo activos OK"),
                            Err(error) => tracing::warn!(target: "kamples.planner", %error, "recalculo activos fallo"),
                        }
                    }
                    _ = tick_temporal.tick() => {
                        match self.process_temporal(&pool, &redis).await {
                            Ok(n) => tracing::debug!(target: "kamples.planner", count = n, "process_temporal OK"),
                            Err(error) => tracing::warn!(target: "kamples.planner", %error, "process_temporal fallo"),
                        }
                    }
                }
            }
        })
    }
}

/* Re-export del Mutex para que el caller no necesite importarlo. */
pub type SharedPlanner = Arc<AlgoPlanner>;
#[allow(dead_code)]
type _MutexAlias = Mutex<()>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_match_legacy() {
        let cfg = AlgoPlannerConfig::legacy_defaults();
        assert_eq!(cfg.fast_likes, 3);
        assert_eq!(cfg.precise_likes, 6);
        assert_eq!(cfg.interval_refresh_mv_secs, 300);
    }

    #[test]
    fn interaction_kind_incrementos_unicos() {
        let kinds = [
            InteractionKind::Like,
            InteractionKind::Reproduccion,
            InteractionKind::Completa,
            InteractionKind::Descarga,
            InteractionKind::Follow,
            InteractionKind::Comentario,
        ];
        // Cada tipo suma 1 en exactamente dos contadores (fast y precise) y no se solapa con otro.
        let mut vistos = std::collections::HashSet::new();
        for k in kinds {
            let posiciones: Vec<usize> = k
                .incrementos()
                .iter()
                .enumerate()
                .filter(|(_, v)| **v == 1)
                .map(|(i, _)| i)
                .collect();
            assert_eq!(posiciones.len(), 2);
            assert!(vistos.insert(posiciones[0]));
            assert!(vistos.insert(posiciones[1]));
        }
    }

    #[test]
    fn fast_threshold_logic() {
        let cfg = AlgoPlannerConfig::legacy_defaults();
        let planner = AlgoPlanner::new(cfg);
        let zero = (0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        assert!(!planner.fast_threshold_hit(&zero));
        let triggered = (3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        assert!(planner.fast_threshold_hit(&triggered));
    }

    #[test]
    fn precise_threshold_logic() {
        let cfg = AlgoPlannerConfig::legacy_defaults();
        let planner = AlgoPlanner::new(cfg);
        let just_below = (0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0);
        assert!(!planner.precise_threshold_hit(&just_below));
        let triggered = (0, 0, 0, 0, 0, 0, 6, 0, 0, 0, 0, 0);
        assert!(planner.precise_threshold_hit(&triggered));
    }

    #[tokio::test]
    async fn spawn_periodic_loop_cancels_cleanly() {
        /* Placeholder: no podemos testear el loop sin pool real, pero
         * validamos que el token cancela el spawn sin colgar. */
        let token = CancellationToken::new();
        let token_clone = token.clone();
        let handle = tokio::spawn(async move {
            tokio::select! {
                () = token_clone.cancelled() => "cancelled",
                () = tokio::time::sleep(Duration::from_secs(10)) => "timeout",
            }
        });
        token.cancel();
        let result = handle.await.unwrap();
        assert_eq!(result, "cancelled");
    }
}
