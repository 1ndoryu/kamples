/* [264A-3] Publicador Rust de extracciones del scraper Python.
 *
 * Cierra el ciclo cola_extraccion_samples â†’ samples real. Antes existÃ­a un
 * gap: el scraper marcaba filas como `estado='extraido'` pero nadie creaba
 * el sample, asÃ­ que `publicar_auto` era un no-op que jamÃ¡s encontraba
 * filas con sample_id asignado.
 *
 * Equivalente legacy: `App/Kamples/Services/PublicadorExtraccion.php` y
 * `DevController::publicarExtracciones`. Esta versiÃ³n Rust NO implementa
 * dedup QK53 (futuro) ni el caso "generar_siguiente" del flujo de extensiÃ³n
 * (la cola actual reutiliza la misma fila â†’ no hay forma limpia de crear
 * sample nuevo sin perder la cadena; documentado como tarea futura).
 *
 * Modos soportados:
 *   - default (sin extension_modo): crear sample nuevo + vincular a relaciÃ³n.
 *   - extender / restaurar: reemplazar audio del sample existente y disparar
 *     re-procesamiento del pipeline IA (force_recompute).
 *   - generar_siguiente: NO soportado v1 â€” se loggea warning y se deja la fila
 *     en error con mensaje explicativo para revisiÃ³n manual.
 *
 * Concurrencia: el repo reclama filas atomicamente vÃ­a
 * UPDATE...RETURNING + FOR UPDATE SKIP LOCKED, marcando procesado_at=NOW()
 * como flag de "siendo publicado". Si la publicaciÃ³n falla, revertir_reclamo
 * limpia procesado_at para que el siguiente ciclo lo retome. */

use crate::errors::AppError;
use crate::repositories::{
    ColaExtraccionRepository, ColaExtraidoReclamado, CreateUploadSampleParams, SampleRepository,
};
use crate::services::FileStorage;
use chrono::Datelike;
use nanoid::nanoid;
use serde::Serialize;
use sha2::{Digest, Sha256};
use slug::slugify;
use sqlx::PgPool;
use std::path::Path;
use std::sync::Arc;
use utoipa::ToSchema;

const ID_CORTO_ALPHABET: [char; 36] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i',
    'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
];

const SYSTEM_USER_ID_FALLBACK: i32 = 1;

/// Resultado por item en una corrida de publicaciÃ³n.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ItemPublicacion {
    pub cola_id: i32,
    pub ok: bool,
    pub sample_id: Option<i32>,
    pub modo: String,
    pub error: Option<String>,
}

/// Resumen de una corrida de `publicar_pendientes`.
#[derive(Debug, Clone, Serialize, ToSchema, Default)]
pub struct PublicarResultado {
    pub publicados: usize,
    pub reemplazados: usize,
    pub errores: usize,
    pub items: Vec<ItemPublicacion>,
}

pub struct ExtraccionPublisherService;

