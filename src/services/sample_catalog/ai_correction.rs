/* sentinel-disable-file limite-lineas — orquestador IA correccion sample */
/* [254A-8b] Servicio de correccion de metadata IA para samples extraidos por
 * el pipeline. Espejo Rust del PHP `SamplesModificacionController::corregirIA`
 * + `ServicioIA::corregirMetadata`.
 *
 * Flujo:
 *   1. Validar instrucciones (5..1000 chars).
 *   2. Cargar sample (id, titulo, bpm, key, escala, metadata, id_corto).
 *   3. Llamar `AudioIaService::correct_metadata` con prompt de correccion.
 *   4. Mergear AudioCreativeMetadata corregida con metadata original
 *      preservando campos del pipeline (youtube_id, lado_extraccion, etc.).
 *   5. UPDATE samples SET metadata, tipo, titulo+slug si cambia
 *      nombre_archivo_base, tags si vienen, descripcion si viene, updated_at.
 *   6. Devolver mapa de cambios para que el frontend refresque la UI.
 */

use chrono::Utc;
use serde::Serialize;
use serde_json::{json, Map, Value};
use sqlx::{PgPool, Postgres, QueryBuilder};

use crate::audio::ia::json_repairer::AudioCreativeMetadata;
use crate::errors::AppError;
use crate::services::{AudioIaService, AudioIaServiceError, MetadataCorrectionRequest};

const MIN_INSTRUCTIONS_LEN: usize = 5;
const MAX_INSTRUCTIONS_LEN: usize = 1_000;

const PRESERVED_PIPELINE_FIELDS: &[&str] = &[
    "youtube_id",
    "lado_extraccion",
    "relacion_id",
    "bpm_confianza",
    "key_confianza",
];

const VALID_SAMPLE_TYPES: &[&str] = &["loop", "oneshot"];

#[derive(Debug, Clone, Serialize)]
pub struct CorrectionOutcome {
    pub ok: bool,
    pub mensaje: String,
    pub cambios: Map<String, Value>,
}

struct SampleRow {
    id: i32,
    id_corto: Option<String>,
    titulo: String,
    bpm: Option<i32>,
    key: Option<String>,
    escala: Option<String>,
    metadata: Value,
}

#[allow(clippy::cast_precision_loss)]
pub async fn correct_sample_metadata(
    pool: &PgPool,
    ia: &AudioIaService,
    sample_id: i32,
    instructions: &str,
) -> Result<CorrectionOutcome, AppError> {
    let trimmed = instructions.trim();
    let len = trimmed.chars().count();
    if !(MIN_INSTRUCTIONS_LEN..=MAX_INSTRUCTIONS_LEN).contains(&len) {
        return Err(AppError::Validation(format!(
            "Las instrucciones deben tener entre {MIN_INSTRUCTIONS_LEN} y {MAX_INSTRUCTIONS_LEN} caracteres"
        )));
    }

    let row = load_sample_for_correction(pool, sample_id).await?;

    let metadata_json = serde_json::to_string(&row.metadata)
        .map_err(|error| AppError::Internal(format!("serializar metadata: {error}")))?;

    let result = ia
        .correct_metadata(&MetadataCorrectionRequest {
            current_metadata_json: metadata_json,
            title: row.titulo.clone(),
            instructions: trimmed.to_owned(),
            bpm: row.bpm.map(|value| value as f32),
            musical_key: row.key.clone(),
        })
        .await
        .map_err(|error| map_ia_error(&error))?;

    let merged_metadata = merge_corrected_metadata(&row.metadata, &result.metadata, trimmed);
    let new_type = pick_sample_type(&result.metadata.tipo);

    let title_and_slug = compute_new_title_and_slug(
        &result.metadata.nombre_archivo_base,
        row.bpm,
        row.key.as_deref(),
        row.escala.as_deref(),
        row.id_corto.as_deref(),
        row.id,
    );

    let new_tags = pick_new_tags(&result.metadata);
    let new_description = pick_new_description(&result.metadata);

    let mut cambios = Map::<String, Value>::new();
    cambios.insert("metadata".into(), Value::Bool(true));
    cambios.insert("tipo".into(), Value::String(new_type.clone()));
    if let Some((titulo, slug)) = title_and_slug.as_ref() {
        cambios.insert("titulo".into(), Value::String(titulo.clone()));
        cambios.insert("slug".into(), Value::String(slug.clone()));
    }
    if let Some(tags) = new_tags.as_ref() {
        cambios.insert(
            "tags".into(),
            Value::Array(tags.iter().cloned().map(Value::String).collect()),
        );
    }
    if let Some(desc) = new_description.as_ref() {
        cambios.insert("descripcion".into(), Value::String(desc.clone()));
    }

    let campos = CambiosCorreccion {
        metadata: &merged_metadata,
        new_type: &new_type,
        title_and_slug: title_and_slug.as_ref(),
        new_tags: new_tags.as_deref(),
        new_description: new_description.as_deref(),
    };
    apply_correction_update(pool, row.id, campos).await?;

    Ok(CorrectionOutcome {
        ok: true,
        mensaje: "Metadata corregida correctamente".to_owned(),
        cambios,
    })
}

