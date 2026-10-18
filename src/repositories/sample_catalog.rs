use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, QueryBuilder};

use super::sample::SampleRepository;

mod aggregates;
mod query;
mod scoring;
mod similar;

pub use aggregates::{TagAggregateFilters, TagAggregateItem, TagAggregatesResult};
pub use scoring::CriterioScoring;

use query::{
    push_auto_hide_filter, push_guardado_en_coleccion_select, push_public_filters,
    push_public_order, push_reaccion_select, CountRow, SampleSummaryRow, SAMPLE_SUMMARY_SELECT,
};

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

impl SampleRepository {
    pub async fn list_public_samples(
        pool: &PgPool,
        filters: &SampleListFilters,
    ) -> Result<SampleListResult, sqlx::Error> {
        let total = Self::count_public_samples(pool, filters).await?;
        if total == 0 {
            return Ok(SampleListResult {
                items: Vec::new(),
                total,
            });
        }

        let mut builder = QueryBuilder::<Postgres>::new(SAMPLE_SUMMARY_SELECT);

        /* [296A-1] Agregar flag ya_guardado_en_coleccion si hay usuario autenticado */
        push_guardado_en_coleccion_select(&mut builder, filters.viewer_id);

        /* [296A-3] Agregar liked/reaccion del usuario autenticado */
        push_reaccion_select(&mut builder, filters.viewer_id);

        push_public_filters(&mut builder, filters);
        push_public_order(&mut builder, filters);
        builder.push(" LIMIT ");
        builder.push_bind(filters.per_page);
        builder.push(" OFFSET ");
        builder.push_bind(filters.offset());

        let rows = builder
            .build_query_as::<SampleSummaryRow>()
            .fetch_all(pool)
            .await?;

        Ok(SampleListResult {
            items: rows
                .into_iter()
                .map(SampleCatalogSummaryRecord::from)
                .collect(),
            total,
        })
    }

