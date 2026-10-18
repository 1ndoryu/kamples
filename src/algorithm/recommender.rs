/* [174A-52] MotorRecomendacion portado del legado Kamples (1013 LoC PHP →
 * arquitectura limpia en Rust). Ver plan completo en
 * `Agente/planes/plan-recommender-2026-04-18.md`.
 *
 * DECISIÓN ARQUITECTÓNICA CLAVE
 * ─────────────────────────────
 * El legado generaba SQL gigante con scoring inline (100+ líneas de SQL por
 * request, con CTEs encadenadas, ROW_NUMBER, EXISTS correlacionados).
 * Aquí invertimos: SQL hace solo lo que SQL hace bien (filtrar, agregar,
 * traer datos enriquecidos para un set de IDs), y el scoring corre en Rust
 * usando `algorithm::signals` (ya tipado y testeado en 174A-49).
 *
 * Beneficios:
 *   - Scoring testeable sin BD (ya cubierto por tests de signals).
 *   - SQL parametrizado y validado en compile-time vía sqlx::query!.
 *   - Diversidad y serendipia se aplican como funciones puras.
 *   - El recommender es composable: la Fase 5 podrá inyectar afinidad por
 *     tags (174A-54) y la Fase 4 reutiliza el mismo enriquecimiento.
 *
 * 5 fases del plan, todas en este módulo (split por commits no por archivos):
 *   Fase 1: API + cache stale-while-revalidate + scoring + fallback.
 *   Fase 2: Bulk-fetch (3 páginas calculadas en una sola pasada).
 *   Fase 3: Warm async con tokio::spawn + lock SETNX.
 *   Fase 4: similar_to_sample (motor "más como esto").
 *   Fase 5: hooks de invalidación (invalidate_user_feed). */

/* Permitidos a nivel de módulo: el dominio es algoritmo numérico con muchos
 * casts entre usize/i64/f64 (coordenadas, conteos, scores) y queries SQL
 * grandes son inherentes al motor. Otros lints siguen activos. */
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::map_unwrap_or,
    clippy::explicit_iter_loop,
    clippy::too_many_lines,
    clippy::large_types_passed_by_value,
    clippy::type_complexity
)]

mod cache;
mod consultas;
mod contratos;
mod fallback;
mod puntuacion;
mod service;
mod similares;

pub use contratos::{RankedSample, RecommenderConfig};
pub use service::RecommenderService;

#[cfg(test)]
mod tests;
