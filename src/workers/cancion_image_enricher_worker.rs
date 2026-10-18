/* [304A-1] Worker lento de enriquecimiento de portadas de canciones.
 * [166A-5] Ahora descarga la imagen de iTunes localmente y la optimiza
 * (max 600px, WebP quality 80) en vez de solo guardar la URL remota.
 *
 * Problema: ~2.7k canciones tienen `imagen_url` NULL porque el scraper de
 * WhoSampled no siempre extrae la portada. Sin imagen en la cancion origen,
 * el sample extraido tampoco hereda imagen → feed/admin renderiza placeholder.
 *
 * Estrategia (lento e idempotente, sin lotes grandes ni rate-limit storms):
 *   - Cada `INTERVAL` segundos, toma UNA cancion sin imagen.
 *   - Consulta iTunes Search API (gratis, sin API key, ~20 req/min) por
 *     "<artista> <titulo>" y extrae `artworkUrl100` → upgrade a 600x600.
 *   - Si encuentra coincidencia, descarga la imagen, la convierte a WebP
 *     (max 600px, quality 80), la guarda en uploads/kamples/itunes/ y
 *     UPDATE imagen_url con la ruta local.
 *   - Si no, marca metadata.imagen_lookup_at = now() para no reintentar pronto
 *     (cooldown de 30 dias antes del proximo intento).
 *
 * Por que iTunes:
 *   - Devuelve artwork de alta calidad (cambiar `100x100bb.jpg` → `600x600bb.jpg`).
 *   - Sin API key, sin OAuth, sin rate limit estricto documentado.
 *   - Cobertura amplia para musica comercial (mainstream + catalogo).
 *
 * NO bloquea: errores se loguean, la cancion se reintenta tras cooldown.
 * NO satura: 1 cancion cada 30s = ~2880/dia, suficiente para drenar 2.7k en ~24h.
 */

use serde::Deserialize;
use sqlx::PgPool;
use std::path::PathBuf;
use tokio::fs;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};

use crate::errors::AppError;
use crate::services::image_processing::{self, OptimizeParams, OutputFormat};

const TICK_INTERVAL: Duration = Duration::from_secs(30);
const ERROR_INTERVAL: Duration = Duration::from_mins(2);
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);
const COOLDOWN_DAYS: i32 = 30;
const ITUNES_IMAGE_DIR: &str = "kamples/itunes";
const MAX_IMAGE_WIDTH: u32 = 600;
const IMAGE_QUALITY: u8 = 80;

pub fn spawn_cancion_image_enricher_worker(pool: &PgPool, storage_root: &str) -> JoinHandle<()> {
    let pool = pool.clone();
    let storage_root = storage_root.to_string();
    tokio::spawn(async move {
        run_forever(pool, storage_root).await;
    })
}

async fn run_forever(pool: PgPool, storage_root: String) {
    tracing::info!(
        "cancion image enricher worker iniciado (intervalo: {:?})",
        TICK_INTERVAL
    );

    let client = match reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .user_agent("glory-kamples-image-enricher/1.0")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(%e, "no se pudo construir reqwest client; worker abortado");
            return;
        }
    };

    loop {
        match procesar_una(&pool, &client, &storage_root).await {
            Ok(true) => sleep(TICK_INTERVAL).await,
            Ok(false) => {
                /* Sin candidatos: dormir mas para no martillar la BD. */
                sleep(Duration::from_mins(10)).await;
            }
            Err(e) => {
                tracing::error!(%e, "error en cancion image enricher tick");
                sleep(ERROR_INTERVAL).await;
            }
        }
    }
}

