use sqlx::PgPool;

/* [254A-8c-refactor] Repositorio de cola_extraccion_samples para extensión.
 *
 * Cambio vs. versión inicial: ya no recortamos en Rust. Solo encolamos para
 * que el scraper Python procese. `encolar_para_scraper` deja la fila en
 * estado='pendiente' con el nuevo timing y un flag `extension_modo` en
 * metadata_extraccion para que pipeline.py decida cómo tratarla
 * (extender, generar_siguiente, restaurar).
 *
 * Todas las consultas usan macros query!/query_as!/query_scalar! (validadas en
 * compilacion y cacheadas en .sqlx). */

pub struct ColaExtraccionRepository;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ColaExtraccionRow {
    pub id: i32,
    pub relacion_id: i32,
    pub youtube_id: Option<String>,
    pub compas_inicio_seg: Option<f64>,
    pub compas_fin_seg: Option<f64>,
    pub ruta_audio_completo: Option<String>,
    pub sample_id: Option<i32>,
}

/* [264A-3] Fila reclamada por el publicador Rust. Incluye todo lo necesario
 * para crear el sample (ruta del audio extraido, metadata, lado, sample_id
 * si la fila ya estaba vinculada — caso extender/restaurar). */
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ColaExtraidoReclamado {
    pub id: i32,
    pub relacion_id: i32,
    pub lado: String,
    pub youtube_id: Option<String>,
    pub spotify_id: Option<String>,
    pub ruta_audio_extraido: Option<String>,
    pub ruta_audio_completo: Option<String>,
    pub metadata_extraccion: Option<serde_json::Value>,
    pub sample_id: Option<i32>,
    pub bpm_detectado: Option<i16>,
    pub compas_inicio_seg: Option<f64>,
    pub compas_fin_seg: Option<f64>,
}

/// Conteos agregados de la cola para el panel de administracion.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EstadisticasCola {
    pub pendientes: i64,
    pub en_proceso: i64,
    pub revision_humana: i64,
    pub extraido_sin_publicar: i64,
    pub completados_24h: i64,
    pub errores_24h: i64,
    pub total: i64,
}

pub struct EncolarParams<'a> {
    pub cola_id: i32,
    pub nuevo_inicio: f64,
    pub nuevo_fin: f64,
    /// Una de: "extender", "generar_siguiente", "restaurar".
    pub modo: &'a str,
}

impl ColaExtraccionRepository {
    /// Busca la fila de cola_extraccion_samples vinculada a un sample.
    pub async fn find_by_sample_id(
        pool: &PgPool,
        sample_id: i32,
    ) -> Result<Option<ColaExtraccionRow>, sqlx::Error> {
        sqlx::query_as!(
            ColaExtraccionRow,
            r#"SELECT
                id AS "id!",
                relacion_id AS "relacion_id!",
                youtube_id,
                compas_inicio_seg::DOUBLE PRECISION AS compas_inicio_seg,
                compas_fin_seg::DOUBLE PRECISION    AS compas_fin_seg,
                ruta_audio_completo,
                sample_id
             FROM cola_extraccion_samples
             WHERE sample_id = $1
             LIMIT 1"#,
            sample_id
        )
        .fetch_optional(pool)
        .await
    }

    /// Calcula todos los conteos del panel en una sola pasada con FILTER,
    /// para no hacer una consulta por estado.
    pub async fn estadisticas(pool: &PgPool) -> Result<EstadisticasCola, sqlx::Error> {
        sqlx::query_as!(
            EstadisticasCola,
            r#"SELECT
                  COUNT(*) FILTER (WHERE estado = 'pendiente')                                          AS "pendientes!",
                  COUNT(*) FILTER (WHERE estado IN ('descargando','analizando','recortando'))           AS "en_proceso!",
                  COUNT(*) FILTER (WHERE estado = 'revision_humana')                                    AS "revision_humana!",
                  COUNT(*) FILTER (WHERE estado = 'extraido' AND sample_id IS NULL)                     AS "extraido_sin_publicar!",
                  COUNT(*) FILTER (WHERE estado = 'completado' AND procesado_at >= NOW() - INTERVAL '24 hours') AS "completados_24h!",
                  COUNT(*) FILTER (WHERE estado = 'error'      AND procesado_at >= NOW() - INTERVAL '24 hours') AS "errores_24h!",
                  COUNT(*)                                                                              AS "total!"
              FROM cola_extraccion_samples"#
        )
        .fetch_one(pool)
        .await
    }

