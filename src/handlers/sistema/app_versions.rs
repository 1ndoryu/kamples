use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use utoipa::ToSchema;

use crate::AppState;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AppVersionInfo {
    pub version: String,
    pub url: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AppVersionsResponse {
    pub windows: Option<AppVersionInfo>,
    pub apk: Option<AppVersionInfo>,
    pub web: Option<AppVersionInfo>,
}

fn first_env(keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| std::env::var(key).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn build_version_info(prefix: &str, fallback_version: Option<String>) -> Option<AppVersionInfo> {
    let upper = prefix.to_ascii_uppercase();
    let version = first_env(&[
        &format!("KAMPLES_{upper}_VERSION"),
        &format!("APP_{upper}_VERSION"),
    ])
    .or(fallback_version);
    let url = first_env(&[&format!("KAMPLES_{upper}_URL"), &format!("APP_{upper}_URL")]);
    let notes = first_env(&[
        &format!("KAMPLES_{upper}_NOTES"),
        &format!("APP_{upper}_NOTES"),
    ]);

    if version.is_none() && url.is_none() && notes.is_none() {
        return None;
    }

    Some(AppVersionInfo {
        version: version.unwrap_or_default(),
        url,
        notes,
    })
}

#[utoipa::path(
    get,
    path = "/api/app/versions",
    tag = "sync",
    responses((status = 200, description = "Versiones disponibles por plataforma", body = AppVersionsResponse))
)]
pub async fn get_app_versions() -> Json<AppVersionsResponse> {
    Json(AppVersionsResponse {
        windows: build_version_info("windows", None),
        apk: build_version_info("apk", None),
        web: build_version_info("web", Some(env!("CARGO_PKG_VERSION").to_string())),
    })
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/app/versions", get(get_app_versions))
        /* [174A-111b] Endpoint del updater Tauri 2.
         * Tauri sustituye {{target}} (windows/macos/linux), {{arch}} (x86_64/aarch64)
         * y {{current_version}} (semver instalado). Debemos responder:
         *   - 200 + manifest JSON si hay update disponible
         *   - 204 No Content si la versión actual ya es la última
         * El manifest se construye desde env vars KAMPLES_DESKTOP_* (ver get_desktop_updater). */
        .route(
            "/app/updater/:target/:arch/:current_version",
            get(get_desktop_updater),
        )
}

/* [174A-111b] Manifest individual del updater Tauri 2 (formato esperado
 * por @tauri-apps/plugin-updater): version, pub_date, url, signature, notes. */
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DesktopUpdaterManifest {
    pub version: String,
    pub notes: String,
    pub pub_date: String,
    pub url: String,
    pub signature: String,
}

#[utoipa::path(
    get,
    path = "/api/app/updater/{target}/{arch}/{current_version}",
    tag = "sync",
    params(
        ("target" = String, Path, description = "Plataforma: windows, darwin, linux"),
        ("arch" = String, Path, description = "Arquitectura: x86_64, aarch64, i686"),
        ("current_version" = String, Path, description = "Versión semver instalada en el cliente")
    ),
    responses(
        (status = 200, description = "Hay update disponible", body = DesktopUpdaterManifest),
        (status = 204, description = "El cliente ya tiene la última versión")
    )
)]
pub async fn get_desktop_updater(
    Path((target, arch, current_version)): Path<(String, String, String)>,
) -> Response {
    /* Sanitizar parámetros (solo alfanumérico + guión + punto) para evitar inyección
     * en construcción de claves de env var y logging. */
    let safe_target: String = target.chars().filter(char::is_ascii_alphanumeric).collect();
    let safe_arch: String = arch
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    let safe_current: String = current_version
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
        .take(32)
        .collect();

    /* Buscar variables específicas por plataforma+arch primero, luego por plataforma sola.
     * Ej: KAMPLES_DESKTOP_WINDOWS_X86_64_VERSION o KAMPLES_DESKTOP_WINDOWS_VERSION. */
    let upper_target = safe_target.to_ascii_uppercase();
    let upper_arch = safe_arch.to_ascii_uppercase();
    let version = first_env(&[
        &format!("KAMPLES_DESKTOP_{upper_target}_{upper_arch}_VERSION"),
        &format!("KAMPLES_DESKTOP_{upper_target}_VERSION"),
        "KAMPLES_DESKTOP_VERSION",
    ]);
    let url = first_env(&[
        &format!("KAMPLES_DESKTOP_{upper_target}_{upper_arch}_URL"),
        &format!("KAMPLES_DESKTOP_{upper_target}_URL"),
        "KAMPLES_DESKTOP_URL",
    ]);
    let signature = first_env(&[
        &format!("KAMPLES_DESKTOP_{upper_target}_{upper_arch}_SIGNATURE"),
        &format!("KAMPLES_DESKTOP_{upper_target}_SIGNATURE"),
        "KAMPLES_DESKTOP_SIGNATURE",
    ]);

    let Some(latest_version) = version else {
        tracing::debug!(
            target = %safe_target,
            arch = %safe_arch,
            current = %safe_current,
            "updater: sin KAMPLES_DESKTOP_*_VERSION configurada → 204"
        );
        return StatusCode::NO_CONTENT.into_response();
    };

    /* Si la versión actual ya iguala o supera la latest, devolver 204
     * (Tauri interpreta 204 como "ya estás al día"). */
    if !is_newer_than_current(&latest_version, &safe_current) {
        return StatusCode::NO_CONTENT.into_response();
    }

    let (Some(url), Some(signature)) = (url, signature) else {
        /* Versión configurada pero falta URL o firma: configuración incompleta.
         * Mejor 204 que romper el cliente. */
        tracing::warn!(
            latest = %latest_version,
            "updater: KAMPLES_DESKTOP_*_VERSION definida pero falta URL o SIGNATURE"
        );
        return StatusCode::NO_CONTENT.into_response();
    };

    let pub_date = first_env(&[
        &format!("KAMPLES_DESKTOP_{upper_target}_PUB_DATE"),
        "KAMPLES_DESKTOP_PUB_DATE",
    ])
    .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

    let notes = first_env(&[
        &format!("KAMPLES_DESKTOP_{upper_target}_NOTES"),
        "KAMPLES_DESKTOP_NOTES",
    ])
    .unwrap_or_default();

    Json(DesktopUpdaterManifest {
        version: latest_version,
        notes,
        pub_date,
        url,
        signature,
    })
    .into_response()
}

/* Comparador semver simplificado: devuelve true si `latest` > `current`.
 * No depende del crate `semver` para mantener el binario más liviano; las
 * versiones del desktop siguen formato MAJOR.MINOR.PATCH (sin pre-release). */
fn is_newer_than_current(latest: &str, current: &str) -> bool {
    fn parse(v: &str) -> (u32, u32, u32) {
        let mut iter = v.split(['.', '-']).filter_map(|p| p.parse::<u32>().ok());
        (
            iter.next().unwrap_or(0),
            iter.next().unwrap_or(0),
            iter.next().unwrap_or(0),
        )
    }
    parse(latest) > parse(current)
}
