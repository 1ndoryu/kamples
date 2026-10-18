/* Detalle de samples y muestra aleatoria; separado de sample_catalog.rs por tamano. */
use sqlx::PgPool;

use super::super::sample::SampleRepository;
use super::{OwnedSampleRecord, SampleCatalogDetailRecord};

impl SampleRepository {
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
}
