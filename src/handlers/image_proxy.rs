/* [104A-5] Proxy de optimización de imágenes on-demand.
 * Ruta: GET /api/img/{*path}?w={ancho}&q={calidad}&fmt={formato}
 * Procesa imágenes locales de uploads/ y assets/ al vuelo con cache en disco.
 * Headers de cache agresivos (1 año) porque la URL incluye los params.
 *
 * [306A-1] Fallback SVG para legacy-assets/colors/: cuando la imagen no existe
 * en disco (ej. en producción donde el dir está gitignored), genera un SVG
 * placeholder con gradiente determinista basado en el nombre del archivo.
 * Mismo algoritmo que frontend/vite-plugins/colors-placeholder.ts. */

use axum::extract::State;
use axum::extract::{Path, Query};
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use serde::Deserialize;
use std::path::PathBuf;

use crate::errors::AppError;
use crate::services::image_processing::{self, OptimizeParams, OutputFormat};
use crate::AppState;

/* Límites de seguridad para evitar abuso de recursos */
const MAX_WIDTH: u32 = 2400;
const MIN_WIDTH: u32 = 16;
const MIN_QUALITY: u8 = 10;
const MAX_QUALITY: u8 = 100;

/* Anchos permitidos (whitelist) para evitar cache flooding.
 * Solo se permiten estos valores exactos o ninguno (original). */
const ALLOWED_WIDTHS: &[u32] = &[150, 300, 480, 640, 800, 1024, 1200, 1600, 2400];

/* [306A-1] Prefijo que activa fallback SVG cuando la imagen no existe en disco. */
const COLORS_PLACEHOLDER_PREFIX: &str = "legacy-assets/colors/";

/// Genera un SVG placeholder con gradiente determinista a partir del nombre de archivo.
/// Mismo algoritmo que frontend/vite-plugins/colors-placeholder.ts.
fn generar_svg_placeholder(nombre: &str) -> String {
    /* Hash determinista del nombre */
    let hash = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        nombre.hash(&mut hasher);
        let h = hasher.finish();
        format!("{h:016x}")
    };

    let hue1 = u32::from_str_radix(&hash[..6], 16).unwrap_or(0) % 360;
    let hue2 = (hue1 + 180) % 360;
    let hue3 = u32::from_str_radix(&hash[4..10], 16).unwrap_or(0) % 360;

    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="400" viewBox="0 0 400 400">
  <defs>
    <linearGradient id="g" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="hsl({hue1}, 55%, 35%)"/>
      <stop offset="50%" stop-color="hsl({hue2}, 50%, 30%)"/>
      <stop offset="100%" stop-color="hsl({hue3}, 45%, 40%)"/>
    </linearGradient>
  </defs>
  <rect width="400" height="400" fill="url(#g)"/>
</svg>"#
    )
}

#[derive(Debug, Deserialize)]
pub struct ImageQueryParams {
    /* Ancho objetivo en píxeles. Debe ser uno de los valores permitidos. */
    pub w: Option<u32>,
    /* Calidad 10-100 (default 80) */
    pub q: Option<u8>,
    /* Formato: webp, jpeg, png (default: mismo que original) */
    pub fmt: Option<String>,
}