async fn load_sample_for_correction(pool: &PgPool, sample_id: i32) -> Result<SampleRow, AppError> {
    let row = sqlx::query!(
        r#"SELECT
            id,
            id_corto,
            titulo AS "titulo!",
            bpm,
            key,
            escala,
            COALESCE(metadata, '{}'::jsonb) AS "metadata!: serde_json::Value"
         FROM samples
         WHERE id = $1 AND eliminado_en IS NULL
         LIMIT 1"#,
        sample_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("sample {sample_id}")))?;

    Ok(SampleRow {
        id: row.id,
        id_corto: row.id_corto,
        titulo: row.titulo,
        bpm: row.bpm,
        key: row.key,
        escala: row.escala,
        metadata: row.metadata,
    })
}

fn map_ia_error(error: &AudioIaServiceError) -> AppError {
    match error {
        AudioIaServiceError::MissingProviders
        | AudioIaServiceError::ProviderInitialization { .. } => AppError::ExternalService {
            service: "ia".to_owned(),
            message: "IA no configurada en el servidor (define GROQ_API o OPENAI_API_KEY)"
                .to_owned(),
        },
        AudioIaServiceError::Exhausted { .. } => AppError::ExternalService {
            service: "ia".to_owned(),
            message: format!(
                "Error al procesar la correccion con IA: {}",
                error.summary()
            ),
        },
    }
}

fn merge_corrected_metadata(
    original: &Value,
    corrected: &AudioCreativeMetadata,
    instructions: &str,
) -> Value {
    let mut corrected_value = serde_json::to_value(corrected).unwrap_or_else(|_| json!({}));
    /* `corrected` es un struct, asi que serializa a objeto. Si algun dia no lo
     * fuera se devuelve tal cual, sin los campos preservados, en vez de reventar. */
    let Some(corrected_object) = corrected_value.as_object_mut() else {
        return corrected_value;
    };

    /* Preservar campos del pipeline que la IA no genera. */
    if let Some(original_obj) = original.as_object() {
        for field in PRESERVED_PIPELINE_FIELDS {
            if let Some(value) = original_obj.get(*field) {
                corrected_object.insert((*field).to_owned(), value.clone());
            }
        }
    }

    corrected_object.insert(
        "corregido_at".to_owned(),
        Value::String(Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()),
    );
    corrected_object.insert(
        "corregido_instrucciones".to_owned(),
        Value::String(instructions.to_owned()),
    );

    corrected_value
}

fn pick_sample_type(raw: &str) -> String {
    let normalized = raw.trim().to_ascii_lowercase().replace(['-', ' '], "");
    if VALID_SAMPLE_TYPES.contains(&normalized.as_str()) {
        normalized
    } else {
        "oneshot".to_owned()
    }
}

