/* [254A-7c] Repositorio de la biblioteca personal del usuario.
 *
 * Replica BibliotecaSamplesController + SamplesRepository (PHP legacy):
 *   - coleccionados_de_usuario: UNION descargas + samples subidos por el usuario.
 *   - contar_coleccionados: COUNT correspondiente para paginacion.
 *   - carpetas_coleccionados: arbol primaria/secundaria con conteos (metadata JSONB).
 *   - mover_a_carpeta: UPDATE jsonb_set sobre samples.metadata.
 *   - es_coleccionado_por_usuario: el usuario es creador o lo descargo (activo).
 *
 * Decision arquitectonica: aca usamos QueryBuilder en vez de query!() porque
 * los filtros (carpeta, busqueda, reaccion) se combinan dinamicamente y
 * query!() no soporta SQL condicional en compile-time. Mantenemos el SELECT
 * alineado con SAMPLE_SUMMARY_SELECT del modulo sample_catalog para que el
 * mismo parser (SampleSummaryRow) construya el record.
 */

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{FromRow, Postgres, QueryBuilder};

use crate::repositories::SampleCatalogSummaryRecord;

mod carpetas;
mod listados;

pub const CARPETA_DEFAULT: &str = "General";

#[derive(Debug, Clone, Copy)]
pub enum FiltroReaccion {
    Encanta,
    Like,
}

impl FiltroReaccion {
    fn db_value(self) -> &'static str {
        match self {
            Self::Encanta => "encanta",
            Self::Like => "like",
        }
    }
}

#[derive(FromRow)]
struct CountRow {
    total: i64,
}


#[derive(Debug, Clone, Default)]
pub struct ColeccionadosFilters {
    pub user_id: i32,
    pub page: i64,
    pub per_page: i64,
    pub carpeta: String,
    pub orden: String,
    pub busqueda: String,
    pub filtro_reaccion: Option<FiltroReaccion>,
}

impl ColeccionadosFilters {
    fn offset(&self) -> i64 {
        (self.page - 1).max(0) * self.per_page
    }
}

/* Mismo SELECT que sample_catalog::query::SAMPLE_SUMMARY_SELECT pero duplicado
 * aca para no tocar la visibilidad de aquel modulo. Si cambia uno hay que
 * cambiar el otro — TO-DO: extraer a un helper compartido cuando aparezca un
 * tercer consumidor. */
const BIBLIOTECA_SAMPLE_SELECT: &str = "SELECT
    s.id,
    s.id_corto,
    s.slug,
    s.titulo,
    s.descripcion,
    s.bpm,
    s.key AS music_key,
    s.escala,
    s.duracion,
    s.formato,
    COALESCE(s.tags, ARRAY[]::text[]) AS tags,
    s.tipo,
    s.es_premium,
    CAST(s.precio AS double precision) AS precio,
    s.verificado,
    COALESCE(NULLIF(s.ruta_preview, ''), s.ruta_optimizada) AS ruta_preview,
    s.ruta_waveform,
    s.imagen_url,
    COALESCE(s.total_descargas, 0) AS total_descargas,
    COALESCE(s.total_likes, 0) AS total_likes,
    COALESCE(s.total_reproducciones, 0) AS total_reproducciones,
    COALESCE(s.total_comentarios, 0) AS total_comentarios,
    s.publicado_at,
    u.id AS creator_id,
    u.username AS creator_username,
    u.nombre_visible AS creator_nombre_visible,
    u.avatar_url AS creator_avatar_url,
    COALESCE(u.verificado, FALSE) AS creator_verificado,
    COALESCE(s.metadata, '{}'::jsonb) AS metadata";

#[derive(Debug, FromRow)]
struct BibliotecaSampleRow {
    id: i32,
    id_corto: Option<String>,
    slug: String,
    titulo: String,
    descripcion: String,
    bpm: Option<i32>,
    music_key: Option<String>,
    escala: Option<String>,
    duracion: f32,
    formato: String,
    tags: Vec<String>,
    tipo: String,
    es_premium: bool,
    precio: Option<f64>,
    verificado: bool,
    ruta_preview: Option<String>,
    ruta_waveform: Option<String>,
    imagen_url: Option<String>,
    total_descargas: i32,
    total_likes: i32,
    total_reproducciones: i32,
    total_comentarios: i32,
    publicado_at: Option<DateTime<Utc>>,
    creator_id: i32,
    creator_username: String,
    creator_nombre_visible: Option<String>,
    creator_avatar_url: Option<String>,
    creator_verificado: bool,
    metadata: Value,
}

