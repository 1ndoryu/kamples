//! Rescata 1 archivo de audio del servidor kamples caído y lo importa
//! en la BD local + storage del proyecto glory-rust-template.
//!
//! Uso:
//!   cargo run --bin rescatar -- --archivo "kamples/0/2026/04/Vocals-Electronic-C-89bpm-kraftwerk-vocoder-phrase-kamples-oYxPk9V.mp3"
//!
//! [pendiente-asignar-id] Herramienta de rescate post-desastre.
//!   - Descarga vía SSH + base64 (evita depender de coolify-manager-rs).
//!   - Parsea filename kamples para extraer BPM, key, género, artista.
//!   - Inserta en BD local + escribe archivo en ./uploads/.
//!   - Encola análisis IA para que el pipeline complete metadatos.

#![deny(clippy::all)]

use anyhow::{Context, Result};
use base64::Engine;
use chrono::{Datelike, Utc};
use clap::Parser;
use glory_backend::audio::ffmpeg::inspect_audio_file;
use sha2::{Digest, Sha256};
use slug::slugify;
use sqlx::postgres::PgPoolOptions;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

/// Rescata archivos de audio del servidor kamples caído (desastre `PostgreSQL`).
/// Descarga 1 archivo vía SSH, lo importa en la BD local y lo coloca en el storage
/// del proyecto para verificar que el flujo de rescate funciona.
#[derive(Parser)]
#[command(
    name = "rescatar",
    about = "Rescata 1 sample del servidor caído al entorno local"
)]
struct Args {
    /// Ruta remota del archivo dentro del volumen uploads-data
    /// Ej: "kamples/0/2026/04/Vocals-Electronic-C-89bpm-kraftwerk-vocoder-phrase-kamples-oYxPk9V.mp3"
    #[arg(short, long)]
    archivo: String,

    /// ID del creador (`usuarios_ext.id`) al que asignar el sample
    #[arg(short = 'u', long, default_value = "2")]
    creador_id: i32,

    /// Host del servidor (IP o dominio)
    #[arg(short = 'H', long, default_value = "66.94.100.241")]
    host: String,

    /// Ruta a la clave SSH privada. Por defecto busca en ~/.ssh/ y `SSH_KEY_PATH`
    #[arg(short = 'k', long)]
    ssh_key: Option<String>,

    /// Título del sample. Por defecto se extrae del filename
    #[arg(short, long)]
    titulo: Option<String>,

    /// Tags separados por coma
    #[arg(short = 'T', long, default_value = "rescatado,kamples")]
    tags: String,

    /// No preguntar confirmación
    #[arg(long, default_value_t = false)]
    yes: bool,
}

