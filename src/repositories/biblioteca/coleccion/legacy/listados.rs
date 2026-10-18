use sqlx::{PgPool, Postgres, QueryBuilder};

use crate::errors::AppError;

use super::super::ColeccionesRepository;
use super::{
    LegacyColeccionRecord, normalize_search, parse_slug_collection_id, push_collection_projection,
    push_collection_projection_common, push_search_filter,
};

impl ColeccionesRepository {
    pub async fn list_user_legacy(
        pool: &PgPool,
        usuario_id: i32,
        busqueda: Option<&str>,
    ) -> Result<Vec<LegacyColeccionRecord>, AppError> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT c.id, c.usuario_id, c.nombre, c.slug, COALESCE(c.descripcion, '') AS descripcion, \
                    c.publica, c.parent_id, c.imagen_url, c.version, c.total_samples, ",
        );
        push_collection_projection(&mut builder, None);
        builder.push(", NULL::boolean AS contiene_el_sample");
        builder.push(
            " FROM colecciones c \
               JOIN usuarios_ext u ON u.id = c.usuario_id \
               WHERE c.eliminado_en IS NULL \
                 AND c.usuario_id = ",
        );
        builder.push_bind(usuario_id);
        builder.push(" AND u.estado = 'activo'");
        if let Some(search) = normalize_search(busqueda) {
            push_search_filter(&mut builder, &search);
        }
        builder.push(" ORDER BY c.updated_at DESC, c.id DESC");

        let rows = builder
            .build_query_as::<LegacyColeccionRecord>()
            .fetch_all(pool)
            .await?;
        Ok(rows)
    }

    pub async fn list_explore_legacy(
        pool: &PgPool,
        viewer_id: Option<i32>,
        busqueda: Option<&str>,
        limit: i64,
        offset: i64,
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
               WHERE c.eliminado_en IS NULL \
                 AND u.estado = 'activo' \
                 AND (c.publica = TRUE",
        );
        if let Some(current_user_id) = viewer_id {
            builder.push(" OR c.usuario_id = ");
            builder.push_bind(current_user_id);
        }
        builder.push(")");
        if let Some(search) = normalize_search(busqueda) {
            push_search_filter(&mut builder, &search);
        }
        builder.push(" ORDER BY ");
        if let Some(current_user_id) = viewer_id {
            builder.push("CASE WHEN c.usuario_id = ");
            builder.push_bind(current_user_id);
            builder.push(" THEN 0 ELSE 1 END, ");
        }
        builder.push("c.updated_at DESC, c.id DESC LIMIT ");
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let rows = builder
            .build_query_as::<LegacyColeccionRecord>()
            .fetch_all(pool)
            .await?;
        Ok(rows)
    }

    pub async fn list_saved_legacy(
        pool: &PgPool,
        usuario_id: i32,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<LegacyColeccionRecord>, AppError> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT c.id, c.usuario_id, c.nombre, c.slug, COALESCE(c.descripcion, '') AS descripcion, \
                    c.publica, c.parent_id, c.imagen_url, c.version, c.total_samples, ",
        );
        push_collection_projection_common(&mut builder, Some(usuario_id));
        builder.push(
            " FROM colecciones_guardadas g \
               JOIN colecciones c ON c.id = g.coleccion_id \
               JOIN usuarios_ext u ON u.id = c.usuario_id \
               WHERE g.usuario_id = ",
        );
        builder.push_bind(usuario_id);
        builder.push(
            " AND c.eliminado_en IS NULL AND u.estado = 'activo' ORDER BY g.created_at DESC LIMIT ",
        );
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let rows = builder
            .build_query_as::<LegacyColeccionRecord>()
            .fetch_all(pool)
            .await?;
        Ok(rows)
    }

    pub async fn count_saved_legacy(pool: &PgPool, usuario_id: i32) -> Result<i64, AppError> {
        let total = sqlx::query_scalar::<_, i64>(
            r"SELECT COUNT(*)
               FROM colecciones_guardadas g
               JOIN colecciones c ON c.id = g.coleccion_id
               JOIN usuarios_ext u ON u.id = c.usuario_id
               WHERE g.usuario_id = $1
                 AND c.eliminado_en IS NULL
                                 AND u.estado = 'activo'",
        )
        .bind(usuario_id)
        .fetch_one(pool)
        .await?;
        Ok(total)
    }

    pub async fn tags_frecuentes_user_legacy(
        pool: &PgPool,
        usuario_id: i32,
        limit: i64,
    ) -> Result<Vec<String>, AppError> {
        let rows = sqlx::query_scalar::<_, String>(
            r"SELECT tag_val
               FROM coleccion_samples cs
               JOIN colecciones c ON c.id = cs.coleccion_id
               JOIN samples s ON s.id = cs.sample_id
               CROSS JOIN LATERAL jsonb_array_elements_text(
                    COALESCE(
                        CASE WHEN jsonb_typeof(s.metadata->'tags') = 'array' THEN s.metadata->'tags' END,
                        CASE WHEN jsonb_typeof(s.metadata->'tags_es') = 'array' THEN s.metadata->'tags_es' END,
                        '[]'::jsonb
                    )
               ) AS tag_val
               WHERE c.usuario_id = $1
                 AND c.eliminado_en IS NULL
                 AND s.estado = 'activo'
                 AND s.eliminado_en IS NULL
               GROUP BY tag_val
               ORDER BY COUNT(*) DESC, tag_val ASC
                             LIMIT $2",
        )
        .bind(usuario_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn tags_frecuentes_explorar_legacy(
        pool: &PgPool,
        limit: i64,
    ) -> Result<Vec<String>, AppError> {
        let rows = sqlx::query_scalar::<_, String>(
            r"SELECT tag_val
               FROM coleccion_samples cs
               JOIN colecciones c ON c.id = cs.coleccion_id
               JOIN usuarios_ext u ON u.id = c.usuario_id
               JOIN samples s ON s.id = cs.sample_id
               CROSS JOIN LATERAL jsonb_array_elements_text(
                    COALESCE(
                        CASE WHEN jsonb_typeof(s.metadata->'tags') = 'array' THEN s.metadata->'tags' END,
                        CASE WHEN jsonb_typeof(s.metadata->'tags_es') = 'array' THEN s.metadata->'tags_es' END,
                        '[]'::jsonb
                    )
               ) AS tag_val
               WHERE c.publica = TRUE
                 AND c.eliminado_en IS NULL
                 AND u.estado = 'activo'
                 AND s.estado = 'activo'
                 AND s.eliminado_en IS NULL
               GROUP BY tag_val
               ORDER BY COUNT(*) DESC, tag_val ASC
                             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    pub async fn fetch_legacy_by_id(
        pool: &PgPool,
        id: i64,
        viewer_id: Option<i32>,
    ) -> Result<Option<LegacyColeccionRecord>, AppError> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT c.id, c.usuario_id, c.nombre, c.slug, COALESCE(c.descripcion, '') AS descripcion, \
                    c.publica, c.parent_id, c.imagen_url, c.version, c.total_samples, ",
        );
        push_collection_projection(&mut builder, viewer_id);
        builder.push(", NULL::boolean AS contiene_el_sample");
        builder.push(
            " FROM colecciones c \
               JOIN usuarios_ext u ON u.id = c.usuario_id \
               WHERE c.id = ",
        );
        builder.push_bind(id);
        builder.push(" AND c.eliminado_en IS NULL AND u.estado = 'activo'");

        let row = builder
            .build_query_as::<LegacyColeccionRecord>()
            .fetch_optional(pool)
            .await?;
        Ok(row)
    }

    pub async fn fetch_legacy_by_slug(
        pool: &PgPool,
        slug: &str,
        viewer_id: Option<i32>,
    ) -> Result<Option<LegacyColeccionRecord>, AppError> {
        let maybe_id = parse_slug_collection_id(slug);
        if let Some(id) = maybe_id {
            if let Some(row) = Self::fetch_legacy_by_id(pool, id, viewer_id).await? {
                return Ok(Some(row));
            }
        }

        Self::fetch_legacy_by_slug_exact(pool, slug, viewer_id).await
    }

    pub async fn fetch_legacy_by_slug_exact(
        pool: &PgPool,
        slug: &str,
        viewer_id: Option<i32>,
    ) -> Result<Option<LegacyColeccionRecord>, AppError> {
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT c.id, c.usuario_id, c.nombre, c.slug, COALESCE(c.descripcion, '') AS descripcion, \
                    c.publica, c.parent_id, c.imagen_url, c.version, c.total_samples, ",
        );
        push_collection_projection(&mut builder, viewer_id);
        builder.push(", NULL::boolean AS contiene_el_sample");
        builder.push(
            " FROM colecciones c \
               JOIN usuarios_ext u ON u.id = c.usuario_id \
               WHERE c.slug = ",
        );
        builder.push_bind(slug.trim().to_owned());
        builder.push(" AND c.eliminado_en IS NULL AND u.estado = 'activo'");

        let row = builder
            .build_query_as::<LegacyColeccionRecord>()
            .fetch_optional(pool)
            .await?;
        Ok(row)
    }
}
