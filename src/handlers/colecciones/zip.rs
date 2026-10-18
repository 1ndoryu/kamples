use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::Json;

use crate::algorithm::InteractionKind;
use crate::domain::{calculate_subscription_download_revenue_share, KamplesPlanId};
use crate::errors::AppError;
use crate::handlers::free_codes::normalize_optional_free_code;
use crate::middleware::CurrentUser;
use crate::models::DownloadGrantRequest;
use crate::repositories::{
    BillingRepository, ColeccionSampleFile, ColeccionesRepository,
    CompletedDownloadRevenueShareInsert, DownloadRepository, FreeCodeRepository,
};
use crate::services::FileStorage;
use crate::AppState;

/* [174A-62] ZIP de colección — port mínimo de DescargasZipController.php.
 *
 * Endpoint:
 * - POST /api/colecciones/:id/descargar-zip
 *
 * Reglas portadas:
 * - Owner o colección pública.
 * - Máximo MAX_SAMPLES_ZIP samples por descarga (anti-DoS).
 * - Máximo MAX_ZIP_BYTES total (corta el ZIP cuando supera, no falla).
 * - Plan free: límite base 5/día + `creditos_bonus` aplica solo a samples
 *   NUEVOS (los ya descargados no consumen). Si requiere más créditos de los
 *   disponibles → 429.
 * - Plan free: si la colección contiene samples premium (nuevos) → 403, salvo
 *   que `codigoGratis` válido habilite la colección.
 * - Cada sample nuevo: registra DESCARGA + incrementa total_descargas.
 * - Trigger AlgoPlanner Descarga al final.
 * - Nombres de archivo en el ZIP saneados + des-duplicados.
 *
 * NO portado:
 * - Cache a disco / lock por colección. Generamos en memoria cada vez (la
 *   abstracción FileStorage solo expone get_bytes total → ya implica cargar
 *   todo a memoria; cachear redundante por ahora).
 * - Calidad por plan: siempre "wav" como en el resto del port (ver downloads.rs).
 *
 * Respuesta: streaming `application/zip` con filename derivado del nombre.
 */

const MAX_SAMPLES_ZIP: usize = 500;
const MAX_ZIP_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const PLAN_FREE_LIMIT_PER_DAY: i64 = 5;