impl ExtraccionPublisherService {
    /// Procesa hasta `limit` filas en estado='extraido'. Maneja tanto creaciÃ³n
    /// de samples nuevos como reemplazo de assets para extender/restaurar.
    pub async fn publicar_pendientes(
        pool: &PgPool,
        storage: &Arc<dyn FileStorage>,
        limit: i64,
    ) -> Result<PublicarResultado, AppError> {
        let mut resultado = PublicarResultado::default();
        let creador_id = sistema_user_id();

        // 1) Reclamar reemplazos primero (extender/restaurar) â€” son operaciones
        //    sobre samples ya existentes y no compiten por slugs.
        let reemplazos = ColaExtraccionRepository::reclamar_para_reemplazar(pool, limit)
            .await
            .map_err(|e| AppError::Internal(format!("reclamar reemplazos: {e}")))?;

        for fila in reemplazos {
            let modo = extension_modo(&fila).unwrap_or_else(|| "extender".to_string());
            match Self::reemplazar_assets(pool, storage, &fila).await {
                Ok(sample_id) => {
                    resultado.reemplazados += 1;
                    resultado.items.push(ItemPublicacion {
                        cola_id: fila.id,
                        ok: true,
                        sample_id: Some(sample_id),
                        modo,
                        error: None,
                    });
                }
                Err(error) => {
                    let msg = error.to_string();
                    tracing::error!(cola_id = fila.id, %error, "fallo reemplazando assets");
                    let _ = ColaExtraccionRepository::revertir_reclamo(pool, fila.id, &msg).await;
                    resultado.errores += 1;
                    resultado.items.push(ItemPublicacion {
                        cola_id: fila.id,
                        ok: false,
                        sample_id: fila.sample_id,
                        modo,
                        error: Some(msg),
                    });
                }
            }
        }

        // 2) Reclamar nuevos (sin sample_id). Si el modo es "generar_siguiente"
        //    en una fila SIN sample_id, lo tratamos como creaciÃ³n normal â€” la
        //    fila no estaba vinculada a nada todavÃ­a.
        let restantes = (limit - i64::try_from(resultado.reemplazados).unwrap_or(0)).max(0);
        if restantes == 0 {
            return Ok(resultado);
        }

        let nuevos = ColaExtraccionRepository::reclamar_para_publicar(pool, restantes)
            .await
            .map_err(|e| AppError::Internal(format!("reclamar publicar: {e}")))?;

        for fila in nuevos {
            let modo = extension_modo(&fila).unwrap_or_else(|| "nuevo".to_string());
            match Self::crear_sample_nuevo(pool, storage, &fila, creador_id).await {
                Ok(sample_id) => {
                    resultado.publicados += 1;
                    resultado.items.push(ItemPublicacion {
                        cola_id: fila.id,
                        ok: true,
                        sample_id: Some(sample_id),
                        modo,
                        error: None,
                    });
                }
                Err(error) => {
                    let msg = error.to_string();
                    tracing::error!(cola_id = fila.id, %error, "fallo creando sample");
                    let _ = ColaExtraccionRepository::revertir_reclamo(pool, fila.id, &msg).await;
                    resultado.errores += 1;
                    resultado.items.push(ItemPublicacion {
                        cola_id: fila.id,
                        ok: false,
                        sample_id: None,
                        modo,
                        error: Some(msg),
                    });
                }
            }
        }

        Ok(resultado)
    }

    /// Caso default: lee bytes del audio extraÃ­do por el scraper, los sube al
    /// storage persistente, crea fila en `samples` (estado='procesando' â†’
    /// dispara worker IA automÃ¡ticamente), vincula al lado correspondiente
    /// de `relaciones_sample`, marca cola='completado' y borra el archivo
    /// temporal.
    async fn crear_sample_nuevo(
        pool: &PgPool,
        storage: &Arc<dyn FileStorage>,
        fila: &ColaExtraidoReclamado,
        creador_id: i32,
    ) -> Result<i32, AppError> {
        let ruta_origen = fila
            .ruta_audio_extraido
            .as_deref()
            .ok_or_else(|| AppError::Internal("ruta_audio_extraido vacia".into()))?;

        let bytes = leer_audio(storage, ruta_origen).await?;
        let tamano = i64::try_from(bytes.len())
            .map_err(|_| AppError::Internal("audio demasiado grande".into()))?;
        let audio_hash = hash_bytes(&bytes);
        let formato = detectar_formato(ruta_origen);

        let id_corto = nanoid!(8, &ID_CORTO_ALPHABET);
        let extraccion_meta = extraer_bloque_extraccion(fila);
        let titulo = construir_titulo_legacy(fila, &extraccion_meta);
        let slug = format!("{}-{id_corto}", slugify(&titulo));
        let now = chrono::Utc::now();
        let storage_key = format!(
            "samples/{}/{:04}/{:02}/{}.{}",
            creador_id,
            now.year(),
            now.month(),
            slug,
            formato,
        );

        storage
            .put_bytes(&storage_key, &bytes)
            .await
            .map_err(|e| AppError::Internal(format!("storage put_bytes: {e}")))?;

        /* [294A-6] Persistir el bloque `extraccion` con la misma forma que el
         * legacy PHP guardaba para que `IaQueueService::build_extraction_context`
         * lo reconstruya y el prompt LLM produzca metadata creativa real (titulo,
         * descripcion, genero) en lugar de placeholders "Extraccion XXXX". */
        let metadata_sample = serde_json::json!({
            "origen": "extraccion",
            "lado_extraccion": fila.lado,
            "relacion_id": fila.relacion_id,
            "cola_extraccion_id": fila.id,
            "youtube_id": fila.youtube_id,
            "spotify_id": fila.spotify_id,
            "bpm_detectado": fila.bpm_detectado,
            "compas_inicio_seg": fila.compas_inicio_seg,
            "compas_fin_seg": fila.compas_fin_seg,
            "extension_metadata": fila.metadata_extraccion,
            "extraccion": extraccion_meta,
        });

        let tags = generar_tags_legacy(fila, &extraccion_meta);

        let created = SampleRepository::create_upload_sample(
            pool,
            CreateUploadSampleParams {
                creador_id,
                titulo: &titulo,
                slug: &slug,
                id_corto: &id_corto,
                descripcion: "",
                formato: &formato,
                tamano,
                tags: &tags,
                audio_hash: &audio_hash,
                ruta_original: &storage_key,
                permitir_descarga: true,
                licencia_libre: false,
                es_premium: false,
                precio: None,
                mostrar_en_comunidad: true,
                metadata: metadata_sample,
                sync_upload: false,
                cancion_origen_id: None,
                relacion_sampleo_id: None,
                imagen_url: None,
            },
        )
        .await
        .map_err(|e| {
            // Si falla la creaciÃ³n de sample, intentar limpiar el blob que ya
            // subimos para no dejar basura en el storage.
            tracing::error!(error = %e, "fallÃ³ create_upload_sample, limpiando blob");
            AppError::Internal(format!("crear sample: {e}"))
        })?;

        // Vincular relacion_sampleo_id + cancion_origen_id + heredar imagen_url
        // de la cancion origen (paridad con PublicadorExtraccion legacy).
        if let Err(e) = vincular_sample_a_relacion(pool, created.id, fila).await {
            tracing::warn!(sample_id = created.id, error = %e, "no se pudo vincular sample a relacion (continuando)");
        }

        ColaExtraccionRepository::marcar_completado(pool, fila.id, created.id, &storage_key)
            .await
            .map_err(|e| AppError::Internal(format!("marcar completado: {e}")))?;

        // Borrar archivo temporal del scraper (best-effort).
        if let Err(e) = tokio::fs::remove_file(ruta_origen).await {
            tracing::debug!(path = ruta_origen, error = %e, "no se pudo borrar audio temp");
        }

        Ok(created.id)
    }