    /// Marca la fila como pendiente con el nuevo rango y el modo de extensión.
    /// El scraper Python (pipeline.py) leerá `metadata_extraccion.extension_modo`
    /// para decidir cómo tratar el item.
    pub async fn encolar_para_scraper(
        pool: &PgPool,
        params: EncolarParams<'_>,
    ) -> Result<(), sqlx::Error> {
        let metadata_patch = serde_json::json!({
            "extension_modo": params.modo,
            "extension_solicitada_at": chrono::Utc::now().to_rfc3339(),
            "extension_nuevo_inicio": params.nuevo_inicio,
            "extension_nuevo_fin": params.nuevo_fin,
        });

        sqlx::query!(
            "UPDATE cola_extraccion_samples
             SET compas_inicio_seg = $2::float8,
                 compas_fin_seg    = $3::float8,
                 estado            = 'pendiente',
                 intentos          = 0,
                 error_mensaje     = NULL,
                 proximo_intento_at = NULL,
                 metadata_extraccion = COALESCE(metadata_extraccion, '{}'::jsonb) || $4::jsonb
             WHERE id = $1",
            params.cola_id,
            params.nuevo_inicio,
            params.nuevo_fin,
            metadata_patch
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /* [264A-3] Reclama atomicamente N filas en estado='extraido' que aun no
     * tienen sample_id asignado. Usa `procesado_at` como flag de "siendo
     * procesado por el publicador" para evitar que dos workers tomen la misma
     * fila. Si la publicacion falla, `revertir_reclamo` la regresa al pool. */
    pub async fn reclamar_para_publicar(
        pool: &PgPool,
        limit: i64,
    ) -> Result<Vec<ColaExtraidoReclamado>, sqlx::Error> {
        sqlx::query_as!(
            ColaExtraidoReclamado,
            r#"UPDATE cola_extraccion_samples
               SET procesado_at = NOW()
               WHERE id IN (
                   SELECT id FROM cola_extraccion_samples
                   WHERE estado = 'extraido'
                     AND sample_id IS NULL
                     AND procesado_at IS NULL
                     AND ruta_audio_extraido IS NOT NULL
                   ORDER BY id
                   LIMIT $1
                   FOR UPDATE SKIP LOCKED
               )
               RETURNING
                   id,
                   relacion_id,
                   lado AS "lado!",
                   youtube_id,
                   spotify_id,
                   ruta_audio_extraido,
                   ruta_audio_completo,
                   metadata_extraccion,
                   sample_id,
                   bpm_detectado,
                   compas_inicio_seg::DOUBLE PRECISION AS compas_inicio_seg,
                   compas_fin_seg::DOUBLE PRECISION    AS compas_fin_seg"#,
            limit
        )
        .fetch_all(pool)
        .await
    }

    /* [264A-3] Variante para los modos extender/restaurar: reclama filas que
     * YA tienen sample_id (apunta al sample existente). El publicador detecta
     * `metadata_extraccion.extension_modo` y reemplaza assets en lugar de crear
     * un sample nuevo. */
    pub async fn reclamar_para_reemplazar(
        pool: &PgPool,
        limit: i64,
    ) -> Result<Vec<ColaExtraidoReclamado>, sqlx::Error> {
        sqlx::query_as!(
            ColaExtraidoReclamado,
            r#"UPDATE cola_extraccion_samples
               SET procesado_at = NOW()
               WHERE id IN (
                   SELECT id FROM cola_extraccion_samples
                   WHERE estado = 'extraido'
                     AND sample_id IS NOT NULL
                     AND procesado_at IS NULL
                     AND ruta_audio_extraido IS NOT NULL
                     AND (metadata_extraccion ->> 'extension_modo') IN ('extender', 'restaurar')
                   ORDER BY id
                   LIMIT $1
                   FOR UPDATE SKIP LOCKED
               )
               RETURNING
                   id,
                   relacion_id,
                   lado AS "lado!",
                   youtube_id,
                   spotify_id,
                   ruta_audio_extraido,
                   ruta_audio_completo,
                   metadata_extraccion,
                   sample_id,
                   bpm_detectado,
                   compas_inicio_seg::DOUBLE PRECISION AS compas_inicio_seg,
                   compas_fin_seg::DOUBLE PRECISION    AS compas_fin_seg"#,
            limit
        )
        .fetch_all(pool)
        .await
    }

    /* [264A-3] Marca una fila publicada con exito: estado=completado, sample_id
     * vinculado, ruta_audio_extraido apuntando ahora a la storage_key persistente. */
    pub async fn marcar_completado(
        pool: &PgPool,
        cola_id: i32,
        sample_id: i32,
        ruta_storage: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE cola_extraccion_samples
               SET estado = 'completado',
                   sample_id = $2,
                   ruta_audio_extraido = $3,
                   procesado_at = NOW(),
                   error_mensaje = NULL
               WHERE id = $1",
            cola_id,
            sample_id,
            ruta_storage
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /* [264A-3] Revierte el reclamo cuando la publicacion falla: deja la fila
     * de vuelta en estado='extraido' con procesado_at=NULL para que el siguiente
     * ciclo del publicador la vuelva a tomar. Registra el mensaje de error. */
    pub async fn revertir_reclamo(
        pool: &PgPool,
        cola_id: i32,
        error: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE cola_extraccion_samples
               SET procesado_at = NULL,
                   error_mensaje = LEFT($2::text, 1000)
               WHERE id = $1",
            cola_id,
            error
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /* [294A-6] Canciones (fuente, destino) de una relación; None si no existe. */
    pub async fn canciones_de_relacion(
        pool: &PgPool,
        relacion_id: i32,
    ) -> Result<Option<(i32, i32)>, sqlx::Error> {
        let fila = sqlx::query!(
            r#"SELECT cancion_fuente_id AS "cancion_fuente_id!",
                      cancion_destino_id AS "cancion_destino_id!"
               FROM relaciones_sample
               WHERE id = $1"#,
            relacion_id
        )
        .fetch_optional(pool)
        .await?;
        Ok(fila.map(|f| (f.cancion_fuente_id, f.cancion_destino_id)))
    }

    /* [294A-6] Vincula el sample al lado fuente o destino de la relación y fija
     * samples.relacion_sampleo_id / cancion_origen_id. El UPDATE de la relación
     * es una sola sentencia: la columna del otro lado queda igual a sí misma. */
    pub async fn vincular_lado(
        pool: &PgPool,
        relacion_id: i32,
        sample_id: i32,
        es_fuente: bool,
        cancion_id: i32,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE relaciones_sample
             SET sample_fuente_id  = CASE WHEN $3 THEN $2 ELSE sample_fuente_id END,
                 sample_destino_id = CASE WHEN $3 THEN sample_destino_id ELSE $2 END,
                 updated_at        = NOW()
             WHERE id = $1",
            relacion_id,
            sample_id,
            es_fuente
        )
        .execute(pool)
        .await?;

        sqlx::query!(
            "UPDATE samples
             SET relacion_sampleo_id = $2,
                 cancion_origen_id   = $3
             WHERE id = $1",
            sample_id,
            relacion_id,
            cancion_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /* [294A-6] Hereda canciones.imagen_url al sample (paridad con
     * PublicadorExtraccion::publicarItem PHP). Solo si el sample no tiene imagen
     * y la canción la tiene no vacía. */
    pub async fn heredar_imagen(
        pool: &PgPool,
        sample_id: i32,
        cancion_id: i32,
    ) -> Result<(), sqlx::Error> {
        let imagen: Option<String> = sqlx::query_scalar!(
            "SELECT imagen_url FROM canciones WHERE id = $1",
            cancion_id
        )
        .fetch_optional(pool)
        .await?
        .flatten();

        if let Some(url) = imagen.filter(|v| !v.trim().is_empty()) {
            sqlx::query!(
                "UPDATE samples SET imagen_url = $2 WHERE id = $1 AND imagen_url IS NULL",
                sample_id,
                url
            )
            .execute(pool)
            .await?;
        }
        Ok(())
    }
}
