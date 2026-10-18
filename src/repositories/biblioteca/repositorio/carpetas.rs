use sqlx::PgPool;

use crate::errors::AppError;

use super::{BibliotecaRepository, CARPETA_DEFAULT, CarpetaRow, SampleContextRow};

impl BibliotecaRepository {
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
