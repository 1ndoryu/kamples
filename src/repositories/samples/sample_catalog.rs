use chrono::{DateTime, Utc};

use super::sample::SampleRepository;

mod aggregates;
mod detalle;
mod listado;
mod mutacion;
mod query;
mod scoring;
mod similar;

pub use aggregates::{TagAggregateFilters, TagAggregateItem, TagAggregatesResult};
pub use scoring::CriterioScoring;

/* [174A-44] Listado público de samples con filtros combinables.
 * Se mantiene en un módulo separado para no seguir inflando sample.rs, que ya
 * concentra upload, deduplicación y pipeline. El query queda encapsulado acá
 * porque comparte joins, filtros base y ordenamiento estable. */

/* [304A-2] Modos de ordenamiento del feed publico.
 * - `Smart`: equivalente al "inteligente" del legacy cuando NO hay busqueda
 *   (publicado_at DESC con tiebreakers); cuando hay busqueda, ranking FTS.
 * - `Recent`: estricto por `publicado_at DESC` — boton "Recientes" del UI.
 * - `Trending`: por engagement (descargas + likes*2 + reproducciones) DESC —
 *   boton "Destacados" del UI.
 *
 * Antes del 304A-2 el handler `/api/feed` ignoraba el parametro `tipo` y los
 * tres botones del frontend (Inteligente/Recientes/Destacados) devolvian
 * exactamente lo mismo. Esto restaura paridad con el legacy PHP. */
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SampleSortOrder {
    #[default]
    Smart,
    Recent,
    Trending,
}

#[derive(Debug, Clone)]
pub struct SampleListFilters {
    pub page: i64,
    pub per_page: i64,
    pub viewer_id: Option<i32>,
    pub search: Option<SampleTextSearch>,
    pub bpm: Option<i32>,
    pub music_key: Option<String>,
    pub sample_type: Option<String>,
    pub tags: Vec<String>,
    pub exclude_tags: Vec<String>,
    pub premium: Option<bool>,
    pub creator: Option<String>,
    pub sort: SampleSortOrder,
}

#[derive(Debug, Clone)]
pub struct SampleTextSearch {
    pub query: String,
    pub title_like: String,
    pub title_prefix: String,
    pub lower_query: String,
    pub lower_like: String,
    pub normalized_like: Option<String>,
}

impl SampleTextSearch {
    pub fn new(query: String, normalized: Option<String>) -> Self {
        let lower_query = query.to_ascii_lowercase();

        Self {
            title_like: format!("%{query}%"),
            title_prefix: format!("{query}%"),
            lower_like: format!("%{lower_query}%"),
            normalized_like: normalized.map(|value| format!("%{value}%")),
            lower_query,
            query,
        }
    }
}

impl SampleListFilters {
    pub fn offset(&self) -> i64 {
        (self.page - 1) * self.per_page
    }
}

#[derive(Debug, Clone)]
pub struct SampleListResult {
    pub items: Vec<SampleCatalogSummaryRecord>,
    pub total: i64,
}

