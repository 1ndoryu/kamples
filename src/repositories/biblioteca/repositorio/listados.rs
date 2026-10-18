use sqlx::{PgPool, Postgres, QueryBuilder};

use crate::errors::AppError;
use crate::repositories::SampleCatalogSummaryRecord;

use super::{
    BIBLIOTECA_SAMPLE_SELECT, BibliotecaRepository, BibliotecaSampleRow, ColeccionadosFilters,
    CountRow, FiltroReaccion, SampleContextRow,
};

impl BibliotecaRepository {
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