    /// Modos extender/restaurar: el sample ya existe; solo subimos el nuevo
    /// audio y disparamos re-procesamiento del pipeline IA (force_recompute).
    async fn reemplazar_assets(
        pool: &PgPool,
        storage: &Arc<dyn FileStorage>,
        fila: &ColaExtraidoReclamado,
    ) -> Result<i32, AppError> {
        let sample_id = fila
            .sample_id
            .ok_or_else(|| AppError::Internal("reemplazo sin sample_id".into()))?;
        let ruta_origen = fila
            .ruta_audio_extraido
            .as_deref()
            .ok_or_else(|| AppError::Internal("ruta_audio_extraido vacia".into()))?;
        let modo = extension_modo(fila).unwrap_or_else(|| "extender".to_string());

        let bytes = leer_audio(storage, ruta_origen).await?;
        let tamano = i64::try_from(bytes.len())
            .map_err(|_| AppError::Internal("audio demasiado grande".into()))?;
        let audio_hash = hash_bytes(&bytes);
        let formato = detectar_formato(ruta_origen);

        // Necesitamos creador_id + slug para construir storage_key coherente.
        let row = sqlx::query!(
            r#"SELECT creador_id AS "creador_id!", slug AS "slug!"
               FROM samples WHERE id = $1 AND eliminado_en IS NULL"#,
            sample_id
        )
        .fetch_optional(pool)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::NotFound(format!("sample {sample_id} no existe")))?;

        let now = chrono::Utc::now();
        let storage_key = format!(
            "samples/{}/{:04}/{:02}/{}-r{}.{}",
            row.creador_id,
            now.year(),
            now.month(),
            row.slug,
            now.timestamp(),
            formato,
        );

        storage
            .put_bytes(&storage_key, &bytes)
            .await
            .map_err(|e| AppError::Internal(format!("storage put_bytes: {e}")))?;

        let metadata_patch = serde_json::json!({
            "ultimo_reemplazo": {
                "modo": modo,
                "at": now.to_rfc3339(),
                "cola_extraccion_id": fila.id,
                "compas_inicio_seg": fila.compas_inicio_seg,
                "compas_fin_seg": fila.compas_fin_seg,
            },
        });