#[allow(clippy::too_many_lines)]
fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let args = Args::parse();

    // ─── Resumen ────────────────────────────────────────────────────────────
    let filename = Path::new(&args.archivo)
        .file_name()
        .context("No se pudo extraer el nombre del archivo de --archivo")?
        .to_string_lossy()
        .to_string();

    println!("=== Rescate de Sample ===");
    println!("  Archivo remoto:  {}", args.archivo);
    println!("  Nombre:          {filename}");
    println!("  Creador ID:      {}", args.creador_id);
    println!("  Host:            {}", args.host);

    if !args.yes {
        println!();
        print!("¿Continuar? (s/N): ");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("s") {
            println!("Cancelado.");
            return Ok(());
        }
    }

    // ─── 1. Resolver SSH key ────────────────────────────────────────────────
    let ssh_key = resolve_ssh_key(args.ssh_key.as_ref())?;
    println!("  SSH key:         {ssh_key}");

    // ─── 2. Ruta remota absoluta ────────────────────────────────────────────
    let volume_uploads = "/var/lib/docker/volumes/mo4so4440c488g8woow4cow0_uploads-data/_data";
    let remote_full = format!("{}/{}", volume_uploads, args.archivo);

    // ─── 3. Verificar existencia ────────────────────────────────────────────
    let check = ssh_exec(
        &args.host,
        &ssh_key,
        &format!("test -f '{remote_full}' && echo 'EXISTS' || echo 'NOT_FOUND'"),
    )?;
    if check.trim() != "EXISTS" {
        anyhow::bail!("El archivo remoto no existe o no es accesible:\n  {remote_full}");
    }
    println!("  ✓ Archivo existe en el servidor");

    // ─── 4. Tamaño remoto ───────────────────────────────────────────────────
    let remote_size = ssh_exec(
        &args.host,
        &ssh_key,
        &format!("stat --format='%s' '{remote_full}'"),
    )?;
    let remote_size = remote_size.trim().parse::<u64>().unwrap_or(0);
    #[allow(clippy::cast_precision_loss)]
    let remote_size_mb = remote_size as f64 / 1_048_576.0;
    println!("  Tamaño remoto:   {remote_size_mb:.2} MB");

    // ─── 5. Descargar vía base64 ────────────────────────────────────────────
    println!("  ↓ Descargando...");
    let b64_output = ssh_exec(&args.host, &ssh_key, &format!("base64 -w0 '{remote_full}'"))?;
    let audio_bytes = base64::engine::general_purpose::STANDARD
        .decode(b64_output.trim())
        .context("Error al decodificar base64 del archivo remoto")?;
    #[allow(clippy::cast_precision_loss)]
    let downloaded_mb = audio_bytes.len() as f64 / 1_048_576.0;
    println!(
        "  ✓ Descargado: {} bytes ({downloaded_mb:.2} MB)",
        audio_bytes.len()
    );

    // ─── 6. Parsear filename ────────────────────────────────────────────────
    let metadata = parse_filename(&filename);
    let titulo = args.titulo.clone().unwrap_or(metadata.titulo.clone());
    let extension = filename.rsplit('.').next().unwrap_or("mp3").to_string();

    // ─── 7. Generar id_corto y slug ─────────────────────────────────────────
    let id_corto = generate_short_id();
    let slug_base = slugify(&titulo);
    let slug = if slug_base.is_empty() {
        format!("sample-{id_corto}")
    } else {
        format!("{slug_base}-{id_corto}")
    };

    // ─── 8. Guardar archivo en storage ─────────────────────────────────────
    let now = Utc::now();
    let storage_key = format!(
        "samples/{}/{:04}/{:02}/{}.{}",
        args.creador_id,
        now.year(),
        now.month(),
        slug,
        extension
    );
    let storage_root = env::var("STORAGE_ROOT").unwrap_or_else(|_| "./uploads".to_string());
    let storage_path = PathBuf::from(&storage_root).join(&storage_key); // sentinel-disable path-join-sin-canonicalize — los segmentos son un i32, un slug y una extension de file_name()
    if let Some(parent) = storage_path.parent() {
        fs::create_dir_all(parent).context("Error al crear directorios de storage")?;
    }
    fs::write(&storage_path, &audio_bytes).context("Error al escribir archivo en storage")?;
    println!("  ✓ Archivo guardado: {}", storage_path.display());

    // ─── 9. Insertar en BD ──────────────────────────────────────────────────
    let database_url = env::var("DATABASE_URL").context("DATABASE_URL no definida en .env")?;
    println!("  🗄  Conectando a BD...");

    let rt = tokio::runtime::Runtime::new()?;
    let sample_id = rt.block_on(async {
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(&database_url)
            .await
            .context("Error al conectar a BD")?;

        // Calcular hash SHA-256
        let audio_hash = {
            let mut hasher = Sha256::new();
            hasher.update(&audio_bytes);
            format!("{:x}", hasher.finalize())
        };

        let formato = extension.to_uppercase();
        #[allow(clippy::cast_possible_wrap)]
        let tamano = audio_bytes.len() as i64;

        // Tags como array PostgreSQL
        let tags: Vec<String> = args.tags.split(',').map(|s| s.trim().to_string()).collect();

        // Metadata con info del rescate
        let mut meta = serde_json::json!({
            "rescatado": true,
            "fecha_rescate": Utc::now().to_rfc3339(),
            "archivo_original": args.archivo,
            "servidor_origen": args.host,
        });
        if let Some(v) = &metadata.bpm {
            meta["bpm"] = serde_json::json!(v);
        }
        if let Some(v) = &metadata.key {
            meta["tonalidad"] = serde_json::json!(v);
        }
        if let Some(v) = &metadata.genero {
            meta["genero"] = serde_json::json!(v);
        }
        if let Some(v) = &metadata.artista {
            meta["artista"] = serde_json::json!(v);
        }

        // Mapear tipo desde filename (Vocals → "vocal", Loop → "loop", etc.)
        let tipo_sample = metadata.tipo.as_deref().map_or("loop", map_tipo);

        // INSERT con todos los campos relevantes
        // Los argumentos son tipados por la columna: bpm INT, key VARCHAR(3), tamano BIGINT
        let fila_id = sqlx::query_scalar!(
            r#"
            INSERT INTO samples (
                creador_id, titulo, slug, id_corto, descripcion,
                formato, tamano, tags, audio_hash,
                estado, ruta_original,
                permitir_descarga, licencia_libre, es_premium,
                precio, mostrar_en_comunidad, metadata,
                bpm, key, tipo, duracion
            )
            VALUES ($1, $2, $3, $4, $5,
                    $6, $7, $8, $9,
                    'activo', $10,
                    true, false, false,
                    0.0, true, $11,
                    $12, $13, $14, 0.0)
            RETURNING id AS "id!"
            "#,
            args.creador_id,
            titulo,
            slug,
            id_corto,
            "Sample rescatado del servidor",
            formato,
            tamano,
            &tags,
            audio_hash,
            storage_key,
            meta,
            metadata.bpm,
            metadata.key.as_deref(),
            tipo_sample
        )
        .fetch_one(&pool)
        .await
        .context("Error al insertar sample en BD")?;

        // Actualizar contador en usuarios_ext
        sqlx::query!(
            "UPDATE usuarios_ext SET total_samples = COALESCE(total_samples, 0) + 1 WHERE id = $1",
            args.creador_id
        )
        .execute(&pool)
        .await
        .context("Error al actualizar contador de samples del usuario")?;

        // ─── Análisis de audio: duración + waveform ────────────────────────────
        println!("  🎵 Analizando audio...");
        let audio_meta = inspect_audio_file(&storage_path, Some("mp3"), None)
            .await
            .context("Error al inspeccionar archivo de audio")?;
        let duracion = audio_meta.duration_seconds;
        println!(
            "     Duración: {:.2}s | Formato: {} | Peaks: {}",
            duracion,
            audio_meta.format,
            audio_meta.waveform_peaks.len()
        );

        // Guardar waveform como JSON en storage
        let waveform_key = {
            let parent = Path::new(&storage_key)
                .parent()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            format!("{parent}/{id_corto}_waveform.json")
        };
        let waveform_storage_path = PathBuf::from(&storage_root).join(&waveform_key); // sentinel-disable path-join-sin-canonicalize — los segmentos son un i32, un slug y una extension de file_name()
        if let Some(parent) = waveform_storage_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let waveform_bytes = serde_json::to_vec(&audio_meta.waveform_peaks)
            .context("Error al serializar waveform")?;
        fs::write(&waveform_storage_path, &waveform_bytes).context("Error al guardar waveform")?;

        // Actualizar sample con duración, ruta_optimizada (mismo archivo, ya es MP3),
        // waveform y publicado_at para que aparezca primero en "Recientes"
        // duracion es REAL en BD: el f32 de inspect_audio_file encaja sin conversión
        sqlx::query!(
            r#"
            UPDATE samples
            SET duracion = $1,
                ruta_optimizada = $2,
                ruta_waveform = $3,
                publicado_at = $4,
                formato = $5
            WHERE id = $6
            "#,
            duracion,
            storage_key, // ruta_optimizada = misma que original (ya es MP3)
            waveform_key,
            Utc::now(),
            formato,
            fila_id
        )
        .execute(&pool)
        .await
        .context("Error al actualizar análisis de audio en BD")?;

        // Encolar análisis IA técnico (cola_procesamiento_ia) para workers
        // que procesan embedding vectorial cuando estén habilitados.
        sqlx::query!(
            r#"
            INSERT INTO cola_procesamiento_ia (tipo, entidad_id, operacion, metadata)
            VALUES ('sample', $1, 'analisis_audio', $2)
            "#,
            fila_id,
            serde_json::json!({"prioridad": "baja", "origen": "rescate"})
        )
        .execute(&pool)
        .await
        .context("Error al encolar análisis IA técnico")?;

        // Encolar metadata creativa IA (ia_queue) para que el worker de IA
        // genere tags, género, emoción, instrumentos, etc. cuando esté activo.
        // Esto salta el pipeline técnico (rescatar ya hizo duración/waveform/BPM/key).
        // Usamos ON CONFLICT sobre el índice parcial para no duplicar si ya existe
        // una entrada activa para este sample.
        sqlx::query!(
            r#"
            INSERT INTO ia_queue (sample_id, status, metadata)
            VALUES ($1, 'pending', $2)
            ON CONFLICT (sample_id) WHERE status IN ('pending', 'processing', 'retry_scheduled')
            DO NOTHING
            "#,
            fila_id,
            serde_json::json!({"origen": "rescate"})
        )
        .execute(&pool)
        .await
        .context("Error al encolar metadata creativa IA")?;

        Ok::<i32, anyhow::Error>(fila_id)
    })?;

    // ─── 10. Resultado final ────────────────────────────────────────────────
    let public_base =
        env::var("PUBLIC_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:3001".to_string());
    let upload_url = format!(
        "{}/uploads/{}",
        public_base.trim_end_matches('/'),
        storage_key
    );

    println!();
    println!("🎉 ¡RESCATE COMPLETADO!");
    println!("   ID:            {sample_id}");
    println!("   ID corto:      {id_corto}");
    println!("   Slug:          {slug}");
    println!("   Storage key:   {storage_key}");
    println!("   Archivo local: {}", storage_path.display());
    println!("   URL pública:   {upload_url}");
    println!();
    println!("   Para verlo:  cargo run --bin glory-backend");
    println!("   Y luego:     curl http://127.0.0.1:3001/api/samples/{sample_id}");

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Funciones auxiliares
// ═══════════════════════════════════════════════════════════════════════════════