#[utoipa::path(
    post, path = "/api/colecciones/{id}/descargar-zip", tag = "colecciones",
    params(("id" = i64, Path, description = "ID de la coleccion a descargar como ZIP")),
    request_body = Option<DownloadGrantRequest>,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Stream binario application/zip"),
        (status = 400, description = "Coleccion vacia o demasiado grande"),
        (status = 403, description = "Sin acceso o samples premium en plan free"),
        (status = 404, description = "Coleccion no encontrada"),
        (status = 429, description = "Creditos insuficientes para los samples nuevos"),
    )
)]
#[allow(clippy::too_many_lines)]
pub async fn descargar_zip_coleccion(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    payload: Option<Json<DownloadGrantRequest>>,
) -> Result<axum::response::Response, AppError> {
    let access_request = payload.map(|Json(value)| value).unwrap_or_default();
    let col = ColeccionesRepository::fetch(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("coleccion {id} no existe")))?;
    let es_propietario = col.usuario_id == user.user_id;
    if !es_propietario && !col.publica {
        return Err(AppError::Forbidden("coleccion privada".into()));
    }

    let samples = ColeccionesRepository::list_samples_for_zip(&state.pool, id).await?;
    if samples.is_empty() {
        return Err(AppError::BadRequest("la coleccion no tiene samples".into()));
    }
    if samples.len() > MAX_SAMPLES_ZIP {
        return Err(AppError::BadRequest(format!(
            "la coleccion tiene demasiados samples ({}). Maximo {} por ZIP",
            samples.len(),
            MAX_SAMPLES_ZIP
        )));
    }

    let mut nuevos: Vec<&ColeccionSampleFile> = Vec::new();
    for sample in &samples {
        let already =
            DownloadRepository::already_downloaded(&state.pool, user.user_id, sample.sample_id)
                .await?;
        if !already {
            nuevos.push(sample);
        }
    }
    let creditos_necesarios = i64::try_from(nuevos.len()).unwrap_or(i64::MAX);

    let allowance = DownloadRepository::user_download_allowance(&state.pool, user.user_id).await?;
    let bonus_credits = allowance.bonus_credits;
    let plan = allowance.plan;
    let codigo_gratis = normalize_optional_free_code(access_request.codigo_gratis)?;
    let es_codigo_gratis = if let Some(code) = codigo_gratis.as_deref() {
        !es_propietario
            && FreeCodeRepository::can_user_download(
                &state.pool,
                code,
                "coleccion",
                id,
                user.user_id,
            )
            .await?
    } else {
        false
    };

    if plan == "free" && !es_codigo_gratis && creditos_necesarios > 0 {
        let usadas = DownloadRepository::count_today(&state.pool, user.user_id).await?;
        let disponibles = (effective_free_daily_limit(bonus_credits) - usadas).max(0);
        if creditos_necesarios > disponibles {
            return Err(AppError::TooManyRequests(format!(
                "creditos insuficientes: necesitas {creditos_necesarios} pero solo tienes {disponibles} disponibles hoy"
            )));
        }
        if nuevos.iter().any(|sample| sample.es_premium) {
            return Err(AppError::Forbidden(
                "la coleccion contiene samples premium nuevos: requiere plan pro o premium".into(),
            ));
        }
    }

    let zip_bytes = build_zip_in_memory(samples.clone(), state.storage.clone()).await?;

    for sample in &nuevos {
        let _ =
            DownloadRepository::register(&state.pool, user.user_id, sample.sample_id, "wav").await;
        if plan != "free" && sample.creador_id != user.user_id {
            let revenue = calculate_subscription_download_revenue_share(
                plan.parse::<KamplesPlanId>().unwrap_or(KamplesPlanId::Free),
            );
            if revenue.price_cents > 0 {
                let insert = CompletedDownloadRevenueShareInsert {
                    buyer_id: user.user_id,
                    creator_id: sample.creador_id,
                    sample_id: sample.sample_id,
                    amount_cents: revenue.price_cents,
                    creator_amount_cents: revenue.creator_payout_cents,
                    platform_fee_cents: revenue.platform_fee_cents,
                };
                if let Err(error) =
                    BillingRepository::insert_completed_download_revenue_share(&state.pool, &insert)
                        .await
                {
                    tracing::error!(
                        buyer_id = user.user_id,
                        creator_id = sample.creador_id,
                        sample_id = sample.sample_id,
                        plan = %plan,
                        error = %error,
                        "revenue share fallido para ZIP de coleccion"
                    );
                }
            }
        }
    }

    let _ = state
        .algo_planner
        .register_interaction(
            &state.pool,
            &state.redis,
            user.user_id,
            InteractionKind::Descarga,
        )
        .await;

    let filename = format!("{}.zip", sanitize_filename(&col.nombre));
    let mut response = (StatusCode::OK, zip_bytes).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/zip"),
    );
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    Ok(response)
}

/* Construye un ZIP DEFLATE en memoria. Trunca cuando supera MAX_ZIP_BYTES.
 * El crate `zip` es síncrono → usamos spawn_blocking. */
