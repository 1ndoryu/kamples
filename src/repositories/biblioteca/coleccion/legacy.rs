use chrono::{DateTime, Utc};
use sqlx::{FromRow, Postgres, QueryBuilder};

mod listados;
mod relacionados;

const COLLECTION_TAGS_SQL: &str = r"
COALESCE(
    (
        SELECT array_agg(DISTINCT tag_val)
        FROM coleccion_samples cs_tags
        JOIN samples s_tags ON s_tags.id = cs_tags.sample_id
        CROSS JOIN LATERAL jsonb_array_elements_text(
            COALESCE(
                CASE WHEN jsonb_typeof(s_tags.metadata->'tags') = 'array' THEN s_tags.metadata->'tags' END,
                CASE WHEN jsonb_typeof(s_tags.metadata->'tags_es') = 'array' THEN s_tags.metadata->'tags_es' END,
                '[]'::jsonb
            )
        ) AS tag_val
        WHERE cs_tags.coleccion_id = c.id
          AND s_tags.estado = 'activo'
          AND s_tags.eliminado_en IS NULL
    ),
    ARRAY[]::text[]
)
";

const COLLECTION_TOTAL_ITEMS_SQL: &str = r"
(
    SELECT COUNT(*)
    FROM coleccion_samples cs_count
    JOIN samples s_count ON s_count.id = cs_count.sample_id
    WHERE s_count.estado = 'activo'
      AND s_count.eliminado_en IS NULL
      AND (
        cs_count.coleccion_id = c.id
        OR cs_count.coleccion_id IN (
            SELECT sub.id
            FROM colecciones sub
            WHERE sub.parent_id = c.id
              AND sub.eliminado_en IS NULL
        )
      )
)
";

#[derive(Debug, Clone, FromRow)]
#[allow(clippy::struct_excessive_bools)]
pub struct LegacyColeccionRecord {
    pub id: i64,
    pub usuario_id: i32,
    pub nombre: String,
    pub slug: Option<String>,
    pub descripcion: String,
    pub publica: bool,
    pub parent_id: Option<i64>,
    pub imagen_url: Option<String>,
    pub version: i32,
    pub total_samples: i32,
    pub total_items: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub tags: Vec<String>,
    pub username: Option<String>,
    pub nombre_visible: Option<String>,
    pub avatar_url: Option<String>,
    pub verificado: bool,
    pub esta_guardada: bool,
    pub esta_likeada: bool,
    pub total_likes: i32,
    pub contiene_el_sample: Option<bool>,
}

#[derive(Debug, Clone, FromRow)]
pub struct LegacyColeccionParentRecord {
    pub id: i64,
    pub nombre: String,
    pub slug: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct LegacyColeccionSampleRecord {
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
    pub metadata: serde_json::Value,
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
    pub creator_id: i32,
    pub creator_username: String,
    pub creator_nombre_visible: Option<String>,
    pub creator_avatar_url: Option<String>,
    pub creator_verificado: bool,
}

fn push_collection_projection(builder: &mut QueryBuilder<Postgres>, viewer_id: Option<i32>) {
    push_collection_projection_common(builder, viewer_id);
}

fn push_collection_projection_common(builder: &mut QueryBuilder<Postgres>, viewer_id: Option<i32>) {
    builder.push(COLLECTION_TOTAL_ITEMS_SQL);
    builder.push(" AS total_items, c.created_at, c.updated_at, ");
    builder.push(COLLECTION_TAGS_SQL);
    builder.push(" AS tags, u.username, u.nombre_visible, u.avatar_url, COALESCE(u.verificado, FALSE) AS verificado, ");
    match viewer_id {
        Some(current_user_id) => {
            builder.push("EXISTS(SELECT 1 FROM colecciones_guardadas cg WHERE cg.coleccion_id = c.id AND cg.usuario_id = ");
            builder.push_bind(current_user_id);
            builder.push(") AS esta_guardada, ");
        }
        None => {
            builder.push("FALSE AS esta_guardada, ");
        }
    }
    builder.push("FALSE AS esta_likeada, 0 AS total_likes");
}

fn normalize_search(busqueda: Option<&str>) -> Option<String> {
    let trimmed = busqueda?.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(format!("%{trimmed}%"))
    }
}

fn push_search_filter(builder: &mut QueryBuilder<Postgres>, search: &str) {
    builder.push(" AND (c.nombre ILIKE ");
    builder.push_bind(search.to_owned());
    builder.push(" OR COALESCE(c.descripcion, '') ILIKE ");
    builder.push_bind(search.to_owned());
    builder.push(")");
}

fn parse_slug_collection_id(slug: &str) -> Option<i64> {
    let trimmed = slug.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(id) = trimmed.parse::<i64>() {
        return Some(id);
    }

    trimmed
        .rsplit('-')
        .next()
        .and_then(|fragment| fragment.parse::<i64>().ok())
}
