/* sentinel-disable-file limite-lineas — gestor central legacy de procesos admin: scraping, extraccion, seed, locks, cookies y entorno Python. 294A-1 extrae helpers de spawn_python; dividir el modulo completo requiere separar contratos/rutas en una tarea arquitectonica propia. */

mod api;
mod spawn;

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{DateTime, Utc};
use serde_json::Value;
use url::Url;

use crate::errors::AppError;
use crate::models::AdminProcessCookieInfo;


const PROCESS_SCRAPING: &str = "scraping";
const PROCESS_EXTRACTION: &str = "extraccion";
const PROCESS_SEED: &str = "seed";
const COOKIE_TYPES: [&str; 2] = ["youtube", "soundcloud"];

#[derive(Debug, Clone, Copy)]
enum ProcessKind {
    Scraping,
    Extraction,
    Seed,
}

impl ProcessKind {
    fn name(self) -> &'static str {
        match self {
            Self::Scraping => PROCESS_SCRAPING,
            Self::Extraction => PROCESS_EXTRACTION,
            Self::Seed => PROCESS_SEED,
        }
    }

    fn all() -> [Self; 3] {
        [Self::Scraping, Self::Extraction, Self::Seed]
    }

    fn from_name(name: &str) -> Result<Self, AppError> {
        match name {
            PROCESS_SCRAPING => Ok(Self::Scraping),
            PROCESS_EXTRACTION => Ok(Self::Extraction),
            PROCESS_SEED => Ok(Self::Seed),
            other => Err(AppError::BadRequest(format!(
                "Proceso desconocido: '{other}'. Validos: scraping, extraccion, seed"
            ))),
        }
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct ProcessLock {
    nombre: String,
    estado: String,
    pid: Option<u32>,
    iniciado_at: Option<DateTime<Utc>>,
    ultimo_log: Option<DateTime<Utc>>,
    progreso: Option<f32>,
    error: Option<String>,
    resultado: Option<BTreeMap<String, Value>>,
}

impl ProcessLock {
    fn stopped(name: &str) -> Self {
        Self {
            nombre: name.to_string(),
            estado: "stopped".to_string(),
            pid: None,
            iniciado_at: None,
            ultimo_log: None,
            progreso: None,
            error: None,
            resultado: None,
        }
    }
}

pub struct AdminProcessService;

impl AdminProcessService {
    fn project_root() -> PathBuf {
        std::env::var("KAMPLES_PROJECT_ROOT").map_or_else(
            |_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            PathBuf::from,
        )
    }

    fn logs_dir() -> PathBuf {
        Self::project_root().join("logs")
    }

    fn scraper_dir() -> PathBuf {
        std::env::var("KAMPLES_SCRAPER_DIR").map_or_else(
            |_| Self::project_root().join("clients").join("kamples-scraper"),
            PathBuf::from,
        )
    }

    fn extractions_dir() -> Result<PathBuf, AppError> {
        let path = std::env::var("KAMPLES_EXTRACTIONS_DIR").map_or_else(
            |_| {
                Self::project_root()
                    .join("uploads")
                    .join("kamples")
                    .join("extracciones")
            },
            PathBuf::from,
        );
        fs::create_dir_all(&path).map_err(|error| {
            AppError::Internal(format!(
                "No se pudo crear directorio de extracciones: {error}"
            ))
        })?;
        Ok(path)
    }

    fn lock_path(name: &str) -> PathBuf {
        Self::logs_dir().join(format!("{name}.lock"))
    }

    fn log_path(name: &str) -> Result<PathBuf, AppError> {
        let logs_dir = Self::logs_dir();
        fs::create_dir_all(&logs_dir).map_err(|error| {
            AppError::Internal(format!("No se pudo crear directorio de logs: {error}"))
        })?;
        Ok(logs_dir.join(format!(
            "{name}-output-{}.log",
            Utc::now().format("%Y-%m-%d")
        )))
    }

    fn read_lock(name: &str) -> Result<Option<ProcessLock>, AppError> {
        let path = Self::lock_path(name);
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)
            .map_err(|error| AppError::Internal(format!("Error leyendo lock {name}: {error}")))?;
        let lock = serde_json::from_str::<ProcessLock>(&content)
            .map_err(|error| AppError::Internal(format!("Lock invalido {name}: {error}")))?;
        Ok(Some(lock))
    }

    fn write_lock(lock: &ProcessLock) -> Result<(), AppError> {
        let logs_dir = Self::logs_dir();
        fs::create_dir_all(&logs_dir).map_err(|error| {
            AppError::Internal(format!("No se pudo crear directorio de logs: {error}"))
        })?;
        let json = serde_json::to_string_pretty(lock)
            .map_err(|error| AppError::Internal(format!("No se pudo serializar lock: {error}")))?;
        fs::write(Self::lock_path(&lock.nombre), json)
            .map_err(|error| AppError::Internal(format!("No se pudo escribir lock: {error}")))
    }

    fn append_log_header(path: &Path, name: &str) -> Result<(), AppError> {
        let mut file = Self::open_log_append(path)?;
        writeln!(
            file,
            "\n------------------------------------------------------------"
        )
        .map_err(|error| AppError::Internal(format!("No se pudo escribir log: {error}")))?;
        writeln!(
            file,
            "[{}] INICIO proceso={name}",
            Utc::now().format("%Y-%m-%d %H:%M:%S")
        )
        .map_err(|error| AppError::Internal(format!("No se pudo escribir log: {error}")))?;
        writeln!(
            file,
            "------------------------------------------------------------"
        )
        .map_err(|error| AppError::Internal(format!("No se pudo escribir log: {error}")))
    }

    fn open_log_append(path: &Path) -> Result<File, AppError> {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|error| AppError::Internal(format!("No se pudo abrir log: {error}")))
    }

    fn read_log_tail(name: &str, lines: usize) -> Result<String, AppError> {
        /* [294A-4] Buscar el archivo de log mas reciente que matchee
         * `{name}-output-*.log` en lugar de depender del UTC date actual.
         * Esto evita que el tail aparezca vacio cuando el archivo fue creado
         * en otra fecha (ej: TZ shift, proceso largo cruzando medianoche, o
         * date mismatch entre Utc::now() y la fecha local del filesystem). */
        let logs_dir = Self::logs_dir();
        if !logs_dir.exists() {
            return Ok(String::new());
        }
        let prefix = format!("{name}-output-");
        let Ok(entries) = fs::read_dir(&logs_dir) else {
            return Ok(String::new());
        };
        let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !file_name.starts_with(&prefix)
                || !Path::new(file_name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("log"))
            {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let Ok(modified) = meta.modified() else {
                continue;
            };
            if best.as_ref().is_none_or(|(t, _)| modified > *t) {
                best = Some((modified, path));
            }
        }
        let Some((_, path)) = best else {
            return Ok(String::new());
        };
        let content = fs::read_to_string(&path)
            .map_err(|error| AppError::Internal(format!("No se pudo leer log: {error}")))?;
        let tail = content
            .lines()
            .rev()
            .take(lines)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        Ok(tail)
    }

    fn detect_python() -> Result<String, AppError> {
        if let Ok(path) = std::env::var("KAMPLES_PYTHON_PATH") {
            if Self::python_works(&path) {
                return Ok(path);
            }
        }

        let venv_python = if cfg!(windows) {
            Self::scraper_dir()
                .join(".venv")
                .join("Scripts")
                .join("python.exe")
        } else {
            Self::scraper_dir().join(".venv").join("bin").join("python")
        };
        if venv_python.exists() {
            let path = venv_python.to_string_lossy().to_string();
            if Self::python_works(&path) {
                return Ok(path);
            }
        }

        for candidate in ["python3", "python", "py"] {
            if Self::python_works(candidate) {
                return Ok(candidate.to_string());
            }
        }

        Err(AppError::BadRequest(
            "Python no encontrado en el sistema.".to_string(),
        ))
    }

    fn python_works(candidate: &str) -> bool {
        Command::new(candidate)
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
    }

    fn apply_python_env(command: &mut Command) {
        let mut has_backend_url = false;

        if let Ok(public_base_url) = env::var("PUBLIC_BASE_URL") {
            command.env("BACKEND_URL", &public_base_url);
            command.env("KAMPLES_INTERNAL_URL", &public_base_url);
            command.env("KAMPLES_SITE_URL", public_base_url);
            has_backend_url = true;
        }
        for key in ["BACKEND_URL", "KAMPLES_INTERNAL_URL", "KAMPLES_SITE_URL"] {
            if let Ok(value) = env::var(key) {
                if !value.trim().is_empty() {
                    has_backend_url = true;
                }
                command.env(key, value);
            }
        }

        if !has_backend_url {
            let backend_url = Self::local_backend_url();
            command.env("BACKEND_URL", &backend_url);
            command.env("KAMPLES_INTERNAL_URL", &backend_url);
            command.env("KAMPLES_SITE_URL", backend_url);
        }

        if let Ok(secret) = env::var("SCRAPER_SECRET") {
            command.env("SCRAPER_SECRET", &secret);
            command.env("KAMPLES_CRON_SECRET", secret);
        } else if let Ok(secret) = env::var("KAMPLES_CRON_SECRET") {
            command.env("KAMPLES_CRON_SECRET", &secret);
            command.env("SCRAPER_SECRET", secret);
        }

        let Ok(database_url) = env::var("DATABASE_URL") else {
            return;
        };
        let Ok(url) = Url::parse(&database_url) else {
            return;
        };

        if let Some(host) = url.host_str() {
            command.env("KAMPLES_PG_HOST", host);
        }
        if let Some(port) = url.port() {
            command.env("KAMPLES_PG_PORT", port.to_string());
        }
        let database = url.path().trim_start_matches('/');
        if !database.is_empty() {
            command.env("KAMPLES_PG_DBNAME", database);
        }
        if !url.username().is_empty() {
            command.env("KAMPLES_PG_USER", url.username());
        }
        if let Some(password) = url.password() {
            command.env("KAMPLES_PG_PASSWORD", password);
        }
    }

    fn local_backend_url() -> String {
        let raw_host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let host = match raw_host.trim() {
            "" | "0.0.0.0" | "::" => "127.0.0.1",
            value => value,
        };
        /* [13AA-1] 3001 = puerto del backend en registry y .env.example; 3000 es de glory-pulse. */
        let port = env::var("PORT").unwrap_or_else(|_| "3001".to_string());
        format!("http://{host}:{port}")
    }

    fn pid_alive(pid: u32) -> bool {
        if cfg!(windows) {
            Command::new("tasklist")
                .args(["/FI", &format!("PID eq {pid}"), "/NH"])
                .output()
                .is_ok_and(|output| {
                    String::from_utf8_lossy(&output.stdout).contains(&pid.to_string())
                })
        } else {
            Command::new("kill")
                .args(["-0", &pid.to_string()])
                .status()
                .is_ok_and(|status| status.success())
        }
    }

    fn kill_process_tree(pid: u32) -> Result<(), AppError> {
        let status = if cfg!(windows) {
            Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .status()
        } else {
            Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status()
        }
        .map_err(|error| AppError::Internal(format!("No se pudo detener proceso: {error}")))?;

        if status.success() {
            Ok(())
        } else {
            Err(AppError::Internal(format!(
                "El comando para detener PID {pid} fallo"
            )))
        }
    }

    fn validate_cookie_type(cookie_type: &str) -> Result<(), AppError> {
        if COOKIE_TYPES.contains(&cookie_type) {
            Ok(())
        } else {
            Err(AppError::BadRequest(format!(
                "Tipo de cookies invalido: {cookie_type}"
            )))
        }
    }

    fn cookie_filename(cookie_type: &str) -> Result<&'static str, AppError> {
        match cookie_type {
            "youtube" => Ok("cookies_youtube.txt"),
            "soundcloud" => Ok("cookies_soundcloud.txt"),
            other => Err(AppError::BadRequest(format!(
                "Tipo de cookies invalido: {other}"
            ))),
        }
    }

    fn cookie_info(cookie_type: &str) -> AdminProcessCookieInfo {
        let primary = match Self::cookie_filename(cookie_type) {
            Ok(filename) => Self::scraper_dir().join(filename),
            Err(_) => return AdminProcessCookieInfo::missing(),
        };
        let path = if cookie_type == "youtube" && !primary.exists() {
            let legacy = Self::scraper_dir().join("cookies.txt");
            if legacy.exists() {
                legacy
            } else {
                primary
            }
        } else {
            primary
        };

        let Ok(metadata) = fs::metadata(&path) else {
            return AdminProcessCookieInfo::missing();
        };
        let modified = metadata.modified().ok().map(DateTime::<Utc>::from);
        AdminProcessCookieInfo {
            existe: true,
            tamano: Some(metadata.len()),
            modificado: modified,
        }
    }
}
