use sqlx::{PgPool, Postgres, QueryBuilder};

use crate::errors::AppError;

use super::super::ColeccionesRepository;
use super::{
    LegacyColeccionParentRecord, LegacyColeccionRecord, LegacyColeccionSampleRecord,
    push_collection_projection,
};

impl ColeccionesRepository {
    pub async fn list_relevant_for_sample_legacy(
        pool: &PgPool,
        usuario_id: i32,
        sample_id: i32,
        limit: i64,
    ) -> Result<Vec<LegacyColeccionRecord>, AppError> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "WITH reference_tags AS (
                SELECT DISTINCT tag_val
                FROM samples rs
                CROSS JOIN LATERAL jsonb_array_elements_text(
                    COALESCE(
                        CASE WHEN jsonb_typeof(rs.metadata->'tags') = 'array' THEN rs.metadata->'tags' END,
                        CASE WHEN jsonb_typeof(rs.metadata->'tags_es') = 'array' THEN rs.metadata->'tags_es' END,
                        to_jsonb(COALESCE(rs.tags_enriquecidos, ARRAY[]::text[])),
                        '[]'::jsonb
                    )
                ) AS tag_val
                WHERE rs.id = ",
        );
        builder.push_bind(sample_id);
        builder.push(
            " AND rs.eliminado_en IS NULL
            )
            SELECT c.id, c.usuario_id, c.nombre, c.slug, COALESCE(c.descripcion, '') AS descripcion,
                   c.publica, c.parent_id, c.imagen_url, c.version, c.total_samples, ",
        );
        push_collection_projection(&mut builder, Some(usuario_id));
        builder.push(
            ", EXISTS(
                SELECT 1 FROM coleccion_samples cs_contains
                WHERE cs_contains.coleccion_id = c.id AND cs_contains.sample_id = ",
        );
        builder.push_bind(sample_id);
        builder.push(
            ") AS contiene_el_sample
             FROM colecciones c
             JOIN usuarios_ext u ON u.id = c.usuario_id
             LEFT JOIN LATERAL (
                SELECT COUNT(*) AS overlap
                FROM coleccion_samples cs_rel
                JOIN samples s_rel ON s_rel.id = cs_rel.sample_id
                CROSS JOIN LATERAL jsonb_array_elements_text(
                    COALESCE(
                        CASE WHEN jsonb_typeof(s_rel.metadata->'tags') = 'array' THEN s_rel.metadata->'tags' END,
                        CASE WHEN jsonb_typeof(s_rel.metadata->'tags_es') = 'array' THEN s_rel.metadata->'tags_es' END,
                        to_jsonb(COALESCE(s_rel.tags_enriquecidos, ARRAY[]::text[])),
                        '[]'::jsonb
                    )
                ) AS rel_tag
                WHERE cs_rel.coleccion_id = c.id
                  AND rel_tag IN (SELECT tag_val FROM reference_tags)
                  AND s_rel.estado = 'activo'
                  AND s_rel.eliminado_en IS NULL
             ) rel ON TRUE
             WHERE c.usuario_id = ",
        );
        builder.push_bind(usuario_id);
        builder.push(
            " AND c.eliminado_en IS NULL AND u.estado = 'activo'
             ORDER BY contiene_el_sample DESC, COALESCE(rel.overlap, 0) DESC, c.updated_at DESC, c.id DESC
             LIMIT ",
        );
        builder.push_bind(limit.clamp(1, 50));

        let rows = builder
            .build_query_as::<LegacyColeccionRecord>()
            .fetch_all(pool)
            .await?;
        Ok(rows)
    }

    pub async fn list_subcollection_ids_legacy(
        pool: &PgPool,
        parent_id: i64,
    ) -> Result<Vec<i64>, AppError> {
        let rows = sqlx::query_scalar::<_, i64>(
            r"SELECT id
               FROM colecciones
               WHERE parent_id = $1
                 AND eliminado_en IS NULL
                             ORDER BY updated_at DESC, id DESC",
        )
        .bind(parent_id)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_subcollections_legacy(
        pool: &PgPool,
        parent_id: i64,
        viewer_id: Option<i32>,
    ) -> Result<Vec<LegacyColeccionRecord>, AppError> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT c.id, c.usuario_id, c.nombre, c.slug, COALESCE(c.descripcion, '') AS descripcion, \
                    c.publica, c.parent_id, c.imagen_url, c.version, c.total_samples, ",
        );
        push_collection_projection(&mut builder, viewer_id);
        builder.push(", NULL::boolean AS contiene_el_sample");
        builder.push(
            " FROM colecciones c \
               JOIN usuarios_ext u ON u.id = c.usuario_id \
               WHERE c.parent_id = ",
        );
        builder.push_bind(parent_id);
        builder.push(" AND c.eliminado_en IS NULL AND u.estado = 'activo' ORDER BY c.updated_at DESC, c.id DESC");

        let rows = builder
            .build_query_as::<LegacyColeccionRecord>()
            .fetch_all(pool)
            .await?;
        Ok(rows)
    }

    pub async fn fetch_parent_legacy(
        pool: &PgPool,
        id: i64,
    ) -> Result<Option<LegacyColeccionParentRecord>, AppError> {
        let row = sqlx::query_as!(
            LegacyColeccionParentRecord,
            r#"SELECT id, nombre, slug
                             FROM colecciones
                             WHERE id = $1
                                 AND eliminado_en IS NULL"#,
            id,
        )
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn list_detail_samples_legacy(
        pool: &PgPool,
        coleccion_ids: &[i64],
    ) -> Result<Vec<LegacyColeccionSampleRecord>, AppError> {
        if coleccion_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT s.id, s.id_corto, s.slug, s.titulo, COALESCE(s.descripcion, '') AS descripcion, \
                    s.bpm, s.key AS music_key, s.escala, s.duracion, s.formato, \
                    COALESCE(s.metadata, '{}'::jsonb) AS metadata, \
                    COALESCE(s.tags_enriquecidos, ARRAY[]::text[]) AS tags, \
                    s.tipo, s.es_premium, s.precio, COALESCE(s.verificado, FALSE) AS verificado, \
                    s.ruta_preview, s.ruta_waveform, s.imagen_url, \
                    COALESCE(s.total_descargas, 0) AS total_descargas, \
                    COALESCE(s.total_likes, 0) AS total_likes, \
                    COALESCE(s.total_reproducciones, 0) AS total_reproducciones, \
                    COALESCE(s.total_comentarios, 0) AS total_comentarios, \
                    u.id AS creator_id, u.username AS creator_username, u.nombre_visible AS creator_nombre_visible, \
                    u.avatar_url AS creator_avatar_url, COALESCE(u.verificado, FALSE) AS creator_verificado \
               FROM coleccion_samples cs \
               JOIN samples s ON s.id = cs.sample_id \
               JOIN usuarios_ext u ON u.id = s.creador_id \
               WHERE cs.coleccion_id = ANY(",
        );
        builder.push_bind(coleccion_ids.to_vec());
        builder.push(") AND s.estado = 'activo' AND s.eliminado_en IS NULL AND u.estado = 'activo' ORDER BY array_position(");
        builder.push_bind(coleccion_ids.to_vec());
        builder.push(", cs.coleccion_id), cs.orden ASC, cs.added_at ASC");

        let rows = builder
            .build_query_as::<LegacyColeccionSampleRecord>()
            .fetch_all(pool)
            .await?;
        Ok(rows)
    }
}