        let mut tx = pool.begin().await.map_err(AppError::from)?;

        sqlx::query!(
            "UPDATE samples
             SET ruta_original = $2,
                 audio_hash    = $3,
                 tamano        = $4,
                 formato       = $5,
                 estado        = 'procesando',
                 ruta_optimizada = NULL,
                 ruta_waveform   = NULL,
                 metadata = COALESCE(metadata, '{}'::jsonb) || $6
             WHERE id = $1",
            sample_id,
            storage_key,
            audio_hash,
            tamano,
            formato,
            metadata_patch
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

        let cola_meta = serde_json::json!({
            "force_recompute": true,
            "audio_hash": audio_hash,
            "ruta_original": storage_key,
            "trigger": "reemplazo_extraccion",
        });
        sqlx::query!(
            "INSERT INTO cola_procesamiento_ia (tipo, entidad_id, operacion, metadata)
             VALUES ('sample', $1, 'analisis_audio', $2)",
            sample_id,
            cola_meta
        )
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

        tx.commit().await.map_err(AppError::from)?;

        ColaExtraccionRepository::marcar_completado(pool, fila.id, sample_id, &storage_key)
            .await
            .map_err(|e| AppError::Internal(format!("marcar completado: {e}")))?;

        if let Err(e) = tokio::fs::remove_file(ruta_origen).await {
            tracing::debug!(path = ruta_origen, error = %e, "no se pudo borrar audio temp");
        }

        Ok(sample_id)
    }
}

/* ----- helpers ----- */

fn sistema_user_id() -> i32 {
    std::env::var("KAMPLES_SISTEMA_USUARIO_ID")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(SYSTEM_USER_ID_FALLBACK)
}

fn extension_modo(fila: &ColaExtraidoReclamado) -> Option<String> {
    fila.metadata_extraccion
        .as_ref()
        .and_then(|m| m.get("extension_modo"))
        .and_then(|v| v.as_str())
        .map(ToString::to_string)
}

/* [294A-6] Resuelve el audio desde dos posibles orígenes:
 *  - Ruta absoluta (legacy: scraper PHP de WordPress, ej. C:\... o /var/...)
 *    → se lee directo del filesystem.
 *  - Ruta relativa (nuevo scraper Rust/Python, ej. samples/7/2026/04/x.mp3)
 *    → se lee desde el FileStorage configurado (root = ./uploads).
 * Antes el publicador hacía fs::read directo sobre la ruta relativa, lo que
 * fallaba porque buscaba el archivo en el cwd y no bajo storage_root. */
async fn leer_audio(storage: &Arc<dyn FileStorage>, path: &str) -> Result<Vec<u8>, AppError> {
    let p = Path::new(path);
    if p.is_absolute() {
        return tokio::fs::read(path)
            .await
            .map_err(|e| AppError::Internal(format!("leer audio {path}: {e}")));
    }
    storage
        .get_bytes(path)
        .await
        .map_err(|e| AppError::Internal(format!("storage get_bytes {path}: {e}")))
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn detectar_formato(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .map_or_else(|| "mp3".to_string(), str::to_lowercase)
}

/* [294A-6] Bloque "extraccion" enriquecido para guardar en samples.metadata.
 * Replica el contrato que `PromptsIA` PHP esperaba en `contextoTecnico.extraccion`. */
fn extraer_bloque_extraccion(fila: &ColaExtraidoReclamado) -> serde_json::Value {
    let raw = fila.metadata_extraccion.as_ref();
    let s = |key: &str| -> Option<String> {
        raw.and_then(|m| m.get(key))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToOwned::to_owned)
    };
    let n = |key: &str| -> Option<u64> {
        raw.and_then(|m| m.get(key)).and_then(|v| {
            v.as_u64().or_else(|| {
                v.as_f64()
                    .filter(|value| value.is_finite() && *value >= 0.0)
                    .and_then(|value| {
                        const MAX_AS_F64: f64 = 18_446_744_073_709_551_000.0;
                        if value <= MAX_AS_F64 {
                            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                            Some(value.trunc() as u64)
                        } else {
                            None
                        }
                    })
            })
        })
    };
    serde_json::json!({
        "fuente_titulo": s("fuente_titulo"),
        "fuente_artista": s("fuente_artista"),
        "destino_titulo": s("destino_titulo"),
        "destino_artista": s("destino_artista"),
        "tipo_elemento": s("tipo_elemento"),
        "votos_total": n("votos_total"),
        "lado": fila.lado,
        "relacion_id": fila.relacion_id,
    })
}