/// Resuelve la ruta a la clave SSH privada.
/// Busca en: --ssh-key > `SSH_KEY_PATH` > ~/.`ssh/id_ed25519` > ~/.`ssh/id_rsa`
fn resolve_ssh_key(cli_key: Option<&String>) -> Result<String> {
    // Si se pasó por CLI, usarla directamente
    if let Some(path) = cli_key {
        if Path::new(path).exists() {
            return Ok(path.clone());
        }
        anyhow::bail!("Clave SSH no encontrada en la ruta especificada: {path}");
    }

    // Variable de entorno SSH_KEY_PATH
    if let Ok(env_key) = env::var("SSH_KEY_PATH") {
        if Path::new(&env_key).exists() {
            return Ok(env_key);
        }
    }

    // Buscar en ~/.ssh/
    let home = env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .context("No se pudo determinar HOME/USERPROFILE")?;
    let ssh_dir = PathBuf::from(&home).join(".ssh");
    for key_name in ["id_ed25519", "id_rsa", "id_ecdsa"] {
        let key_path = ssh_dir.join(key_name);
        if key_path.exists() {
            return Ok(key_path.to_string_lossy().to_string());
        }
    }

    anyhow::bail!(
        "No se encontró clave SSH.\n\
         Opciones:\n\
         \x20 1. Usar --ssh-key <ruta>\n\
         \x20 2. Definir SSH_KEY_PATH en .env\n\
         \x20 3. Tener id_ed25519 o id_rsa en ~/.ssh/"
    );
}