fn compute_new_title_and_slug(
    nombre_archivo_base: &str,
    bpm: Option<i32>,
    key: Option<&str>,
    escala: Option<&str>,
    id_corto: Option<&str>,
    sample_id: i32,
) -> Option<(String, String)> {
    let base = nombre_archivo_base.trim();
    if base.is_empty() {
        return None;
    }

    let mut titulo = title_case(base);
    if let Some(value) = bpm {
        titulo = format!("{titulo} {value}bpm");
    }
    if let Some(value) = key.filter(|s| !s.is_empty()) {
        let mut key_str = value.to_owned();
        if matches!(escala, Some("menor")) {
            key_str.push('m');
        }
        titulo = format!("{titulo} {key_str}");
    }

    let id_short = id_corto
        .filter(|s| !s.is_empty())
        .map_or_else(|| short_id_from_sample(sample_id), ToOwned::to_owned);

    let slug = format!("{}-{}", slugify(&titulo), id_short);
    Some((titulo, slug))
}

fn pick_new_tags(metadata: &AudioCreativeMetadata) -> Option<Vec<String>> {
    let source = if !metadata.tags_es.is_empty() {
        &metadata.tags_es
    } else if !metadata.tags.is_empty() {
        &metadata.tags
    } else {
        return None;
    };

    let normalized: Vec<String> = source
        .iter()
        .map(|tag| tag.trim().to_ascii_lowercase())
        .filter(|tag| !tag.is_empty())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn pick_new_description(metadata: &AudioCreativeMetadata) -> Option<String> {
    let candidate = if !metadata.descripcion_es.is_empty() {
        &metadata.descripcion_es
    } else if !metadata.descripcion.is_empty() {
        &metadata.descripcion
    } else {
        return None;
    };

    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

/// Campos que la corrección IA reescribe en `samples`; agrupados para no alargar la firma.
struct CambiosCorreccion<'a> {
    metadata: &'a Value,
    new_type: &'a str,
    title_and_slug: Option<&'a (String, String)>,
    new_tags: Option<&'a [String]>,
    new_description: Option<&'a str>,
}

async fn apply_correction_update(
    pool: &PgPool,
    sample_id: i32,
    cambios: CambiosCorreccion<'_>,
) -> Result<(), AppError> {
    let CambiosCorreccion {
        metadata,
        new_type,
        title_and_slug,
        new_tags,
        new_description,
    } = cambios;
    /* [254A-8a lesson] Usar push_bind_unseparated tras push("col = ") para
     * que el separador (", ") solo aparezca entre asignaciones, no entre
     * "col = " y su bind. */
    let mut builder = QueryBuilder::<Postgres>::new("UPDATE samples SET ");
    {
        let mut separated = builder.separated(", ");

        let metadata_json = serde_json::to_string(metadata)
            .map_err(|error| AppError::Internal(format!("serializar metadata: {error}")))?;
        separated.push("metadata = ");
        separated.push_bind_unseparated(metadata_json);
        separated.push_unseparated("::jsonb");

        separated.push("tipo = ");
        separated.push_bind_unseparated(new_type.to_owned());

        if let Some((titulo, slug)) = title_and_slug {
            separated.push("titulo = ");
            separated.push_bind_unseparated(titulo.clone());
            separated.push("slug = ");
            separated.push_bind_unseparated(slug.clone());
        }

        if let Some(tags) = new_tags {
            separated.push("tags = ");
            separated.push_bind_unseparated(tags.to_vec());
            separated.push_unseparated("::text[]");
        }

        if let Some(descripcion) = new_description {
            separated.push("descripcion = ");
            separated.push_bind_unseparated(descripcion.to_owned());
        }

        separated.push("updated_at = NOW()");
    }

    builder.push(" WHERE id = ");
    builder.push_bind(sample_id);

    builder.build().execute(pool).await?;
    Ok(())
}

fn title_case(input: &str) -> String {
    input
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    let rest: String = chars.collect();
                    format!("{}{}", first.to_uppercase(), rest.to_lowercase())
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn slugify(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut last_dash = true;
    for ch in input.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            result.push(ch);
            last_dash = false;
        } else if !last_dash {
            result.push('-');
            last_dash = true;
        }
    }
    while result.ends_with('-') {
        result.pop();
    }
    result
}

fn short_id_from_sample(sample_id: i32) -> String {
    let hex = format!("{sample_id:x}");
    if hex.len() >= 7 {
        hex[..7].to_owned()
    } else {
        format!("{hex:0>7}")
    }
}