#[derive(Debug, Clone)]
pub struct SampleCatalogSummaryRecord {
    pub id: i32,
    pub id_corto: Option<String>,
    pub slug: String,
    pub titulo: String,
    pub descripcion: String,
    pub bpm: Option<i32>,
    pub music_key: Option<String>,
    pub escala: Option<String>,
    pub duracion: f32,
    pub formato: String,
    pub tags: Vec<String>,
    pub tipo: String,
    pub es_premium: bool,
    pub precio: Option<f64>,
    pub verificado: bool,
    pub ruta_preview: Option<String>,
    pub ruta_waveform: Option<String>,
    pub imagen_url: Option<String>,
    pub total_descargas: i32,
    pub total_likes: i32,
    pub total_reproducciones: i32,
    pub total_comentarios: i32,
    pub publicado_at: Option<DateTime<Utc>>,
    pub creator_id: i32,
    pub creator_username: String,
    pub creator_nombre_visible: Option<String>,
    pub creator_avatar_url: Option<String>,
    pub creator_verificado: bool,
    pub metadata: serde_json::Value,
    pub ya_guardado_en_coleccion: Option<bool>,
    pub liked: Option<bool>,
    pub reaccion: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct SampleCatalogDetailRecord {
    pub id: i32,
    pub id_corto: Option<String>,
    pub slug: String,
    pub titulo: String,
    pub descripcion: String,
    pub bpm: Option<i32>,
    pub music_key: Option<String>,
    pub escala: Option<String>,
    pub duracion: f32,
    pub formato: String,
    pub tamano: i64,
    pub tags: Vec<String>,
    pub tipo: String,
    pub estado: String,
    pub es_premium: bool,
    pub precio: Option<f64>,
    pub metadata: serde_json::Value,
    pub ruta_preview: Option<String>,
    pub ruta_waveform: Option<String>,
    pub ruta_original: Option<String>,
    pub ruta_optimizada: Option<String>,
    pub permitir_descarga: bool,
    pub licencia_libre: bool,
    pub imagen_url: Option<String>,
    pub total_descargas: i32,
    pub total_likes: i32,
    pub total_reproducciones: i32,
    pub total_comentarios: i32,
    pub audio_hash: Option<String>,
    pub verificado: bool,
    pub mostrar_en_comunidad: bool,
    pub publicado_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub cancion_origen_id: Option<i32>,
    pub relacion_sampleo_id: Option<i32>,

    /* [166A-1] Datos enriquecidos de cancion origen via LEFT JOIN canciones */
    pub cancion_origen_titulo: Option<String>,
    pub cancion_origen_slug: Option<String>,
    pub cancion_origen_artista: Option<String>,
    pub cancion_origen_whosampled_url: Option<String>,
    pub cancion_origen_bpm: Option<i16>,

    /* [166A-1] Datos de relacion sampleo via LEFT JOIN relaciones_sample */
    pub relacion_sampleo_whosampled_id: Option<i32>,

    /* [166A-1] Datos de extraccion via LEFT JOIN LATERAL cola_extraccion_samples */
    pub extraccion_youtube_id: Option<String>,
    pub extraccion_spotify_id: Option<String>,
    pub extraccion_timing_inicio_seg: Option<i32>,
    pub extraccion_bpm_detectado: Option<i32>,
    pub extraccion_duracion_compas_seg: Option<f64>,
    pub extraccion_compas_inicio_seg: Option<f64>,
    pub extraccion_compas_fin_seg: Option<f64>,
    pub extraccion_lado: Option<String>,
    pub extraccion_estado: Option<String>,
    pub extraccion_ruta_audio: Option<String>,
    pub extraccion_ruta_audio_completo: Option<String>,
    pub extraccion_metadata: Option<serde_json::Value>,

    /* [166A-1] Slug/album de canciones fuente/destino via relaciones_sample -> canciones */
    pub extraccion_fuente_slug: Option<String>,
    pub extraccion_fuente_album: Option<String>,
    pub extraccion_destino_slug: Option<String>,
    pub extraccion_destino_album: Option<String>,

    pub creator_id: i32,
    pub creator_username: String,
    pub creator_nombre_visible: Option<String>,
    pub creator_avatar_url: Option<String>,
    pub creator_verificado: bool,
}

#[derive(Debug, Clone)]
pub struct OwnedSampleRecord {
    pub id: i32,
    pub id_corto: Option<String>,
    pub slug: String,
    pub creator_id: i32,
}

#[derive(Debug, Clone, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct UpdateSamplePatch {
    pub titulo: Option<String>,
    pub descripcion: Option<String>,
    pub tags: Option<Vec<String>>,
    pub sample_type: Option<String>,
    pub es_premium: Option<bool>,
    pub precio: Option<Option<f64>>,
    pub permitir_descarga: Option<bool>,
    pub licencia_libre: Option<bool>,
    pub mostrar_en_comunidad: Option<bool>,
    pub imagen_url: Option<Option<String>>,
}

impl UpdateSamplePatch {
    pub fn has_changes(&self) -> bool {
        self.titulo.is_some()
            || self.descripcion.is_some()
            || self.tags.is_some()
            || self.sample_type.is_some()
            || self.es_premium.is_some()
            || self.precio.is_some()
            || self.permitir_descarga.is_some()
            || self.licencia_libre.is_some()
            || self.mostrar_en_comunidad.is_some()
            || self.imagen_url.is_some()
    }
}
