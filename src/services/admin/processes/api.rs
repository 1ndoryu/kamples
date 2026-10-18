use std::collections::BTreeMap;
use std::fs;

use chrono::Utc;
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::errors::AppError;
use crate::models::{AdminProcessCookieInfo, AdminProcessState, AdminProcessesResponse};
use crate::repositories::{AdminAutomationRepository, AutomationBatchUpdate};

use super::{AdminProcessService, ProcessKind, ProcessLock, COOKIE_TYPES};

impl AdminProcessService {
    /* [254A-1] Paridad con GestorProcesosFondo legacy: el panel admin debe ver
     * siempre scraping, extraccion y seed, aunque no existan locks todavía.
     * Gotcha: start/stop usa std::process::Command con args separados para no
     * reintroducir shell escaping manual. Pendiente: portar el seed PHP real. */
    pub fn list() -> Result<AdminProcessesResponse, AppError> {
        let procesos = ProcessKind::all()
            .into_iter()
            .map(Self::state_for_kind)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(AdminProcessesResponse {
            ok: true,
            procesos,
            cookies: Self::all_cookie_info(),
        })
    }

    pub fn state(name: &str) -> Result<AdminProcessState, AppError> {
        Self::state_for_kind(ProcessKind::from_name(name)?)
    }

    pub async fn start(name: &str, limit: Option<u32>, pool: &PgPool) -> Result<Value, AppError> {
        Self::start_with_metadata(name, limit, pool, Some(json!({ "origen": "manual" }))).await
    }

    pub async fn start_automated(
        name: &str,
        limit: Option<u32>,
        pool: &PgPool,
    ) -> Result<Value, AppError> {
        Self::start_with_metadata(name, limit, pool, Some(json!({ "origen": "automatico" }))).await
    }

    async fn start_with_metadata(
        name: &str,
        limit: Option<u32>,
        pool: &PgPool,
        batch_metadata: Option<Value>,
    ) -> Result<Value, AppError> {
        let kind = ProcessKind::from_name(name)?;
        let current = Self::state_for_kind(kind)?;
        if current.estado == "running" {
            return Err(AppError::Conflict(format!(
                "El proceso '{name}' ya esta corriendo (PID: {}).",
                current.pid.unwrap_or_default()
            )));
        }

        match kind {
            ProcessKind::Seed => Self::run_seed(kind, pool).await,
            ProcessKind::Scraping | ProcessKind::Extraction => {
                let batch =
                    AdminAutomationRepository::create_batch(pool, kind.name(), batch_metadata)
                        .await
                        .map_err(|error| {
                            AppError::Internal(format!(
                                "No se pudo crear lote de automatizacion '{}': {error}",
                                kind.name()
                            ))
                        })?;

                match Self::spawn_python(kind, limit, Some(i64::from(batch.id))) {
                    Ok(mut response) => {
                        if let Some(map) = response.as_object_mut() {
                            map.insert("batch_id".to_string(), json!(batch.id));
                        }
                        Ok(response)
                    }
                    Err(error) => {
                        let update = AutomationBatchUpdate {
                            estado: "error".to_string(),
                            exitosos: 0,
                            fallidos: 0,
                            recortes: 0,
                            samples_publicados: 0,
                            canciones_nuevas: 0,
                            sampleos_nuevos: 0,
                            error_mensaje: Some(error.to_string()),
                            metadata: batch.metadata.clone(),
                        };
                        let _ = AdminAutomationRepository::complete_batch(
                            pool,
                            i64::from(batch.id),
                            &update,
                        )
                        .await;
                        Err(error)
                    }
                }
            }
        }
    }

    pub fn stop(name: &str) -> Result<Value, AppError> {
        let kind = ProcessKind::from_name(name)?;
        let state = Self::state_for_kind(kind)?;
        let Some(pid) = state.pid else {
            return Ok(
                json!({ "ok": true, "mensaje": format!("El proceso '{name}' no esta corriendo.") }),
            );
        };

        Self::kill_process_tree(pid)?;
        Self::write_lock(&ProcessLock {
            nombre: name.to_string(),
            estado: "stopped".to_string(),
            pid: None,
            iniciado_at: state.iniciado_at,
            ultimo_log: Some(Utc::now()),
            progreso: state.progreso,
            error: None,
            resultado: state.resultado,
        })?;

        Ok(json!({ "ok": true, "mensaje": format!("Proceso '{name}' detenido (PID: {pid}).") }))
    }

    pub fn save_cookies(cookie_type: &str, content: &str) -> Result<Value, AppError> {
        Self::validate_cookie_type(cookie_type)?;
        let scraper_dir = Self::scraper_dir();
        fs::create_dir_all(&scraper_dir).map_err(|error| {
            AppError::Internal(format!("No se pudo crear directorio scraper: {error}"))
        })?;

        let cookie_path = scraper_dir.join(Self::cookie_filename(cookie_type)?);
        let backup = if cookie_path.exists() {
            let backup_path = cookie_path
                .with_extension(format!("txt.bak.{}", Utc::now().format("%Y%m%d_%H%M%S")));
            fs::copy(&cookie_path, &backup_path).map_err(|error| {
                AppError::Internal(format!("No se pudo crear backup de cookies: {error}"))
            })?;
            backup_path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
        } else {
            None
        };

        let final_content = if content.contains("# Netscape HTTP Cookie File") {
            content.to_string()
        } else {
            format!("# Netscape HTTP Cookie File\n# https://curl.haxx.se/rfc/cookie_spec.html\n\n{content}")
        };

        fs::write(&cookie_path, final_content)
            .map_err(|error| AppError::Internal(format!("No se pudo escribir cookies: {error}")))?;

        Ok(json!({
            "ok": true,
            "mensaje": format!("Cookies {cookie_type} actualizadas correctamente."),
            "backup": backup,
        }))
    }

    pub fn all_cookie_info() -> BTreeMap<String, AdminProcessCookieInfo> {
        COOKIE_TYPES
            .into_iter()
            .map(|cookie_type| (cookie_type.to_string(), Self::cookie_info(cookie_type)))
            .collect()
    }

    fn state_for_kind(kind: ProcessKind) -> Result<AdminProcessState, AppError> {
        let name = kind.name();
        let mut lock = Self::read_lock(name)?.unwrap_or_else(|| ProcessLock::stopped(name));
        let log_tail = Self::read_log_tail(name, 30).unwrap_or_default();

        if lock.estado == "running" {
            if let Some(pid) = lock.pid {
                if !Self::pid_alive(pid) {
                    let inferred_error = Self::infer_error_from_tail(&log_tail);
                    lock.estado = if inferred_error.is_some() {
                        "error".to_string()
                    } else {
                        "stopped".to_string()
                    };
                    lock.pid = None;
                    lock.ultimo_log = Some(Utc::now());
                    lock.error = inferred_error.or(lock.error);
                    Self::write_lock(&lock)?;
                }
            }
        }

        Ok(AdminProcessState {
            nombre: lock.nombre,
            estado: lock.estado,
            pid: lock.pid,
            iniciado_at: lock.iniciado_at,
            ultimo_log: lock.ultimo_log,
            log_tail,
            progreso: lock.progreso,
            error: lock.error,
            resultado: lock.resultado,
        })
    }
}