async fn procesar_una(
    pool: &PgPool,
    client: &reqwest::Client,
    storage_root: &str,
) -> Result<bool, sqlx::Error> {
    /* Selecciona una cancion sin imagen y sin lookup reciente.
     * Prioriza canciones con `total_sampleada > 0` (las que aparecen en relaciones)
     * para que el feed/extracciones se beneficien primero. */
    // Alias "!" fuerza no nulo: el JOIN y el titulo siempre traen valor.
    let row = sqlx::query!(
        r#"
        SELECT c.id AS "id!", c.titulo AS "titulo!", a.nombre AS "nombre!"
          FROM canciones c
          INNER JOIN artistas_musicales a ON a.id = c.artista_id
         WHERE (c.imagen_url IS NULL OR TRIM(c.imagen_url) = '')
           AND COALESCE(
                 (c.metadata->>'imagen_lookup_at')::timestamptz,
                 'epoch'::timestamptz
               ) < NOW() - make_interval(days => $1)
         ORDER BY c.total_sampleada DESC NULLS LAST, c.id ASC
         LIMIT 1
        "#,
        COOLDOWN_DAYS
    )
    .fetch_optional(pool)
    .await?;

    let Some(fila) = row else {
        return Ok(false);
    };
    let (cancion_id, titulo, artista) = (fila.id, fila.titulo, fila.nombre);

    let query = format!("{} {}", artista.trim(), titulo.trim());
    let trimmed = query.trim();
    if trimmed.is_empty() {
        marcar_lookup(pool, cancion_id, "query_vacia").await?;
        return Ok(true);
    }

    match buscar_artwork_itunes(client, trimmed).await {
        Ok(Some(url)) => {
            /* Descargar, optimizar y guardar localmente */
            match descargar_y_optimizar(client, &url, storage_root, cancion_id).await {
                Ok(local_path) => {
                    sqlx::query!(
                        r#"
                        UPDATE canciones
                           SET imagen_url = $2,
                               metadata = COALESCE(metadata, '{}'::jsonb)
                                          || jsonb_build_object(
                                               'imagen_fuente', 'itunes',
                                               'imagen_lookup_at', NOW()::text
                                             )
                         WHERE id = $1
                        "#,
                        cancion_id,
                        local_path
                    )
                    .execute(pool)
                    .await?;
                    tracing::info!(
                        cancion_id,
                        local = %local_path,
                        "portada descargada y optimizada (itunes)"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        cancion_id,
                        %e,
                        "fallo descarga/optimizacion de portada itunes"
                    );
                    marcar_lookup(pool, cancion_id, "error_descarga").await?;
                }
            }
        }
        Ok(None) => {
            marcar_lookup(pool, cancion_id, "sin_resultados").await?;
        }
        Err(e) => {
            tracing::warn!(cancion_id, %e, "error consultando iTunes");
            marcar_lookup(pool, cancion_id, "error_http").await?;
        }
    }

    Ok(true)
}

/* Descarga la imagen de la URL remota, la optimiza (resize + WebP) y la guarda
 * en {storage_root}/kamples/itunes/{cancion_id}.webp.
 * Retorna la ruta relativa para almacenar en la BD: /uploads/kamples/itunes/{id}.webp */
async fn descargar_y_optimizar(
    client: &reqwest::Client,
    url: &str,
    storage_root: &str,
    cancion_id: i32,
) -> Result<String, AppError> {
    /* Descargar */
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Error descargando imagen iTunes: {e}")))?;

    if !response.status().is_success() {
        return Err(AppError::Internal(format!(
            "iTunes CDN devolvio HTTP {}",
            response.status()
        )));
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();

    let bytes = response
        .bytes()
        .await
        .map_err(|e| AppError::Internal(format!("Error leyendo bytes de iTunes: {e}")))?
        .to_vec();

    /* Procesar imagen: resize a max 600px, WebP quality 80 */
    let params = OptimizeParams {
        width: Some(MAX_IMAGE_WIDTH),
        quality: IMAGE_QUALITY,
        format: OutputFormat::Webp,
    };

    /* process_image es CPU-bound; ejecutar en spawn_blocking */
    let processed = tokio::task::spawn_blocking(move || {
        image_processing::process_image(&bytes, &content_type, &params)
    })
    .await
    .map_err(|e| AppError::Internal(format!("Error en spawn_blocking: {e}")))?
    .map_err(|e| AppError::Internal(format!("Error procesando imagen: {e}")))?;

    let (webp_bytes, _content_type) = processed;

    /* Guardar a disco */
    let rel_path = format!("{ITUNES_IMAGE_DIR}/{cancion_id}.webp");
    let full_path: PathBuf = [storage_root, &rel_path].iter().collect();

    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::Internal(format!("Error creando directorio: {e}")))?;
    }

    fs::write(&full_path, &webp_bytes)
        .await
        .map_err(|e| AppError::Internal(format!("Error guardando imagen: {e}")))?;

    tracing::info!(
        path = %full_path.display(),
        bytes = webp_bytes.len(),
        "imagen itunes guardada localmente"
    );

    /* Ruta relativa que el frontend resuelve via proxy /api/img/ */
    Ok(format!("/uploads/{rel_path}"))
}

async fn marcar_lookup(pool: &PgPool, cancion_id: i32, motivo: &str) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE canciones
           SET metadata = COALESCE(metadata, '{}'::jsonb)
                          || jsonb_build_object(
                               'imagen_lookup_at', NOW()::text,
                               'imagen_lookup_motivo', $2::text
                             )
         WHERE id = $1
        "#,
        cancion_id,
        motivo
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct ItunesResponse {
    results: Vec<ItunesResult>,
}

#[derive(Debug, Deserialize)]
struct ItunesResult {
    #[serde(rename = "artworkUrl100")]
    artwork_url_100: Option<String>,
}

async fn buscar_artwork_itunes(
    client: &reqwest::Client,
    query: &str,
) -> Result<Option<String>, reqwest::Error> {
    let url = "https://itunes.apple.com/search";
    let resp = client
        .get(url)
        .query(&[
            ("term", query),
            ("media", "music"),
            ("entity", "song"),
            ("limit", "1"),
        ])
        .send()
        .await?
        .error_for_status()?;
    let body: ItunesResponse = resp.json().await?;
    Ok(body
        .results
        .into_iter()
        .find_map(|r| r.artwork_url_100)
        .map(|u| u.replace("100x100bb.jpg", "600x600bb.jpg")))
}
