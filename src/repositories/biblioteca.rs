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
 * sentinel-disable-file limite-lineas — repositorio legacy central de biblioteca
 * en proceso de migracion; dividirlo durante estos endpoints multiplicaria el
 * blast radius. Las queries dinamicas estan parametrizadas con QueryBuilder/bind.
 */

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};

use crate::errors::AppError;
use crate::repositories::SampleCatalogSummaryRecord;

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

    /* [274A-15] Descargas del usuario: solo samples descargados (no propios).
     * INNER JOIN con `descargas` filtrado por usuario. Ordena por created_at
     * de la descarga por defecto. */
    pub async fn descargados_de_usuario(
        pool: &PgPool,
        f: &ColeccionadosFilters,
    ) -> Result<Vec<SampleCatalogSummaryRecord>, AppError> {
        let mut b = QueryBuilder::<Postgres>::new(BIBLIOTECA_SAMPLE_SELECT);
        b.push(" FROM samples s INNER JOIN usuarios_ext u ON u.id = s.creador_id");
        b.push(" INNER JOIN descargas d ON d.sample_id = s.id AND d.usuario_id = ");
        b.push_bind(f.user_id);
        b.push(" WHERE s.estado = 'activo' AND s.eliminado_en IS NULL");
        match f.orden.as_str() {
            "antiguas" => {
                b.push(" ORDER BY d.created_at ASC, s.id ASC");
            }
            "populares" => {
                b.push(" ORDER BY s.total_likes DESC, s.id DESC");
            }
            "descargas" => {
                b.push(" ORDER BY s.total_descargas DESC, s.id DESC");
            }
            _ => {
                b.push(" ORDER BY d.created_at DESC, s.id DESC");
            }
        }
        b.push(" LIMIT ");
        b.push_bind(f.per_page);
        b.push(" OFFSET ");
        b.push_bind(f.offset());

        let rows = b
            .build_query_as::<BibliotecaSampleRow>()
            .fetch_all(pool)
            .await
            .map_err(AppError::from)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn contar_descargados(pool: &PgPool, user_id: i32) -> Result<i64, AppError> {
        // COUNT(*) siempre devuelve una fila; "total!" fuerza el tipo no nulo i64.
        let total = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "total!" FROM descargas d
               INNER JOIN samples s ON s.id = d.sample_id
               WHERE d.usuario_id = $1 AND s.estado = 'activo' AND s.eliminado_en IS NULL"#,
            user_id
        )
        .fetch_one(pool)
        .await
        .map_err(AppError::from)?;
        Ok(total)
    }

    /* [274A-17] Historial de reproducciones del usuario, samples completos
     * ordenados por reproducción más reciente. Port de
     * ReproduccionesRepository::historialUsuario. */
    pub async fn historial_reproducciones(
        pool: &PgPool,
        user_id: i32,
        per_page: i64,
        offset: i64,
    ) -> Result<Vec<SampleCatalogSummaryRecord>, AppError> {
        let mut b = QueryBuilder::<Postgres>::new(BIBLIOTECA_SAMPLE_SELECT);
        b.push(
            " FROM samples s \
             INNER JOIN usuarios_ext u ON u.id = s.creador_id \
             INNER JOIN reproducciones r ON r.sample_id = s.id AND r.usuario_id = ",
        );
        b.push_bind(user_id);
        b.push(
            " WHERE s.estado = 'activo' AND s.eliminado_en IS NULL \
             GROUP BY s.id, u.id, u.username, u.nombre_visible, u.avatar_url, u.verificado \
             ORDER BY MAX(r.created_at) DESC LIMIT ",
        );
        b.push_bind(per_page);
        b.push(" OFFSET ");
        b.push_bind(offset);

        let rows = b
            .build_query_as::<BibliotecaSampleRow>()
            .fetch_all(pool)
            .await
            .map_err(AppError::from)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn coleccionados_de_usuario(
        pool: &PgPool,
        f: &ColeccionadosFilters,
    ) -> Result<Vec<SampleCatalogSummaryRecord>, AppError> {
        let mut b = QueryBuilder::<Postgres>::new(BIBLIOTECA_SAMPLE_SELECT);
        b.push(" FROM samples s INNER JOIN usuarios_ext u ON u.id = s.creador_id");
        b.push(" LEFT JOIN descargas d ON d.sample_id = s.id AND d.usuario_id = ");
        b.push_bind(f.user_id);
        b.push(" WHERE s.estado != 'eliminado' AND s.eliminado_en IS NULL");
        b.push(" AND ((s.creador_id = ");
        b.push_bind(f.user_id);
        b.push(") OR (d.id IS NOT NULL AND s.estado = 'activo'))");

        Self::apply_filters(&mut b, f);
        Self::push_order(&mut b, &f.orden);

        b.push(" LIMIT ");
        b.push_bind(f.per_page);
        b.push(" OFFSET ");
        b.push_bind(f.offset());

        let rows = b
            .build_query_as::<BibliotecaSampleRow>()
            .fetch_all(pool)
            .await
            .map_err(AppError::from)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn contar_coleccionados(
        pool: &PgPool,
        f: &ColeccionadosFilters,
    ) -> Result<i64, AppError> {
        let mut b = QueryBuilder::<Postgres>::new("SELECT COUNT(DISTINCT s.id) AS total");
        b.push(" FROM samples s");
        b.push(" LEFT JOIN descargas d ON d.sample_id = s.id AND d.usuario_id = ");
        b.push_bind(f.user_id);
        b.push(" WHERE s.estado != 'eliminado' AND s.eliminado_en IS NULL");
        b.push(" AND ((s.creador_id = ");
        b.push_bind(f.user_id);
        b.push(") OR (d.id IS NOT NULL AND s.estado = 'activo'))");

        Self::apply_filters(&mut b, f);

        let row = b
            .build_query_as::<CountRow>()
            .fetch_one(pool)
            .await
            .map_err(AppError::from)?;
        Ok(row.total)
    }

    /* [274A-7] Favoritos del usuario: samples a los que dio like/encanta.
     * INNER JOIN con `likes` filtrado por tipo='sample' y usuario. El filtro
     * de reaccion en ColeccionadosFilters refina:
     *   - Encanta -> reaccion = 'encanta'
     *   - Like    -> reaccion = 'like'
     *   - None    -> reaccion IN ('like','encanta')  (se excluye 'dislike')
     * Orden "recientes" usa l.created_at (cuando dio like), no s.publicado_at. */
    pub async fn favoritos_de_usuario(
        pool: &PgPool,
        f: &ColeccionadosFilters,
    ) -> Result<Vec<SampleCatalogSummaryRecord>, AppError> {
        let mut b = QueryBuilder::<Postgres>::new(BIBLIOTECA_SAMPLE_SELECT);
        b.push(" FROM samples s INNER JOIN usuarios_ext u ON u.id = s.creador_id");
        b.push(
            " INNER JOIN likes l ON l.target_id = s.id AND l.tipo = 'sample' AND l.usuario_id = ",
        );
        b.push_bind(f.user_id);
        b.push(" WHERE s.estado = 'activo' AND s.eliminado_en IS NULL");
        Self::push_favoritos_reaccion(&mut b, f.filtro_reaccion);
        Self::apply_favoritos_busqueda(&mut b, f);
        Self::push_favoritos_order(&mut b, &f.orden);
        b.push(" LIMIT ");
        b.push_bind(f.per_page);
        b.push(" OFFSET ");
        b.push_bind(f.offset());

        let rows = b
            .build_query_as::<BibliotecaSampleRow>()
            .fetch_all(pool)
            .await
            .map_err(AppError::from)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn contar_favoritos(
        pool: &PgPool,
        f: &ColeccionadosFilters,
    ) -> Result<i64, AppError> {
        let mut b = QueryBuilder::<Postgres>::new("SELECT COUNT(*) AS total");
        b.push(" FROM samples s");
        b.push(
            " INNER JOIN likes l ON l.target_id = s.id AND l.tipo = 'sample' AND l.usuario_id = ",
        );
        b.push_bind(f.user_id);
        b.push(" WHERE s.estado = 'activo' AND s.eliminado_en IS NULL");
        Self::push_favoritos_reaccion(&mut b, f.filtro_reaccion);
        Self::apply_favoritos_busqueda(&mut b, f);

        let row = b
            .build_query_as::<CountRow>()
            .fetch_one(pool)
            .await
            .map_err(AppError::from)?;
        Ok(row.total)
    }

    fn push_favoritos_reaccion(b: &mut QueryBuilder<'_, Postgres>, reac: Option<FiltroReaccion>) {
        match reac {
            Some(r) => {
                b.push(" AND l.reaccion = ");
                b.push_bind(r.db_value());
            }
            None => {
                b.push(" AND l.reaccion IN ('like','encanta')");
            }
        }
    }

    fn apply_favoritos_busqueda(b: &mut QueryBuilder<'_, Postgres>, f: &ColeccionadosFilters) {
        if !f.busqueda.is_empty() {
            let like = format!("%{}%", f.busqueda);
            b.push(" AND (s.titulo ILIKE ");
            b.push_bind(like.clone());
            b.push(" OR EXISTS (SELECT 1 FROM UNNEST(s.tags) tag WHERE tag ILIKE ");
            b.push_bind(like);
            b.push("))");
        }
    }

    fn push_favoritos_order(b: &mut QueryBuilder<'_, Postgres>, orden: &str) {
        match orden {
            "antiguos" => b.push(" ORDER BY l.created_at ASC, s.id ASC"),
            "populares" => b.push(" ORDER BY s.total_likes DESC, s.id DESC"),
            "descargas" => b.push(" ORDER BY s.total_descargas DESC, s.id DESC"),
            _ => b.push(" ORDER BY l.created_at DESC, s.id DESC"),
        };
    }

    pub async fn carpetas_coleccionados(
        pool: &PgPool,
        user_id: i32,
    ) -> Result<Vec<CarpetaRow>, AppError> {
        let rows = sqlx::query!(
            r#"
            SELECT
                COALESCE(s.metadata->>'carpeta_primaria', $3)  AS "primaria!: String",
                s.metadata->>'carpeta_secundaria'              AS "secundaria?: String",
                COUNT(*)                                       AS "total!: i64"
            FROM (
                SELECT s.id, s.metadata FROM samples s
                JOIN descargas d ON d.sample_id = s.id AND d.usuario_id = $1
                WHERE s.estado = 'activo' AND s.eliminado_en IS NULL
                UNION
                SELECT s.id, s.metadata FROM samples s
                WHERE s.creador_id = $2 AND s.estado != 'eliminado' AND s.eliminado_en IS NULL
            ) s
            GROUP BY COALESCE(s.metadata->>'carpeta_primaria', $3),
                     s.metadata->>'carpeta_secundaria'
            ORDER BY 1, 2
            "#,
            user_id,
            user_id,
            CARPETA_DEFAULT
        )
        .fetch_all(pool)
        .await
        .map_err(AppError::from)?;

        Ok(rows
            .into_iter()
            .map(|r| CarpetaRow {
                primaria: r.primaria,
                secundaria: r.secundaria,
                total: r.total,
            })
            .collect())
    }

    /* Verifica que el usuario es creador o lo descargo (activo). */
    pub async fn es_coleccionado_por_usuario(
        pool: &PgPool,
        sample_id: i32,
        user_id: i32,
    ) -> Result<bool, AppError> {
        let row = sqlx::query!(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM samples s
                WHERE s.id = $1 AND s.creador_id = $2
                UNION
                SELECT 1 FROM samples s
                JOIN descargas d ON d.sample_id = s.id AND d.usuario_id = $2
                WHERE s.id = $1 AND s.estado = 'activo'
            ) AS "existe!: bool"
            "#,
            sample_id,
            user_id
        )
        .fetch_one(pool)
        .await
        .map_err(AppError::from)?;
        Ok(row.existe)
    }

    /* Mueve el sample a una nueva carpeta usando jsonb_set encadenado para no
     * perder otras claves del metadata. */
    pub async fn mover_a_carpeta(
        pool: &PgPool,
        sample_id: i32,
        primaria: &str,
        secundaria: &str,
    ) -> Result<bool, AppError> {
        let pri_json = serde_json::Value::String(primaria.to_string());
        let sec_json = serde_json::Value::String(secundaria.to_string());

        let result = sqlx::query!(
            r#"
            UPDATE samples
            SET metadata = jsonb_set(
                jsonb_set(COALESCE(metadata, '{}'::jsonb), '{carpeta_primaria}', $2::jsonb),
                '{carpeta_secundaria}', $3::jsonb
            ),
            updated_at = NOW()
            WHERE id = $1
            "#,
            sample_id,
            pri_json,
            sec_json
        )
        .execute(pool)
        .await
        .map_err(AppError::from)?;

        Ok(result.rows_affected() > 0)
    }

    /* [274A-6] Contexto y exclusiones para sugerencias "Más Ideas".
     *
     * Replica DescargasRepository::contextoDescargas/idsDescargados +
     * ColeccionSamplesRepository::contextoColeccionadosUsuario/idsColeccionadosUsuario
     * del legacy. El handler de sugerencias agrega ambos contextos para
     * calcular top tags, BPM promedio y key dominante; los IDs se usan
     * para excluir lo que el usuario ya conoce. */
    pub async fn contexto_descargas(
        pool: &PgPool,
        user_id: i32,
    ) -> Result<Vec<SampleContextRow>, AppError> {
        let rows = sqlx::query_as!(
            SampleContextRow,
            r#"
            SELECT
                COALESCE(s.tags, ARRAY[]::text[]) AS "tags!: Vec<String>",
                s.bpm AS "bpm?",
                s.key AS "music_key?"
            FROM samples s
            JOIN descargas d ON d.sample_id = s.id
            WHERE d.usuario_id = $1
              AND s.estado = 'activo'
              AND s.eliminado_en IS NULL
            "#,
            user_id
        )
        .fetch_all(pool)
        .await
        .map_err(AppError::from)?;
        Ok(rows)
    }

    pub async fn ids_descargados(pool: &PgPool, user_id: i32) -> Result<Vec<i32>, AppError> {
        let rows = sqlx::query_scalar!(
            r#"SELECT sample_id AS "sample_id!" FROM descargas WHERE usuario_id = $1"#,
            user_id
        )
        .fetch_all(pool)
        .await
        .map_err(AppError::from)?;
        Ok(rows)
    }

    pub async fn contexto_coleccionados(
        pool: &PgPool,
        user_id: i32,
    ) -> Result<Vec<SampleContextRow>, AppError> {
        let rows = sqlx::query_as!(
            SampleContextRow,
            r#"
            SELECT
                COALESCE(s.tags, ARRAY[]::text[]) AS "tags!: Vec<String>",
                s.bpm AS "bpm?",
                s.key AS "music_key?"
            FROM samples s
            JOIN coleccion_samples cs ON cs.sample_id = s.id
            JOIN colecciones c ON c.id = cs.coleccion_id
            WHERE c.usuario_id = $1
              AND c.eliminado_en IS NULL
              AND s.estado = 'activo'
              AND s.eliminado_en IS NULL
            "#,
            user_id
        )
        .fetch_all(pool)
        .await
        .map_err(AppError::from)?;
        Ok(rows)
    }

    pub async fn ids_coleccionados(pool: &PgPool, user_id: i32) -> Result<Vec<i32>, AppError> {
        let rows = sqlx::query_scalar!(
            r#"
            SELECT DISTINCT cs.sample_id AS "sample_id!"
            FROM coleccion_samples cs
            JOIN colecciones c ON c.id = cs.coleccion_id
            WHERE c.usuario_id = $1
              AND c.eliminado_en IS NULL
            "#,
            user_id
        )
        .fetch_all(pool)
        .await
        .map_err(AppError::from)?;
        Ok(rows)
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

/* [274A-20] Contexto + IDs basados en likes (favoritos) para sugerencias.
 * Port de SugerenciasController::sugerenciasFavoritos + LikesRepository. */
impl BibliotecaRepository {
    pub async fn contexto_favoritos(
        pool: &PgPool,
        user_id: i32,
    ) -> Result<Vec<SampleContextRow>, AppError> {
        let rows = sqlx::query!(
            "SELECT s.tags, s.bpm, s.key \
             FROM samples s \
             JOIN likes l ON l.target_id = s.id AND l.tipo = 'sample' \
             WHERE l.usuario_id = $1 \
               AND l.reaccion IN ('like','encanta') \
               AND s.estado = 'activo' AND s.eliminado_en IS NULL",
            user_id
        )
        .fetch_all(pool)
        .await
        .map_err(AppError::from)?;
        Ok(rows
            .into_iter()
            .map(|r| SampleContextRow {
                tags: r.tags.unwrap_or_default(),
                bpm: r.bpm,
                music_key: r.key,
            })
            .collect())
    }

    pub async fn ids_favoritos(pool: &PgPool, user_id: i32) -> Result<Vec<i32>, AppError> {
        let ids = sqlx::query_scalar!(
            "SELECT target_id FROM likes WHERE usuario_id = $1 AND tipo = 'sample' \
             AND reaccion IN ('like','encanta')",
            user_id
        )
        .fetch_all(pool)
        .await
        .map_err(AppError::from)?;
        Ok(ids)
    }
}