/// Ejecuta un comando vía SSH y devuelve stdout como String.
/// Usa `ssh -o StrictHostKeyChecking=accept-new` para evitar prompts.
fn ssh_exec(host: &str, ssh_key: &str, command: &str) -> Result<String> {
    let output = Command::new("ssh")
        .arg("-i")
        .arg(ssh_key)
        .arg("-o")
        .arg("StrictHostKeyChecking=accept-new")
        .arg("-o")
        .arg("ConnectTimeout=15")
        .arg(format!("root@{host}"))
        .arg(command)
        .output()
        .with_context(|| format!("Error al ejecutar SSH en {host}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "SSH falló (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Parsing de filename estilo kamples
// ═══════════════════════════════════════════════════════════════════════════════

/// Metadatos extraídos del filename estilo kamples.
#[derive(Debug)]
struct FileMetadata {
    titulo: String,
    bpm: Option<i32>,
    key: Option<String>,
    genero: Option<String>,
    tipo: Option<String>,
    artista: Option<String>,
}

/// Parsea el filename estilo kamples para extraer metadatos.
///
/// Formato típico:
///   `{tipo}-{genero}-{tonalidad}-{bpm}bpm-{artista}-{titulo}-kamples-{id_corto}.mp3`
///
/// Ejemplo real:
///   `Vocals-Electronic-C-89bpm-kraftwerk-vocoder-phrase-kamples-oYxPk9V.mp3`
///
/// El parser busca:
///   - El sufijo "bpm" para extraer BPM y deducir tonalidad + género.
///   - La palabra "kamples" como separador entre título e `id_corto`.
///   - El primer token como tipo de sample (Vocals, Loop, FX, etc.).
fn parse_filename(filename: &str) -> FileMetadata {
    let stem = Path::new(filename)
        .file_stem()
        .map_or_else(|| filename.to_string(), |s| s.to_string_lossy().to_string());

    let titulo_fallback = stem.clone();
    let parts: Vec<&str> = stem.split('-').collect();

    let mut meta = FileMetadata {
        titulo: titulo_fallback,
        bpm: None,
        key: None,
        genero: None,
        tipo: None,
        artista: None,
    };

    if parts.len() < 3 {
        return meta; // Muy corto para tener metadata útil
    }

    // Buscar BPM (patrón: "89bpm", "120bpm", etc.)
    for (i, part) in parts.iter().enumerate() {
        if let Some(bpm_str) = part.strip_suffix("bpm") {
            if let Ok(bpm) = bpm_str.parse::<i32>() {
                meta.bpm = Some(bpm);
                // Token anterior (i-1) → tonalidad (ej: "C", "Dm", "F#m", "Am")
                if i > 0 && parts[i - 1].len() <= 4 {
                    meta.key = Some(parts[i - 1].to_string());
                }
                // Dos tokens antes → género
                if i > 1 && parts[i - 2].len() < 20 {
                    meta.genero = Some(parts[i - 2].to_string());
                }
                break;
            }
        }
    }

    // Buscar "kamples" como separador
    if let Some(pos) = parts.iter().position(|&p| p == "kamples") {
        // Primer token → tipo de sample
        if !parts.is_empty() {
            meta.tipo = Some(parts[0].to_string());
        }
        // Tokens entre tipo (0) y "kamples" (pos)
        if pos > 1 {
            let middle: Vec<&str> = parts[1..pos].to_vec();
            if middle.len() > 1 {
                // El último antes de "kamples" es el id_corto original, ignorarlo
                // Los anteriores forman artista + título
                let artista_titulo: Vec<&str> = middle[..middle.len() - 1].to_vec();
                if !artista_titulo.is_empty() {
                    meta.artista = Some(artista_titulo.join("-"));
                }
            }
        }
        // Reconstruir título: todo antes de "kamples"
        let antes: Vec<&str> = parts[..pos]
            .iter()
            .filter(|&&p| !p.starts_with("kamples"))
            .copied()
            .collect();
        if !antes.is_empty() {
            meta.titulo = antes.join(" ");
            // Capitalizar primera letra
            let mut c = meta.titulo.chars();
            if let Some(first) = c.next() {
                meta.titulo = first.to_uppercase().to_string() + c.as_str();
            }
        }
    }

    meta
}

/// Mapea el tipo detectado en el filename al CHECK constraint de samples.tipo.
/// Los valores permitidos son: loop, oneshot, fx, vocal, stem, otro.
fn map_tipo(tipo: &str) -> &'static str {
    let lower = tipo.to_lowercase();
    match lower.as_str() {
        "vocals" | "vocal" | "voice" => "vocal",
        "loop" | "loops" | "music" => "loop",
        "oneshot" | "one_shot" | "one-shot" | "hit" => "oneshot",
        "fx" | "effects" | "effect" | "sfx" => "fx",
        "stem" | "stems" | "drums" | "bass" | "melody" | "chords" => "stem",
        _ => "otro",
    }
}

/// Genera un `id_corto` de 8 caracteres alfanuméricos (a-z, 0-9) usando nanoid.
fn generate_short_id() -> String {
    const ALPHABET: [char; 36] = [
        'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r',
        's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
    ];
    nanoid::nanoid!(8, &ALPHABET)
}