    pub async fn find_public_samples_by_ids_in_order(
        pool: &PgPool,
        sample_ids: &[i32],
        viewer_id: Option<i32>,
    ) -> Result<Vec<SampleCatalogSummaryRecord>, sqlx::Error> {
        if sample_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = QueryBuilder::<Postgres>::new(SAMPLE_SUMMARY_SELECT);

        /* [296A-1] Agregar flag ya_guardado_en_coleccion si hay usuario autenticado */
        push_guardado_en_coleccion_select(&mut builder, viewer_id);

        /* [296A-3] Agregar liked/reaccion del usuario autenticado */
        push_reaccion_select(&mut builder, viewer_id);

        builder.push(
            " FROM samples s
              INNER JOIN usuarios_ext u ON u.id = s.creador_id
              WHERE s.id = ANY(",
        );
        builder.push_bind(sample_ids.to_vec());
        builder.push(
            "::int[])
              AND s.eliminado_en IS NULL
              AND s.estado = 'activo'",
        );
        push_auto_hide_filter(&mut builder, "s.id", "s.creador_id", viewer_id);
        builder.push(" ORDER BY array_position(");
        builder.push_bind(sample_ids.to_vec());
        builder.push("::int[], s.id)");

        let rows = builder
            .build_query_as::<SampleSummaryRow>()
            .fetch_all(pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(SampleCatalogSummaryRecord::from)
            .collect())
    }

    pub async fn find_sample_by_slug_or_short_id(
        pool: &PgPool,
        slug_or_short_id: &str,
    ) -> Result<Option<SampleCatalogDetailRecord>, sqlx::Error> {
        sqlx::query_as!(
            SampleCatalogDetailRecord,
            "SELECT
                s.id,
                s.id_corto,
                s.slug AS \"slug!\",
                s.titulo AS \"titulo!\",
                s.descripcion AS \"descripcion!\",
                s.bpm,
                s.key AS \"music_key?\",
                s.escala,
                s.duracion,
                s.formato AS \"formato!\",
                s.tamano,
                COALESCE(s.tags, ARRAY[]::text[]) AS \"tags!: Vec<String>\",
                s.tipo AS \"tipo!\",
                s.estado AS \"estado!\",
                COALESCE(s.es_premium, FALSE) AS \"es_premium!\",
                CAST(s.precio AS double precision) AS \"precio?\",
                COALESCE(s.metadata, '{}'::jsonb) AS \"metadata!: serde_json::Value\",
                s.ruta_preview,
                s.ruta_waveform,
                s.ruta_original,
                s.ruta_optimizada,
                COALESCE(s.permitir_descarga, TRUE) AS \"permitir_descarga!\",
                COALESCE(s.licencia_libre, FALSE) AS \"licencia_libre!\",
                s.imagen_url,
                COALESCE(s.total_descargas, 0) AS \"total_descargas!\",
                COALESCE(s.total_likes, 0) AS \"total_likes!\",
                COALESCE(s.total_reproducciones, 0) AS \"total_reproducciones!\",
                COALESCE(s.total_comentarios, 0) AS \"total_comentarios!\",
                s.audio_hash,
                COALESCE(s.verificado, FALSE) AS \"verificado!\",
                COALESCE(s.mostrar_en_comunidad, TRUE) AS \"mostrar_en_comunidad!\",
                s.publicado_at,
                s.created_at,
                s.cancion_origen_id,
                s.relacion_sampleo_id,

                -- [166A-1] QQ51: Datos enriquecidos de cancion origen
                co.titulo AS \"cancion_origen_titulo?\",
                co.slug AS \"cancion_origen_slug?\",
                a.nombre AS \"cancion_origen_artista?\",
                co.whosampled_url AS \"cancion_origen_whosampled_url?\",
                co.bpm::smallint AS \"cancion_origen_bpm?\",

                -- [166A-1] QQ51: Datos de relacion sampleo
                rs.whosampled_id AS \"relacion_sampleo_whosampled_id?\",

                -- [166A-1] QQ117: Extraccion via cola_extraccion_samples (ultima fila)
                ces.youtube_id AS \"extraccion_youtube_id?\",
                ces.spotify_id AS \"extraccion_spotify_id?\",
                ces.timing_inicio_seg::int AS \"extraccion_timing_inicio_seg?\",
                ces.bpm_detectado::int AS \"extraccion_bpm_detectado?\",
                ces.duracion_compas_seg::float8 AS \"extraccion_duracion_compas_seg?\",
                ces.compas_inicio_seg::float8 AS \"extraccion_compas_inicio_seg?\",
                ces.compas_fin_seg::float8 AS \"extraccion_compas_fin_seg?\",
                ces.lado AS \"extraccion_lado?\",
                ces.estado AS \"extraccion_estado?\",
                ces.ruta_audio_extraido AS \"extraccion_ruta_audio?\",
                ces.ruta_audio_completo AS \"extraccion_ruta_audio_completo?\",
                ces.metadata_extraccion AS \"extraccion_metadata?: serde_json::Value\",

                -- [166A-1] Slug/album de canciones fuente y destino via relaciones_sample
                cf.slug AS \"extraccion_fuente_slug?\",
                cf.album AS \"extraccion_fuente_album?\",
                cd.slug AS \"extraccion_destino_slug?\",
                cd.album AS \"extraccion_destino_album?\",

                u.id AS \"creator_id!\",
                u.username AS \"creator_username!\",
                u.nombre_visible AS \"creator_nombre_visible?\",
                u.avatar_url AS \"creator_avatar_url?\",
                COALESCE(u.verificado, FALSE) AS \"creator_verificado!\"
             FROM samples s
             INNER JOIN usuarios_ext u ON u.id = s.creador_id
             -- [166A-1] QQ51: LEFT JOINs para datos enriquecidos
             LEFT JOIN canciones co ON co.id = s.cancion_origen_id
             LEFT JOIN artistas_musicales a ON a.id = co.artista_id
             LEFT JOIN relaciones_sample rs ON rs.id = s.relacion_sampleo_id
             LEFT JOIN canciones cf ON cf.id = rs.cancion_fuente_id
             LEFT JOIN canciones cd ON cd.id = rs.cancion_destino_id
             -- [166A-1] QQ117: Left join lateral para traer la ultima fila de extraccion
             LEFT JOIN LATERAL (
                 SELECT * FROM cola_extraccion_samples
                 WHERE sample_id = s.id
                 ORDER BY created_at DESC
                 LIMIT 1
             ) ces ON TRUE
             WHERE s.eliminado_en IS NULL
               AND (LOWER(s.slug) = LOWER($1) OR s.id_corto = $1)
             ORDER BY CASE WHEN LOWER(s.slug) = LOWER($1) THEN 0 ELSE 1 END, s.id DESC
             LIMIT 1",
            slug_or_short_id
        )
        .fetch_optional(pool)
        .await
    }

    #[allow(clippy::too_many_lines)]
    pub async fn find_random_public_sample(
        pool: &PgPool,
    ) -> Result<Option<SampleCatalogDetailRecord>, sqlx::Error> {
        sqlx::query_as!(
            SampleCatalogDetailRecord,
            "SELECT
                s.id,
                s.id_corto,
                s.slug AS \"slug!\",
                s.titulo AS \"titulo!\",
                s.descripcion AS \"descripcion!\",
                s.bpm,
                s.key AS \"music_key?\",
                s.escala,
                s.duracion,
                s.formato AS \"formato!\",
                s.tamano,
                COALESCE(s.tags, ARRAY[]::text[]) AS \"tags!: Vec<String>\",
                s.tipo AS \"tipo!\",
                s.estado AS \"estado!\",
                COALESCE(s.es_premium, FALSE) AS \"es_premium!\",
                CAST(s.precio AS double precision) AS \"precio?\",
                COALESCE(s.metadata, '{}'::jsonb) AS \"metadata!: serde_json::Value\",
                s.ruta_preview,
                s.ruta_waveform,
                s.ruta_original,
                s.ruta_optimizada,
                COALESCE(s.permitir_descarga, TRUE) AS \"permitir_descarga!\",
                COALESCE(s.licencia_libre, FALSE) AS \"licencia_libre!\",
                s.imagen_url,
                COALESCE(s.total_descargas, 0) AS \"total_descargas!\",
                COALESCE(s.total_likes, 0) AS \"total_likes!\",
                COALESCE(s.total_reproducciones, 0) AS \"total_reproducciones!\",
                COALESCE(s.total_comentarios, 0) AS \"total_comentarios!\",
                s.audio_hash,
                COALESCE(s.verificado, FALSE) AS \"verificado!\",
                COALESCE(s.mostrar_en_comunidad, TRUE) AS \"mostrar_en_comunidad!\",
                s.publicado_at,
                s.created_at,
                s.cancion_origen_id,
                s.relacion_sampleo_id,

                -- [166A-1] QQ51: Datos enriquecidos de cancion origen
                co.titulo AS \"cancion_origen_titulo?\",
                co.slug AS \"cancion_origen_slug?\",
                a.nombre AS \"cancion_origen_artista?\",
                co.whosampled_url AS \"cancion_origen_whosampled_url?\",
                co.bpm::smallint AS \"cancion_origen_bpm?\",

                -- [166A-1] QQ51: Datos de relacion sampleo
                rs.whosampled_id AS \"relacion_sampleo_whosampled_id?\",

                -- [166A-1] QQ117: Extraccion via cola_extraccion_samples (ultima fila)
                ces.youtube_id AS \"extraccion_youtube_id?\",
                ces.spotify_id AS \"extraccion_spotify_id?\",
                ces.timing_inicio_seg::int AS \"extraccion_timing_inicio_seg?\",
                ces.bpm_detectado::int AS \"extraccion_bpm_detectado?\",
                ces.duracion_compas_seg::float8 AS \"extraccion_duracion_compas_seg?\",
                ces.compas_inicio_seg::float8 AS \"extraccion_compas_inicio_seg?\",
                ces.compas_fin_seg::float8 AS \"extraccion_compas_fin_seg?\",
                ces.lado AS \"extraccion_lado?\",
                ces.estado AS \"extraccion_estado?\",
                ces.ruta_audio_extraido AS \"extraccion_ruta_audio?\",
                ces.ruta_audio_completo AS \"extraccion_ruta_audio_completo?\",

                -- [166A-1] QQ117: Extraccion via cola_extraccion_samples (ultima fila)
                ces.metadata_extraccion AS \"extraccion_metadata?: serde_json::Value\",

                -- [166A-1] Slug/album de canciones fuente y destino via relaciones_sample
                cf.slug AS \"extraccion_fuente_slug?\",
                cf.album AS \"extraccion_fuente_album?\",
                cd.slug AS \"extraccion_destino_slug?\",
                cd.album AS \"extraccion_destino_album?\",

                u.id AS \"creator_id!\",
                u.username AS \"creator_username!\",
                u.nombre_visible AS \"creator_nombre_visible?\",
                u.avatar_url AS \"creator_avatar_url?\",
                COALESCE(u.verificado, FALSE) AS \"creator_verificado!\"
             FROM samples s
             INNER JOIN usuarios_ext u ON u.id = s.creador_id
             -- [166A-1] QQ51: LEFT JOINs para datos enriquecidos
             LEFT JOIN canciones co ON co.id = s.cancion_origen_id
             LEFT JOIN artistas_musicales a ON a.id = co.artista_id
             LEFT JOIN relaciones_sample rs ON rs.id = s.relacion_sampleo_id
             LEFT JOIN canciones cf ON cf.id = rs.cancion_fuente_id
             LEFT JOIN canciones cd ON cd.id = rs.cancion_destino_id
             -- [166A-1] QQ117: Left join lateral para traer la ultima fila de extraccion
             LEFT JOIN LATERAL (
                 SELECT * FROM cola_extraccion_samples
                 WHERE sample_id = s.id
                 ORDER BY created_at DESC
                 LIMIT 1
             ) ces ON TRUE
             WHERE s.id IN (
                 SELECT id
                 FROM samples
                 WHERE eliminado_en IS NULL
                   AND estado = 'activo'
                 ORDER BY publicado_at DESC NULLS LAST, created_at DESC, id DESC
                 LIMIT 1000
             )
             ORDER BY RANDOM()
             LIMIT 1"
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_owned_sample_by_slug_or_short_id(
        pool: &PgPool,
        slug_or_short_id: &str,
    ) -> Result<Option<OwnedSampleRecord>, sqlx::Error> {
        sqlx::query_as!(
            OwnedSampleRecord,
            "SELECT
                s.id,
                s.id_corto,
                s.slug AS \"slug!\",
                s.creador_id AS \"creator_id!\"
             FROM samples s
             WHERE s.eliminado_en IS NULL
               AND (LOWER(s.slug) = LOWER($1) OR s.id_corto = $1)
             ORDER BY CASE WHEN LOWER(s.slug) = LOWER($1) THEN 0 ELSE 1 END, s.id DESC
             LIMIT 1",
            slug_or_short_id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn update_sample_metadata(
        pool: &PgPool,
        sample_id: i32,
        patch: &UpdateSamplePatch,
    ) -> Result<(), sqlx::Error> {
        /* [254A-8a] Bug pre-existente: Separated::push_bind inserta el separador
         * antes del bind, lo que generaba `col = , $1`. Se usa push_bind_unseparated
         * y push_unseparated para mantener `col = $1` dentro de cada asignacion. */
        let mut builder = QueryBuilder::<Postgres>::new("UPDATE samples SET ");
        {
            let mut separated = builder.separated(", ");

            if let Some(titulo) = &patch.titulo {
                separated.push("titulo = ");
                separated.push_bind_unseparated(titulo.clone());
            }

            if let Some(descripcion) = &patch.descripcion {
                separated.push("descripcion = ");
                separated.push_bind_unseparated(descripcion.clone());
            }

            if let Some(tags) = &patch.tags {
                separated.push("tags = ");
                separated.push_bind_unseparated(tags.clone());
                separated.push_unseparated("::text[]");
            }

            if let Some(sample_type) = &patch.sample_type {
                separated.push("tipo = ");
                separated.push_bind_unseparated(sample_type.clone());
            }

            if let Some(es_premium) = patch.es_premium {
                separated.push("es_premium = ");
                separated.push_bind_unseparated(es_premium);
            }

            if let Some(precio) = &patch.precio {
                match precio {
                    Some(value) => {
                        separated.push("precio = ");
                        separated.push_bind_unseparated(*value);
                    }
                    None => {
                        separated.push("precio = NULL");
                    }
                }
            }

            if let Some(permitir_descarga) = patch.permitir_descarga {
                separated.push("permitir_descarga = ");
                separated.push_bind_unseparated(permitir_descarga);
            }

            if let Some(licencia_libre) = patch.licencia_libre {
                separated.push("licencia_libre = ");
                separated.push_bind_unseparated(licencia_libre);
            }

            if let Some(mostrar_en_comunidad) = patch.mostrar_en_comunidad {
                separated.push("mostrar_en_comunidad = ");
                separated.push_bind_unseparated(mostrar_en_comunidad);
            }

            if let Some(imagen_url) = &patch.imagen_url {
                match imagen_url {
                    Some(value) => {
                        separated.push("imagen_url = ");
                        separated.push_bind_unseparated(value.clone());
                    }
                    None => {
                        separated.push("imagen_url = NULL");
                    }
                }
            }

            separated.push("updated_at = NOW()");
        }

        builder.push(" WHERE id = ");
        builder.push_bind(sample_id);

        builder.build().execute(pool).await?;
        Ok(())
    }

    pub async fn soft_delete_owned_sample(
        pool: &PgPool,
        sample_id: i32,
        owner_id: i32,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            "UPDATE samples
             SET estado = 'eliminado', eliminado_en = NOW(), updated_at = NOW()
             WHERE id = $1 AND creador_id = $2 AND eliminado_en IS NULL",
            sample_id,
            owner_id,
        )
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn count_public_samples(
        pool: &PgPool,
        filters: &SampleListFilters,
    ) -> Result<i64, sqlx::Error> {
        let mut builder = QueryBuilder::<Postgres>::new("SELECT COUNT(*) AS total");
        push_public_filters(&mut builder, filters);

        let row = builder.build_query_as::<CountRow>().fetch_one(pool).await?;
        Ok(row.total)
    }

    /* [274A-7] GET /canciones/:slug/samples — devuelve los samples publicados
     * cuyo cancion_origen_id apunta a la cancion identificada por slug.
     * Reutiliza SAMPLE_SUMMARY_SELECT y el filtro de auto-hide para consistencia. */
    pub async fn list_by_cancion_id(
        pool: &PgPool,
        cancion_id: i32,
        viewer_id: Option<i32>,
    ) -> Result<Vec<SampleCatalogSummaryRecord>, sqlx::Error> {
        let mut builder = QueryBuilder::<Postgres>::new(SAMPLE_SUMMARY_SELECT);
        builder.push(
            " FROM samples s
              INNER JOIN usuarios_ext u ON u.id = s.creador_id
              WHERE s.eliminado_en IS NULL
                AND s.estado = 'activo'
                AND s.cancion_origen_id = ",
        );
        builder.push_bind(cancion_id);
        push_auto_hide_filter(&mut builder, "s.id", "s.creador_id", viewer_id);
        builder.push(" ORDER BY s.publicado_at DESC NULLS LAST, s.id DESC");

        let rows = builder
            .build_query_as::<SampleSummaryRow>()
            .fetch_all(pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(SampleCatalogSummaryRecord::from)
            .collect())
    }
}
