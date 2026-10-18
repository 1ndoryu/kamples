/* Listado publico de samples y conteo; separado de sample_catalog.rs por tamano. */
use sqlx::{PgPool, Postgres, QueryBuilder};

use super::super::sample::SampleRepository;
use super::query::{
    push_auto_hide_filter, push_guardado_en_coleccion_select, push_public_filters,
    push_public_order, push_reaccion_select, CountRow, SampleSummaryRow, SAMPLE_SUMMARY_SELECT,
};
use super::{
    SampleCatalogDetailRecord, SampleCatalogSummaryRecord, SampleListFilters, SampleListResult,
};

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
}

impl SampleRepository {
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
