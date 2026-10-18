/* [254A-7b] Repositorio para full sync de colecciones+samples del usuario.
 * Replica SyncRepository::coleccionesConSamples + descargasSinColeccion (PHP legacy).
 * - 1 sola query con CTE + json_agg para colecciones (evita N+1).
 * - DISTINCT ON + LEFT JOIN descargas para samples sueltos (descargados o subidos).
 * - Filtra samples por estado='activo' y colecciones por eliminado_en IS NULL. */

use sqlx::PgPool;

use crate::errors::AppError;
use crate::models::{SyncColeccion, SyncSample};

pub struct SyncFullRepository;

impl SyncFullRepository {
    pub async fn colecciones_con_samples(
        pool: &PgPool,
        usuario_id: i32,
    ) -> Result<Vec<SyncColeccion>, AppError> {
        let rows = sqlx::query!(
            r#"
            WITH samples_por_coleccion AS (
                SELECT
                    cs.coleccion_id,
                    json_agg(
                        json_build_object(
                            'id', s.id,
                            'titulo', s.titulo,
                            'formato', s.formato,
                            'tamano', s.tamano,
                            'imagenUrl', s.imagen_url
                        ) ORDER BY cs.orden ASC, cs.added_at DESC
                    ) AS samples_json
                FROM coleccion_samples cs
                JOIN samples s ON cs.sample_id = s.id
                WHERE s.estado = 'activo' AND s.eliminado_en IS NULL
                GROUP BY cs.coleccion_id
            )
            SELECT
                c.id              AS "id!: i64",
                c.nombre          AS "nombre!: String",
                c.parent_id       AS "parent_id?: i64",
                c.version         AS "version!: i32",
                COALESCE(spc.samples_json, '[]'::json) AS "samples_json!: serde_json::Value"
            FROM colecciones c
            LEFT JOIN samples_por_coleccion spc ON spc.coleccion_id = c.id
            WHERE c.usuario_id = $1 AND c.eliminado_en IS NULL
            ORDER BY c.updated_at DESC
            "#,
            usuario_id
        )
        .fetch_all(pool)
        .await
        .map_err(AppError::from)?;

        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let samples: Vec<SyncSample> = serde_json::from_value(r.samples_json)
                .map_err(|e| AppError::Internal(format!("samples_json parse: {e}")))?;
            out.push(SyncColeccion {
                id: r.id,
                nombre: r.nombre,
                parent_id: r.parent_id,
                version: r.version,
                samples,
            });
        }
        Ok(out)
    }

    pub async fn descargas_sin_coleccion(
        pool: &PgPool,
        usuario_id: i32,
    ) -> Result<Vec<SyncSample>, AppError> {
        let rows = sqlx::query!(
            r#"
            SELECT DISTINCT ON (s.id)
                s.id         AS "id!: i32",
                s.titulo     AS "titulo!: String",
                s.formato    AS "formato!: String",
                s.tamano     AS "tamano!: i64",
                s.imagen_url AS "imagen_url?: String"
            FROM samples s
            LEFT JOIN descargas d
                   ON d.sample_id = s.id
                  AND d.usuario_id = $1
            WHERE s.estado = 'activo'
              AND s.eliminado_en IS NULL
              AND (
                    d.usuario_id IS NOT NULL
                    OR s.creador_id = $1
              )
              AND NOT EXISTS (
                  SELECT 1 FROM coleccion_samples cs
                  JOIN colecciones c ON cs.coleccion_id = c.id
                  WHERE cs.sample_id = s.id
                    AND c.usuario_id = $1
                    AND c.eliminado_en IS NULL
              )
            ORDER BY s.id, d.created_at DESC NULLS LAST, s.updated_at DESC
            "#,
            usuario_id
        )
        .fetch_all(pool)
        .await
        .map_err(AppError::from)?;

        Ok(rows
            .into_iter()
            .map(|r| SyncSample {
                id: r.id,
                titulo: r.titulo,
                formato: r.formato,
                tamano: r.tamano,
                imagen_url: r.imagen_url,
            })
            .collect())
    }
}
