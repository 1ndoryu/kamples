/* sentinel-disable-file limite-lineas — gestor central legacy de procesos admin: scraping, extraccion, seed, locks, cookies y entorno Python. 294A-1 extrae helpers de spawn_python; dividir el modulo completo requiere separar contratos/rutas en una tarea arquitectonica propia. */

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::PgPool;
use url::Url;

use crate::errors::AppError;
use crate::models::{AdminProcessCookieInfo, AdminProcessState, AdminProcessesResponse};
use crate::repositories::{AdminAutomationRepository, AutomationBatchUpdate};
use crate::services::AdminSeedService;

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

    fn spawn_python(
        kind: ProcessKind,
        limit: Option<u32>,
        batch_id: Option<i64>,
    ) -> Result<Value, AppError> {
        let python = Self::detect_python()?;
        let scraper_dir = Self::scraper_dir();
        if !scraper_dir.is_dir() {
            return Err(AppError::BadRequest(
                "Directorio del scraper no encontrado.".to_string(),
            ));
        }
        Self::validate_python_entry(kind, &scraper_dir)?;
        Self::validate_python_dependencies(kind, &python, &scraper_dir)?;

        let log_path = Self::log_path(kind.name())?;
        Self::append_log_header(&log_path, kind.name())?;
        let stdout = Self::open_log_append(&log_path)?;
        let stderr = Self::open_log_append(&log_path)?;

        let mut command = Self::build_python_command(
            kind,
            &python,
            &scraper_dir,
            stdout,
            stderr,
            limit,
            batch_id,
        )?;

        let mut child = command
            .spawn()
            .map_err(|error| AppError::Internal(format!("No se pudo iniciar proceso: {error}")))?;
        let pid = child.id();

        std::thread::sleep(Duration::from_millis(1_500));
        if let Some(status) = child.try_wait().map_err(|error| {
            AppError::Internal(format!(
                "No se pudo verificar arranque del proceso: {error}"
            ))
        })? {
            return Self::handle_immediate_exit(kind, pid, &log_path, status);
        }

        Self::write_lock(&ProcessLock {
            nombre: kind.name().to_string(),
            estado: "running".to_string(),
            pid: Some(pid),
            iniciado_at: Some(Utc::now()),
            ultimo_log: Some(Utc::now()),
            progreso: None,
            error: None,
            resultado: None,
        })?;

        Ok(json!({
            "ok": true,
            "pid": pid,
            "mensaje": format!("Proceso '{}' iniciado.", kind.name()),
            "log": log_path.file_name().map(|name| name.to_string_lossy().to_string()),
        }))
    }

    fn build_python_command(
        kind: ProcessKind,
        python: &str,
        scraper_dir: &Path,
        stdout: File,
        stderr: File,
        limit: Option<u32>,
        batch_id: Option<i64>,
    ) -> Result<Command, AppError> {
        let mut command = Command::new(python);
        command
            .current_dir(scraper_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        Self::apply_python_env(&mut command);
        if let Some(batch_id) = batch_id {
            command.env("KAMPLES_BATCH_ID", batch_id.to_string());
        }

        match kind {
            ProcessKind::Scraping => {
                command.arg("scripts/cron_runner.py").arg("daily");
            }
            ProcessKind::Extraction => {
                command
                    .arg("-m")
                    .arg("extractor.pipeline")
                    .arg("--limit")
                    .arg(limit.unwrap_or(20).to_string())
                    .arg("--output-dir")
                    .arg(Self::extractions_dir()?);
            }
            ProcessKind::Seed => unreachable!("seed no usa python"),
        }

        Ok(command)
    }

    fn handle_immediate_exit(
        kind: ProcessKind,
        pid: u32,
        log_path: &Path,
        status: std::process::ExitStatus,
    ) -> Result<Value, AppError> {
        let tail = Self::read_log_tail(kind.name(), 80).unwrap_or_default();
        let exit_code = status
            .code()
            .map_or_else(|| "sin codigo".to_string(), |code| code.to_string());
        let inferred_from_tail = Self::infer_error_from_tail(&tail);
        let failed = !status.success() || inferred_from_tail.is_some();
        let message =
            inferred_from_tail.unwrap_or_else(|| immediate_exit_message(kind, failed, &exit_code));

        Self::write_lock(&ProcessLock {
            nombre: kind.name().to_string(),
            estado: if failed { "error" } else { "stopped" }.to_string(),
            pid: None,
            iniciado_at: Some(Utc::now()),
            ultimo_log: Some(Utc::now()),
            progreso: if failed { None } else { Some(100.0) },
            error: if failed { Some(message.clone()) } else { None },
            resultado: None,
        })?;

        if failed {
            Err(AppError::BadRequest(message))
        } else {
            Ok(json!({
                "ok": true,
                "pid": pid,
                "mensaje": message,
                "log": log_path.file_name().map(|name| name.to_string_lossy().to_string()),
            }))
        }
    }

    fn validate_python_entry(kind: ProcessKind, scraper_dir: &Path) -> Result<(), AppError> {
        let required_path = match kind {
            ProcessKind::Scraping => scraper_dir.join("scripts").join("cron_runner.py"),
            ProcessKind::Extraction => scraper_dir.join("extractor").join("pipeline.py"),
            ProcessKind::Seed => return Ok(()),
        };

        if required_path.is_file() {
            Ok(())
        } else {
            Err(AppError::BadRequest(format!(
                "No se encontro el entrypoint de '{}': {}",
                kind.name(),
                required_path.display()
            )))
        }
    }

    fn validate_python_dependencies(
        kind: ProcessKind,
        python: &str,
        scraper_dir: &Path,
    ) -> Result<(), AppError> {
        let modules = match kind {
            ProcessKind::Scraping => &["scrapy", "psycopg2", "dotenv"][..],
            ProcessKind::Extraction => &[
                "yt_dlp",
                "librosa",
                "soundfile",
                "numpy",
                "psycopg2",
                "dotenv",
            ][..],
            ProcessKind::Seed => return Ok(()),
        };
        let modules_literal = modules
            .iter()
            .map(|module| format!("{module:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        let script = format!(
            "import importlib, sys\nmissing = []\nfor name in [{modules_literal}]:\n    try:\n        importlib.import_module(name)\n    except Exception as error:\n        missing.append(f'{{name}}: {{error}}')\nif missing:\n    sys.stderr.write('\\n'.join(missing))\n    sys.exit(1)\n"
        );

        let output = Command::new(python)
            .current_dir(scraper_dir)
            .arg("-c")
            .arg(script)
            .output()
            .map_err(|error| AppError::Internal(format!("No se pudo validar Python: {error}")))?;

        if output.status.success() {
            return Ok(());
        }

        let details = Self::combine_output(&output.stdout, &output.stderr);
        Err(AppError::BadRequest(format!(
            "Dependencias Python faltantes para '{}': {}. Crea/actualiza el venv del scraper con `python -m venv clients/kamples-scraper/.venv` y `clients/kamples-scraper/.venv/Scripts/python.exe -m pip install -r clients/kamples-scraper/requirements.txt`.",
            kind.name(),
            Self::truncate_message(&details, 900)
        )))
    }

    fn combine_output(stdout: &[u8], stderr: &[u8]) -> String {
        let stdout_text = String::from_utf8_lossy(stdout).trim().to_string();
        let stderr_text = String::from_utf8_lossy(stderr).trim().to_string();
        match (stdout_text.is_empty(), stderr_text.is_empty()) {
            (true, true) => "sin salida del proceso".to_string(),
            (false, true) => stdout_text,
            (true, false) => stderr_text,
            (false, false) => format!("{stdout_text}\n{stderr_text}"),
        }
    }

    fn infer_error_from_tail(tail: &str) -> Option<String> {
        let relevant_lines = tail
            .lines()
            .filter(|line| {
                line.contains("ERROR")
                    || line.contains("Traceback")
                    || line.contains("Error ")
                    || line.contains("No module named")
                    || line.contains("ModuleNotFoundError")
                    || line.contains("fallo con codigo")
                    || line.contains("failed")
                    || line.contains("FATAL")
            })
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();

        if relevant_lines.is_empty() {
            None
        } else {
            Some(format!(
                "El proceso fallo. Ultimas lineas relevantes: {}",
                Self::truncate_message(&relevant_lines.join(" | "), 900)
            ))
        }
    }

    fn truncate_message(message: &str, max_chars: usize) -> String {
        let mut truncated = message.chars().take(max_chars).collect::<String>();
        if message.chars().count() > max_chars {
            truncated.push_str("...");
        }
        truncated
    }

    async fn run_seed(kind: ProcessKind, pool: &PgPool) -> Result<Value, AppError> {
        Self::write_lock(&ProcessLock {
            nombre: kind.name().to_string(),
            estado: "running".to_string(),
            pid: None,
            iniciado_at: Some(Utc::now()),
            ultimo_log: Some(Utc::now()),
            progreso: None,
            error: None,
            resultado: None,
        })?;

        match AdminSeedService::run(pool).await {
            Ok(resultado) => {
                Self::write_lock(&ProcessLock {
                    nombre: kind.name().to_string(),
                    estado: "stopped".to_string(),
                    pid: None,
                    iniciado_at: Some(Utc::now()),
                    ultimo_log: Some(Utc::now()),
                    progreso: Some(100.0),
                    error: None,
                    resultado: Some(resultado.clone()),
                })?;

                Ok(json!({
                    "ok": true,
                    "mensaje": "Proceso 'seed' completado.",
                    "resultado": resultado,
                }))
            }
            Err(error) => {
                let message = error.to_string();
                Self::write_lock(&ProcessLock {
                    nombre: kind.name().to_string(),
                    estado: "error".to_string(),
                    pid: None,
                    iniciado_at: Some(Utc::now()),
                    ultimo_log: Some(Utc::now()),
                    progreso: None,
                    error: Some(message.clone()),
                    resultado: None,
                })?;
                Err(AppError::Internal(format!(
                    "Proceso 'seed' fallo: {message}"
                )))
            }
        }
    }

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

fn immediate_exit_message(kind: ProcessKind, failed: bool, exit_code: &str) -> String {
    if failed {
        format!(
            "Proceso '{}' fallo al arrancar (codigo {exit_code}). Revisa el log reciente.",
            kind.name()
        )
    } else {
        format!(
            "Proceso '{}' termino inmediatamente sin errores reportados.",
            kind.name()
        )
    }
}
