/* [174A-52] Tests del MotorRecomendacion. Las queries reales se cubren vía
 * `cargo sqlx prepare` (compile-time). Aquí validamos lógica pura: defaults,
 * keys de cache, diversidad, jaccard y scoring de fila aislada. */

#![allow(
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss
)]

use chrono::{Duration as ChronoDuration, Utc};

use super::cache::{
    fresh_cache_key, fresh_ttl, release_lock, stale_cache_key, try_acquire_lock, warm_lock_key,
    MEMORY_CACHE, MEMORY_LOCKS,
};
use super::puntuacion::apply_diversity;
use super::similares::jaccard_overlap;
use super::*;
use std::time::{Duration, Instant};

#[test]
fn legacy_defaults_match_php_constants() {
    let cfg = RecommenderConfig::legacy_defaults();
    assert_eq!(cfg.fresh_ttl_p0, 300);
    assert_eq!(cfg.fresh_ttl_pn, 900);
    assert_eq!(cfg.stale_ttl, 7200);
    assert_eq!(cfg.warm_lock_ttl, 90);
    assert_eq!(cfg.umbral_candidatos, 5000);
    assert_eq!(cfg.max_por_creador, 3);
    assert_eq!(cfg.max_por_categoria, 4);
    assert_eq!(cfg.max_por_tipo, 5);
}

#[test]
fn cache_keys_use_legacy_prefixes() {
    assert_eq!(fresh_cache_key(7, 20, 0), "kamples_feed_7_20_0");
    assert_eq!(stale_cache_key(7, 20, 40), "kamples_feed_stale_7_20_40");
    assert_eq!(warm_lock_key(7, 20, 0), "kamples_warm_feed_7_20_0");
}

#[test]
fn jaccard_overlap_is_case_insensitive_and_handles_empty() {
    assert_eq!(jaccard_overlap(&[], &[]), 0.0);
    assert_eq!(
        jaccard_overlap(&["Trap".to_string()], &["trap".to_string()]),
        1.0,
    );
    let result = jaccard_overlap(
        &["a".to_string(), "b".to_string()],
        &["b".to_string(), "c".to_string()],
    );
    assert!((result - (1.0 / 3.0)).abs() < 1e-9);
}

fn sample_row_basic(creador_id: i32, score_seed: f64, tipo: &str) -> RankedSample {
    RankedSample {
        id: score_seed as i32,
        creador_id,
        titulo: format!("S{}", score_seed as i32),
        slug: format!("s-{}", score_seed as i32),
        tipo: tipo.to_string(),
        bpm: Some(120),
        key: Some("C".to_string()),
        escala: Some("major".to_string()),
        tags: vec!["trap".to_string()],
        publicado_at: Some(Utc::now() - ChronoDuration::days(1)),
        total_likes: 0,
        total_reproducciones: 0,
        total_descargas: 0,
        verificado: false,
        es_nuevo: true,
        score: score_seed,
    }
}

#[test]
fn apply_diversity_penalizes_repeated_creator() {
    let cfg = RecommenderConfig::legacy_defaults();
    let input = vec![
        sample_row_basic(1, 10.0, "loop"),
        sample_row_basic(1, 9.0, "loop"),
        sample_row_basic(1, 8.0, "loop"),
        sample_row_basic(1, 7.0, "loop"), // 4to del mismo creador → penalizado
    ];
    let out = apply_diversity(input, &cfg);
    let cuarto = out
        .iter()
        .find(|r| (r.score - 7.0).abs() < 1e-9 || r.score < 7.0)
        .unwrap();
    /* El 4to del mismo creador (rc=4 > max_por_creador=3) sufre factor
     * (1 - 1*0.15) = 0.85 → score base 7.0 * 0.85 = 5.95. */
    assert!(
        cuarto.score < 7.0,
        "esperaba penalización, score={}",
        cuarto.score
    );
}

#[test]
fn apply_diversity_keeps_first_three_creators_untouched() {
    let cfg = RecommenderConfig::legacy_defaults();
    let input = vec![
        sample_row_basic(1, 10.0, "loop"),
        sample_row_basic(2, 9.0, "loop"),
        sample_row_basic(3, 8.0, "loop"),
    ];
    let out = apply_diversity(input, &cfg);
    /* Tres creadores distintos, ninguno repetido → sin penalización. */
    let total_max = out.iter().map(|r| r.score).fold(f64::MIN, f64::max);
    assert!((total_max - 10.0).abs() < 1e-9);
}

#[test]
fn fresh_ttl_distinguishes_first_page_from_paginated() {
    let cfg = RecommenderConfig::legacy_defaults();
    assert_eq!(fresh_ttl(&cfg, 0), 300);
    assert_eq!(fresh_ttl(&cfg, 20), 900);
    assert_eq!(fresh_ttl(&cfg, 40), 900);
}

#[tokio::test]
async fn invalidate_user_feed_without_redis_clears_memory_cache() {
    /* Pre-poblar la cache en memoria con dos usuarios. */
    MEMORY_CACHE.insert(
        fresh_cache_key(99, 20, 0),
        (vec![], Instant::now() + Duration::from_mins(1)),
    );
    MEMORY_CACHE.insert(
        fresh_cache_key(100, 20, 0),
        (vec![], Instant::now() + Duration::from_mins(1)),
    );

    RecommenderService::invalidate_user_feed(&None, 99)
        .await
        .expect("invalidate sin redis no debe fallar");

    assert!(MEMORY_CACHE.get(&fresh_cache_key(99, 20, 0)).is_none());
    assert!(MEMORY_CACHE.get(&fresh_cache_key(100, 20, 0)).is_some());

    /* Cleanup para no contaminar otros tests. */
    MEMORY_CACHE.remove(&fresh_cache_key(100, 20, 0));
}

#[tokio::test]
async fn try_acquire_lock_in_memory_blocks_second_call() {
    let key = "test_lock_concurrent_174a52".to_string();
    MEMORY_LOCKS.remove(&key);
    let first = try_acquire_lock(&None, &key, 60).await.unwrap();
    let second = try_acquire_lock(&None, &key, 60).await.unwrap();
    assert!(first);
    assert!(!second);
    release_lock(&None, &key).await;
}
