/* [274A-55..57] Helpers de proceso para scraper/extractor dev.
 * Usa `std::process::Command` con argumentos separados, sin shell. */

use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::errors::AppError;

const SCRAPER_DIR_DEFAULT: &str = "clients/kamples-scraper";
const LOG_DIR_DEFAULT: &str = "logs";
const WHOSAMPLED_HOST: &str = "whosampled.com";

pub struct StartedProcess {
    pub pid: Option<u32>,
    pub log_display: String,
}

pub fn validate_spider(spider: &str) -> Result<(), AppError> {
    const ALLOWED: &[&str] = &[
        "hot_samples",
        "browse_year",
        "track",
        "sample_detail",
        "artist",
    ];
    if ALLOWED.contains(&spider) {
        Ok(())
    } else {
        Err(AppError::BadRequest("Spider no permitido".into()))
    }
}

pub fn validate_whosampled_url(url: &str) -> Result<(), AppError> {
    let parsed = url::Url::parse(url).map_err(|_| AppError::BadRequest("URL inválida".into()))?;
    let host = parsed.host_str().unwrap_or_default();
    if host == WHOSAMPLED_HOST || host.ends_with(".whosampled.com") {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "URL no permitida. Solo URLs de WhoSampled.".into(),
        ))
    }
}

pub fn spider_para_tipo(
    tipo: &str,
    url_completa: &str,
    modo_cola: bool,
) -> Result<(String, Vec<String>), AppError> {
    let depth = if modo_cola { "0" } else { "2" };
    let args = match tipo {
        "track" | "track_samples" | "track_sampled" => (
            "track".to_string(),
            vec![
                "-a".into(),
                format!("start_url={url_completa}"),
                "-s".into(),
                format!("DEPTH_LIMIT={depth}"),
            ],
        ),
        "sample_detail" | "cover_detail" | "remix_detail" => (
            "sample_detail".to_string(),
            vec![
                "-a".into(),
                format!("urls={url_completa}"),
                "-s".into(),
                format!("DEPTH_LIMIT={depth}"),
            ],
        ),
        "artist" => (
            "artist".to_string(),
            vec![
                "-a".into(),
                format!("start_url={url_completa}"),
                "-s".into(),
                format!("DEPTH_LIMIT={depth}"),
            ],
        ),
        "hot_samples" => ("hot_samples".to_string(), Vec::new()),
        "browse_year" => ("browse_year".to_string(), Vec::new()),
        _ => {
            return Err(AppError::BadRequest(format!(
                "Tipo '{tipo}' sin spider configurado"
            )))
        }
    };
    Ok(args)
}

pub fn spawn_scrapy(
    spider: &str,
    extra_args: &[String],
    log_prefix: &str,
) -> Result<StartedProcess, AppError> {
    let mut args = vec![
        "-m".to_string(),
        "scrapy".to_string(),
        "crawl".to_string(),
        spider.to_string(),
    ];
    args.extend(extra_args.iter().cloned());
    spawn_python(&args, log_prefix)
}

pub fn spawn_extractor_pipeline(limit: usize) -> Result<StartedProcess, AppError> {
    let output_dir = std::env::var("EXTRACCION_STAGING_DIR")
        .unwrap_or_else(|_| "uploads/kamples/extracciones".to_string());
    spawn_python(
        &[
            "-m".into(),
            "extractor.pipeline".into(),
            "--limit".into(),
            limit.to_string(),
            "--output-dir".into(),
            output_dir,
        ],
        "extractor-output",
    )
}

fn spawn_python(args: &[String], log_prefix: &str) -> Result<StartedProcess, AppError> {
    let scraper_dir = scraper_dir();
    if !scraper_dir.is_dir() {
        return Err(AppError::Internal(format!(
            "Directorio del scraper no encontrado: {}",
            scraper_dir.display()
        )));
    }
    let python = detect_python(&scraper_dir)?;
    let log_path = build_log_path(log_prefix)?;
    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| AppError::Internal(format!("abrir log scraper: {e}")))?;
    let stderr = stdout
        .try_clone()
        .map_err(|e| AppError::Internal(format!("clonar log scraper: {e}")))?;

    let child = Command::new(python)
        .args(args)
        .current_dir(&scraper_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|e| AppError::Internal(format!("iniciar scraper: {e}")))?;

    Ok(StartedProcess {
        pid: Some(child.id()),
        log_display: log_path.to_string_lossy().replace('\\', "/"),
    })
}

fn scraper_dir() -> PathBuf {
    std::env::var("KAMPLES_SCRAPER_DIR")
        .map_or_else(|_| PathBuf::from(SCRAPER_DIR_DEFAULT), PathBuf::from)
}

fn detect_python(scraper_dir: &Path) -> Result<PathBuf, AppError> {
    let venv = if cfg!(windows) {
        scraper_dir.join(".venv/Scripts/python.exe")
    } else {
        scraper_dir.join(".venv/bin/python")
    };
    if venv.is_file() {
        return Ok(venv);
    }
    std::env::var("PYTHON")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("PYTHON_EXECUTABLE").map(PathBuf::from))
        .or_else(|_| Ok(PathBuf::from("python")))
}

fn build_log_path(prefix: &str) -> Result<PathBuf, AppError> {
    let dir = PathBuf::from(LOG_DIR_DEFAULT);
    std::fs::create_dir_all(&dir).map_err(|e| AppError::Internal(format!("crear logs: {e}")))?;
    Ok(dir.join(format!( // sentinel-disable path-join-sin-canonicalize — prefijo literal (scraper-output) y una fecha formateada
        "{prefix}-{}.log",
        chrono::Utc::now().format("%Y-%m-%d")
    )))
}