impl From<BibliotecaSampleRow> for SampleCatalogSummaryRecord {
    fn from(row: BibliotecaSampleRow) -> Self {
        Self {
            id: row.id,
            id_corto: row.id_corto,
            slug: row.slug,
            titulo: row.titulo,
            descripcion: row.descripcion,
            bpm: row.bpm,
            music_key: row.music_key,
            escala: row.escala,
            duracion: row.duracion,
            formato: row.formato,
            tags: row.tags,
            tipo: row.tipo,
            es_premium: row.es_premium,
            precio: row.precio,
            verificado: row.verificado,
            ruta_preview: row.ruta_preview,
            ruta_waveform: row.ruta_waveform,
            imagen_url: row.imagen_url,
            total_descargas: row.total_descargas,
            total_likes: row.total_likes,
            total_reproducciones: row.total_reproducciones,
            total_comentarios: row.total_comentarios,
            publicado_at: row.publicado_at,
            creator_id: row.creator_id,
            creator_username: row.creator_username,
            creator_nombre_visible: row.creator_nombre_visible,
            creator_avatar_url: row.creator_avatar_url,
            creator_verificado: row.creator_verificado,
            metadata: row.metadata,
            /* [296A-1] Biblioteca ya es del usuario — siempre guardado en sus propias colecciones.
             * Se fuerza None porque el contexto de biblioteca no requiere este flag. */
            ya_guardado_en_coleccion: None,
            /* [296A-3] Biblioteca no incluye reaccion del usuario — es su propio contenido.
             * Los filtros de reaccion se aplican en WHERE, no en el SELECT. */
            liked: None,
            reaccion: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CarpetaRow {
    pub primaria: String,
    pub secundaria: Option<String>,
    pub total: i64,
}

pub struct BibliotecaRepository;

impl BibliotecaRepository {
    /* Aplica los filtros de carpeta, busqueda y reaccion al WHERE de la query.
     * El usuario_id ya esta bind como :uid en el FROM/JOIN previo. */
    fn apply_filters(builder: &mut QueryBuilder<'_, Postgres>, f: &ColeccionadosFilters) {
        if !f.carpeta.is_empty() {
            if let Some((pri, sec)) = f.carpeta.split_once('/') {
                builder.push(" AND COALESCE(s.metadata->>'carpeta_primaria', ");
                builder.push_bind(CARPETA_DEFAULT);
                builder.push(") = ");
                builder.push_bind(pri.to_string());
                builder.push(" AND s.metadata->>'carpeta_secundaria' = ");
                builder.push_bind(sec.to_string());
            } else {
                builder.push(" AND COALESCE(s.metadata->>'carpeta_primaria', ");
                builder.push_bind(CARPETA_DEFAULT);
                builder.push(") = ");
                builder.push_bind(f.carpeta.clone());
            }
        }

        if !f.busqueda.is_empty() {
            let like = format!("%{}%", f.busqueda);
            builder.push(" AND (s.titulo ILIKE ");
            builder.push_bind(like.clone());
            builder.push(" OR EXISTS (SELECT 1 FROM UNNEST(s.tags) tag WHERE tag ILIKE ");
            builder.push_bind(like);
            builder.push("))");
        }

        if let Some(reac) = f.filtro_reaccion {
            builder.push(
                " AND EXISTS (SELECT 1 FROM likes l2 WHERE l2.target_id = s.id \
                 AND l2.tipo = 'sample' AND l2.usuario_id = ",
            );
            builder.push_bind(f.user_id);
            builder.push(" AND l2.reaccion = ");
            builder.push_bind(reac.db_value());
            builder.push(")");
        }
    }

    fn push_order(builder: &mut QueryBuilder<'_, Postgres>, orden: &str) {
        /* Subset minimo del legacy OrdenamientoHelper: solo "recientes"
         * (default). Otros valores caen al mismo orden por ahora —
         * si el frontend requiere mas, extender. */
        match orden {
            "antiguos" => builder.push(
                " ORDER BY GREATEST(COALESCE(d.created_at, '1970-01-01'::timestamp), s.publicado_at) ASC",
            ),
            "populares" => builder.push(" ORDER BY s.total_likes DESC, s.id DESC"),
            "descargas" => builder.push(" ORDER BY s.total_descargas DESC, s.id DESC"),
            _ => builder.push(
                " ORDER BY GREATEST(COALESCE(d.created_at, '1970-01-01'::timestamp), s.publicado_at) DESC",
            ),
        };
    }
}

/* [274A-6] Tags + BPM + key extraídos de un sample, materia prima del
 * algoritmo de sugerencias agregadas. */
#[derive(Debug, Clone, FromRow)]
pub struct SampleContextRow {
    pub tags: Vec<String>,
    pub bpm: Option<i32>,
    pub music_key: Option<String>,
}