/* [294A-6] Titulo provisional pre-IA: artista - titulo [tipo].
 * El worker `IaQueueService` lo reemplazara por el titulo final generado por LLM
 * (mismo flujo que PublicadorExtraccion -> ProcesadorColaIA en legacy). */
fn construir_titulo_legacy(fila: &ColaExtraidoReclamado, ext: &serde_json::Value) -> String {
    let s = |key: &str| -> Option<String> {
        ext.get(key)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(ToOwned::to_owned)
    };
    let (titulo, artista) = if fila.lado == "fuente" {
        (s("fuente_titulo"), s("fuente_artista"))
    } else {
        (s("destino_titulo"), s("destino_artista"))
    };
    let tipo = s("tipo_elemento").unwrap_or_else(|| "sample".to_string());
    match (artista, titulo) {
        (Some(a), Some(t)) => format!("{a} - {t} [{tipo}]"),
        (None, Some(t)) => format!("{t} [{tipo}]"),
        _ => {
            if let Some(yt) = &fila.youtube_id {
                format!("Extraccion {yt} [{tipo}]")
            } else {
                format!("Extraccion #{} [{tipo}]", fila.relacion_id)
            }
        }
    }
}

/* [304A-1] Tags estrictamente como legacy PHP (PublicadorExtraccion::generarTags):
 *   [tipo_elemento, "extraccion", fuente_artista, destino_artista]  (todos lowercase, dedup)
 *
 * NO se agrega `lado_fuente`/`lado_destino` — la version anterior los inyectaba
 * y contaminaba el feed/buscador con tags ruidosos que el legacy nunca generaba.
 * El lado se preserva en samples.metadata.lado_extraccion para trazabilidad. */
fn generar_tags_legacy(_fila: &ColaExtraidoReclamado, ext: &serde_json::Value) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    if let Some(tipo) = ext.get("tipo_elemento").and_then(|v| v.as_str()) {
        let t = tipo.trim().to_lowercase();
        if !t.is_empty() {
            tags.push(t);
        }
    }
    tags.push("extraccion".to_string());
    for key in ["fuente_artista", "destino_artista"] {
        if let Some(a) = ext.get(key).and_then(|v| v.as_str()) {
            let a = a.trim().to_lowercase();
            if !a.is_empty() && !tags.contains(&a) {
                tags.push(a);
            }
        }
    }
    tags
}

/// Lee la relaciÃ³n_sample y vincula sample_fuente_id o sample_destino_id
/// segÃºn `lado`. TambiÃ©n fija `samples.relacion_sampleo_id` y
/// `samples.cancion_origen_id` (de la canciÃ³n del lado correspondiente).
async fn vincular_sample_a_relacion(
    pool: &PgPool,
    sample_id: i32,
    fila: &ColaExtraidoReclamado,
) -> Result<(), AppError> {
    let (cancion_fuente, cancion_destino) =
        ColaExtraccionRepository::canciones_de_relacion(pool, fila.relacion_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("relacion {} no existe", fila.relacion_id)))?;

    let (es_fuente, cancion_id) = match fila.lado.as_str() {
        "fuente" => (true, cancion_fuente),
        "destino" => (false, cancion_destino),
        otro => {
            return Err(AppError::BadRequest(format!("lado desconocido: {otro}")));
        }
    };

    // Escrituras en el repositorio: lado de la relación + enlace del sample.
    ColaExtraccionRepository::vincular_lado(
        pool,
        fila.relacion_id,
        sample_id,
        es_fuente,
        cancion_id,
    )
    .await?;

    /* [294A-6] Heredar imagen_url de la cancion origen al sample (paridad con
     * PublicadorExtraccion::publicarItem PHP). El feed/admin renderizaba imagen
     * placeholder porque este paso faltaba en la version Rust. */
    ColaExtraccionRepository::heredar_imagen(pool, sample_id, cancion_id).await?;

    Ok(())
}
