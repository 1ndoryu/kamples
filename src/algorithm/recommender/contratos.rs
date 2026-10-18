/* [174A-52] Contratos del MotorRecomendacion: configuración, resultado del
 * feed y fila intermedia del enriquecimiento (SampleRow). */

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::algorithm::candidates::CandidatesConfig;
use crate::algorithm::signals::AlgorithmSignalConfig;

#[derive(Debug, Clone, Copy)]
pub struct RecommenderConfig {
    /// TTL del cache fresco para página 0 (en segundos).
    pub fresh_ttl_p0: u64,
    /// TTL del cache fresco para páginas paginadas (en segundos).
    pub fresh_ttl_pn: u64,
    /// TTL del cache stale (escudo para responder rápido mientras recalcula).
    pub stale_ttl: u64,
    /// TTL del lock distribuido para warm async.
    pub warm_lock_ttl: u64,
    /// Umbral para activar el `SelectorCandidatos` (si total_activos >).
    pub umbral_candidatos: i64,
    /// Máx por creador (diversidad).
    pub max_por_creador: usize,
    /// Máx por género (diversidad).
    pub max_por_categoria: usize,
    /// Máx por tipo (diversidad — solo penaliza one-shots).
    pub max_por_tipo: usize,
    /// Configuración de límites de las 6 fuentes de candidatos.
    pub candidates: CandidatesConfig,
    /// Pesos y subpesos del scoring de las 6 señales.
    pub signal: AlgorithmSignalConfig,
}

impl RecommenderConfig {
    /// Valores idénticos al legado `algoritmoPesos.php`.
    pub const fn legacy_defaults() -> Self {
        Self {
            fresh_ttl_p0: 300,
            fresh_ttl_pn: 900,
            stale_ttl: 7200,
            warm_lock_ttl: 90,
            umbral_candidatos: 5000,
            max_por_creador: 3,
            max_por_categoria: 4,
            max_por_tipo: 5,
            candidates: CandidatesConfig::legacy_defaults(),
            signal: AlgorithmSignalConfig::legacy_current(),
        }
    }
}

impl Default for RecommenderConfig {
    fn default() -> Self {
        Self::legacy_defaults()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct RankedSample {
    pub id: i32,
    pub creador_id: i32,
    pub titulo: String,
    pub slug: String,
    pub tipo: String,
    pub bpm: Option<i32>,
    pub key: Option<String>,
    pub escala: Option<String>,
    pub tags: Vec<String>,
    pub publicado_at: Option<DateTime<Utc>>,
    pub total_likes: i32,
    pub total_reproducciones: i32,
    pub total_descargas: i32,
    pub verificado: bool,
    pub es_nuevo: bool,
    pub score: f64,
}

#[derive(Debug, Clone)]
pub(super) struct SampleRow {
    pub(super) id: i32,
    pub(super) creador_id: i32,
    pub(super) titulo: String,
    pub(super) slug: String,
    pub(super) tipo: String,
    pub(super) bpm: Option<i32>,
    pub(super) key: Option<String>,
    pub(super) escala: Option<String>,
    pub(super) tags: Vec<String>,
    pub(super) publicado_at: Option<DateTime<Utc>>,
    pub(super) total_likes: i32,
    pub(super) total_reproducciones: i32,
    pub(super) total_descargas: i32,
    pub(super) verificado: bool,
    pub(super) embedding_distance: Option<f64>,
    pub(super) likes_24h: f64,
    pub(super) reproducciones_24h: f64,
    pub(super) descargas_7d: f64,
    pub(super) follows_creador_7d: f64,
    pub(super) creador_seguido: bool,
    pub(super) likes_dados: f64,
    pub(super) reproducciones: f64,
    pub(super) tiempo_escucha: f64,
    pub(super) descargas_propias: f64,
    pub(super) completadas: f64,
    pub(super) dislike_penalty: f64,
    pub(super) nunca_reproducido: bool,
}