/// Proxy de optimización de imágenes
#[utoipa::path(
    get,
    path = "/api/img/{path}",
    params(
        ("path" = String, Path, description = "Ruta relativa de la imagen en uploads/"),
        ("w" = Option<u32>, Query, description = "Ancho objetivo (150,300,480,640,800,1024,1200,1600,2400)"),
        ("q" = Option<u8>, Query, description = "Calidad 10-100 (default 80)"),
        ("fmt" = Option<String>, Query, description = "Formato: webp, jpeg, png"),
    ),
    responses(
        (status = 200, description = "Imagen optimizada"),
        (status = 400, description = "Parámetros inválidos"),
        (status = 404, description = "Imagen no encontrada"),
    ),
    tag = "images"
)]
pub async fn image_proxy(
    State(state): State<AppState>,
    Path(path): Path<String>,
    Query(params): Query<ImageQueryParams>,
) -> Result<impl IntoResponse, AppError> {
    /* Validar que la ruta no intente path traversal */
    if path.contains("..") || path.starts_with('/') || path.starts_with('\\') {
        return Err(AppError::BadRequest("Ruta inválida".into()));
    }

    /* Validar y normalizar ancho */
    let width = if let Some(w) = params.w {
        if !(MIN_WIDTH..=MAX_WIDTH).contains(&w) {
            return Err(AppError::BadRequest(format!(
                "Ancho debe estar entre {MIN_WIDTH} y {MAX_WIDTH}"
            )));
        }
        /* Snapear al ancho permitido más cercano */
        let snapped = ALLOWED_WIDTHS
            .iter()
            .min_by_key(|&&allowed| (i64::from(allowed) - i64::from(w)).unsigned_abs())
            .copied()
            .unwrap_or(w);
        Some(snapped)
    } else {
        None
    };

    /* Validar calidad */
    let quality = params.q.map_or(80, |q| q.clamp(MIN_QUALITY, MAX_QUALITY));

    /* Validar formato */
    let format = OutputFormat::from_str_opt(params.fmt.as_deref());

    let optimize_params = OptimizeParams {
        width,
        quality,
        format,
    };

    /* Resolver la raíz local permitida según el namespace solicitado */
    let (source_root, original_path) = resolve_source_path(&state, &path);

    /* Verificar que el archivo existe y está dentro de la raíz permitida */
    let canonical = match original_path.canonicalize() {
        Ok(c) => c,
        Err(_) if path.starts_with(COLORS_PLACEHOLDER_PREFIX) => {
            /* [306A-1] Archivo no existe: generar SVG placeholder para legacy-assets/colors/ */
            let nombre = &path[COLORS_PLACEHOLDER_PREFIX.len()..];
            let svg = generar_svg_placeholder(nombre);
            return Ok((
                [
                    (header::CONTENT_TYPE, "image/svg+xml".to_string()),
                    (
                        header::CACHE_CONTROL,
                        "public, max-age=86400".to_string(),
                    ),
                ],
                svg.into_bytes(),
            ));
        }
        Err(e) => {
            tracing::warn!(
                path = %path,
                resolved = %original_path.display(),
                error = %e,
                "Imagen no encontrada en disco"
            );
            return Err(AppError::NotFound("Imagen no encontrada".into()));
        }
    };

    let source_root = source_root
        .canonicalize()
        .map_err(|_| AppError::Internal("Directorio de imágenes no encontrado".into()))?;

    if !canonical.starts_with(&source_root) {
        return Err(AppError::BadRequest(
            "Ruta fuera de directorio permitido".into(),
        ));
    }

    /* Verificar que es un formato de imagen soportado */
    let ext = original_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    if !["jpg", "jpeg", "png", "webp", "gif"].contains(&ext) {
        return Err(AppError::BadRequest(
            "Formato de archivo no soportado para optimización".into(),
        ));
    }

    /* Si no hay transformación, servir el original con cache headers */
    if width.is_none() && quality == 80 && matches!(format, OutputFormat::Original) {
        let bytes = tokio::fs::read(&original_path)
            .await
            .map_err(|_| AppError::NotFound("Imagen no encontrada".into()))?;

        let content_type = image_processing::mime_from_extension(Some(ext));
        return Ok((
            [
                (header::CONTENT_TYPE, content_type.to_string()),
                (
                    header::CACHE_CONTROL,
                    "public, max-age=31536000, immutable".to_string(),
                ),
            ],
            bytes,
        ));
    }

    /* Procesar con cache */
    let (bytes, content_type) =
        image_processing::get_optimized_image(&original_path, &optimize_params).await?;

    Ok((
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (
                header::CACHE_CONTROL,
                "public, max-age=31536000, immutable".to_string(),
            ),
        ],
        bytes,
    ))
}

fn resolve_source_path(state: &AppState, path: &str) -> (PathBuf, PathBuf) {
    /* assets/ y legacy-assets/ se sirven desde el directorio estático (frontend/public) */
    if path.starts_with("assets/") || path.starts_with("legacy-assets/") {
        let root = state
            .static_dir
            .as_deref()
            .map_or_else(|| PathBuf::from("frontend/public"), PathBuf::from);
        return (root.clone(), root.join(path)); // sentinel-disable path-join-sin-canonicalize — el llamador rechaza .. y / inicial y comprueba starts_with tras canonicalize
    }

    /* El resto (uploads/) se sirve desde el directorio de uploads */
    let root = PathBuf::from("uploads");
    (root.clone(), root.join(path)) // sentinel-disable path-join-sin-canonicalize — el llamador rechaza .. y / inicial y comprueba starts_with tras canonicalize
}

pub fn routes() -> Router<AppState> {
    /* [166A-5] Sin prefijo /api porque api_routes() ya está anidada bajo .nest("/api", ...) */
    Router::new().route("/img/*path", get(image_proxy))
}
