use std::fs::File;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use chrono::Utc;
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::errors::AppError;
use crate::services::AdminSeedService;

use super::{AdminProcessService, ProcessKind, ProcessLock};

impl AdminProcessService {
    pub(super) fn spawn_python(
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

    pub(super) fn infer_error_from_tail(tail: &str) -> Option<String> {
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

    pub(super) async fn run_seed(kind: ProcessKind, pool: &PgPool) -> Result<Value, AppError> {
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