async fn build_zip_in_memory(
    samples: Vec<ColeccionSampleFile>,
    storage: Arc<dyn FileStorage>,
) -> Result<Vec<u8>, AppError> {
    let mut payloads: Vec<(String, Vec<u8>)> = Vec::with_capacity(samples.len());
    let mut acumulado: u64 = 0;
    let mut nombres_usados: HashSet<String> = HashSet::new();

    for sample in samples {
        let Ok(bytes) = storage.get_bytes(&sample.storage_key).await else {
            continue;
        };
        let tam = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if acumulado.saturating_add(tam) > MAX_ZIP_BYTES {
            tracing::warn!(coleccion_size = %acumulado, limit = %MAX_ZIP_BYTES, "ZIP truncado por tamano");
            break;
        }
        acumulado += tam;
        let nombre =
            unique_zip_entry_name(&sample.titulo, &sample.storage_key, &mut nombres_usados);
        payloads.push((nombre, bytes));
    }

    if payloads.is_empty() {
        return Err(AppError::Internal(
            "no se pudo recuperar ningun sample del storage".into(),
        ));
    }

    tokio::task::spawn_blocking(move || -> Result<Vec<u8>, AppError> {
        use std::io::Write;

        let mut cursor = std::io::Cursor::new(Vec::<u8>::new());
        {
            let mut writer = zip::ZipWriter::new(&mut cursor);
            let options: zip::write::FileOptions<'_, '_, ()> = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .large_file(false);
            for (name, bytes) in payloads {
                writer
                    .start_file(&name, options)
                    .map_err(|e| AppError::Internal(format!("zip start_file: {e}")))?;
                writer
                    .write_all(&bytes)
                    .map_err(|e| AppError::Internal(format!("zip write_all: {e}")))?;
            }
            writer
                .finish()
                .map_err(|e| AppError::Internal(format!("zip finish: {e}")))?;
        }
        Ok(cursor.into_inner())
    })
    .await
    .map_err(|e| AppError::Internal(format!("zip task join: {e}")))?
}

fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() {
        "coleccion".to_string()
    } else {
        trimmed.chars().take(100).collect()
    }
}

fn unique_zip_entry_name(titulo: &str, storage_key: &str, used: &mut HashSet<String>) -> String {
    let ext = std::path::Path::new(storage_key)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("bin");
    let base = sanitize_filename(titulo);
    let mut candidato = format!("{base}.{ext}");
    let mut suffix: u32 = 1;
    while used.contains(&candidato) {
        candidato = format!("{base}_{suffix}.{ext}");
        suffix += 1;
    }
    used.insert(candidato.clone());
    candidato
}

fn effective_free_daily_limit(bonus_credits: i32) -> i64 {
    PLAN_FREE_LIMIT_PER_DAY.saturating_add(i64::from(bonus_credits.max(0)))
}

#[cfg(test)]
mod tests {
    use super::{build_zip_in_memory, sanitize_filename};
    use crate::repositories::ColeccionSampleFile;
    use crate::services::{FileStorage, LocalFs};
    use std::io::Read;
    use std::sync::Arc;

    #[tokio::test]
    async fn build_zip_dedupes_names_and_preserves_payloads() {
        let root =
            std::env::temp_dir().join(format!("glory-kamples-zip-test-{}", uuid::Uuid::new_v4()));
        let storage = LocalFs::new(&root).await.expect("storage");
        storage
            .put_bytes("samples/a.wav", b"uno")
            .await
            .expect("put a");
        storage
            .put_bytes("samples/b.wav", b"dos")
            .await
            .expect("put b");

        let zip_bytes = build_zip_in_memory(
            vec![
                ColeccionSampleFile {
                    sample_id: 1,
                    titulo: "Mi sample".into(),
                    storage_key: "samples/a.wav".into(),
                    es_premium: false,
                    creador_id: 1,
                },
                ColeccionSampleFile {
                    sample_id: 2,
                    titulo: "Mi sample".into(),
                    storage_key: "samples/b.wav".into(),
                    es_premium: false,
                    creador_id: 1,
                },
            ],
            Arc::new(storage),
        )
        .await
        .expect("zip");

        let reader = std::io::Cursor::new(zip_bytes);
        let mut archive = zip::ZipArchive::new(reader).expect("archive");
        assert_eq!(archive.len(), 2);

        let mut first = archive.by_name("Mi_sample.wav").expect("first");
        let mut first_buf = String::new();
        first.read_to_string(&mut first_buf).expect("read first");
        assert_eq!(first_buf, "uno");
        drop(first);

        let mut second = archive.by_name("Mi_sample_1.wav").expect("second");
        let mut second_buf = String::new();
        second.read_to_string(&mut second_buf).expect("read second");
        assert_eq!(second_buf, "dos");

        tokio::fs::remove_dir_all(&root).await.expect("cleanup");
    }

    #[test]
    fn sanitize_filename_normalizes_invalid_chars() {
        assert_eq!(sanitize_filename("  Col/lec ción?#  "), "Col_lec_ci_n");
        assert_eq!(sanitize_filename("***"), "coleccion");
    }
}
