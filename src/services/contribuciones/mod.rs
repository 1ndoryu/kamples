mod service;

/* [274A-23..26+48] Servicio de contribuciones comunitarias.
 * Replica el flujo legacy sin stubs: valida propuestas, aplica aprobaciones
 * creando/editando/eliminando relaciones reales y marca moderacion atomica. */

use serde_json::{Map, Value};
use sqlx::PgPool;

use crate::errors::AppError;
use crate::models::{SampleRelationElementType, SampleRelationType, UpdateRelationRequest};
use crate::repositories::{ContribucionesRepository, MusicRepository};


pub struct ContribucionesService;

pub struct CrearContribucionInput {
    pub contribuidor_id: i32,
    pub cancion_destino_id: Option<i32>,
    pub cancion_fuente_id: Option<i32>,
    pub cancion_nueva_titulo: Option<String>,
    pub cancion_nueva_artista: Option<String>,
    pub cancion_nueva_youtube_url: Option<String>,
    pub cancion_nueva_lado: Option<String>,
    pub tipo_relacion: String,
    pub tipo_elemento: Option<String>,
    pub timing_fuente: Option<i32>,
    pub timing_destino: Option<i32>,
}

pub struct ProponerEdicionInput {
    pub contribuidor_id: i32,
    pub relacion_id: i32,
    pub cambios: Value,
}

pub struct ProponerEliminacionInput {
    pub contribuidor_id: i32,
    pub relacion_id: i32,
    pub razon: String,
}

pub struct ModerarContribucionInput {
    pub moderador_id: i32,
    pub id: i32,
    pub accion: String,
    pub nota: Option<String>,
}

pub struct ModerarContribucionOutput {
    pub relacion_id: Option<i32>,
}

async fn ensure_relation_exists(pool: &PgPool, relation_id: i32) -> Result<(), AppError> {
    MusicRepository::find_relation_by_id(pool, relation_id)
        .await?
        .map(|_| ())
        .ok_or_else(|| AppError::NotFound("Relacion no encontrada.".into()))
}

async fn ensure_no_pending_relation_proposal(
    pool: &PgPool,
    relation_id: i32,
    contribuidor_id: i32,
) -> Result<(), AppError> {
    if ContribucionesRepository::existe_propuesta_relacion_usuario(
        pool,
        relation_id,
        contribuidor_id,
    )
    .await?
    {
        Err(AppError::Conflict(
            "Ya tienes una propuesta pendiente para esta relacion.".into(),
        ))
    } else {
        Ok(())
    }
}

fn normalize_relation_type(value: &str) -> Result<String, AppError> {
    parse_relation_type(Some(value)).map(|_| value.to_string())
}

fn normalize_element_type(value: Option<&str>) -> Result<String, AppError> {
    let normalized = value.unwrap_or("multiple_elements");
    parse_element_type(Some(normalized)).map(|_| normalized.to_string())
}

fn normalize_new_song_side(value: Option<&str>) -> Result<Option<String>, AppError> {
    match value.and_then(non_empty) {
        Some("destino" | "fuente") => Ok(value.map(str::to_string)),
        Some(_) => Err(AppError::Validation(
            "Lado de cancion nueva invalido.".into(),
        )),
        None => Ok(None),
    }
}

fn parse_relation_type(value: Option<&str>) -> Result<SampleRelationType, AppError> {
    match value.unwrap_or("sample") {
        "sample" => Ok(SampleRelationType::Sample),
        "cover" => Ok(SampleRelationType::Cover),
        "remix" => Ok(SampleRelationType::Remix),
        "interpolation" => Ok(SampleRelationType::Interpolation),
        _ => Err(AppError::Validation("Tipo de relacion invalido.".into())),
    }
}

fn parse_element_type(value: Option<&str>) -> Result<SampleRelationElementType, AppError> {
    match value.unwrap_or("multiple_elements") {
        "hook_riff" => Ok(SampleRelationElementType::HookRiff),
        "vocals_lyrics" => Ok(SampleRelationElementType::VocalsLyrics),
        "drums" => Ok(SampleRelationElementType::Drums),
        "bass" => Ok(SampleRelationElementType::Bass),
        "keys_synth" => Ok(SampleRelationElementType::KeysSynth),
        "sound_effect" => Ok(SampleRelationElementType::SoundEffect),
        "multiple_elements" => Ok(SampleRelationElementType::MultipleElements),
        "other" => Ok(SampleRelationElementType::Other),
        _ => Err(AppError::Validation("Tipo de elemento invalido.".into())),
    }
}

fn build_timings_patch(timing_fuente: Option<i32>, timing_destino: Option<i32>) -> Option<Value> {
    let mut map = Map::new();
    if let Some(value) = timing_fuente.filter(|value| *value >= 0) {
        map.insert(
            "timings_fuente".into(),
            Value::Array(vec![Value::from(value)]),
        );
    }
    if let Some(value) = timing_destino.filter(|value| *value >= 0) {
        map.insert(
            "timings_destino".into(),
            Value::Array(vec![Value::from(value)]),
        );
    }
    (!map.is_empty()).then_some(Value::Object(map))
}

fn sanitize_edit_changes(value: &Value) -> Result<Value, AppError> {
    let object = value
        .as_object()
        .ok_or_else(|| AppError::Validation("Cambios propuestos vacios o malformados.".into()))?;
    let mut out = Map::new();
    if let Some(value) = object.get("tipo_relacion").and_then(Value::as_str) {
        parse_relation_type(Some(value))?;
        out.insert("tipo_relacion".into(), Value::from(value));
    }
    if let Some(value) = object.get("tipo_elemento").and_then(Value::as_str) {
        parse_element_type(Some(value))?;
        out.insert("tipo_elemento".into(), Value::from(value));
    }
    if let Some(value) = object
        .get("razon")
        .and_then(Value::as_str)
        .and_then(non_empty)
    {
        out.insert("razon".into(), Value::from(value));
    }
    if let Some(values) = positive_i32_array(object.get("timings_fuente")) {
        out.insert("timings_fuente".into(), values);
    }
    if let Some(values) = positive_i32_array(object.get("timings_destino")) {
        out.insert("timings_destino".into(), values);
    }
    if let Some(value) = object.get("verificada").and_then(Value::as_bool) {
        out.insert("verificada".into(), Value::from(value));
    }
    if out.is_empty() {
        Err(AppError::Validation(
            "No se enviaron cambios validos.".into(),
        ))
    } else {
        Ok(Value::Object(out))
    }
}

fn update_relation_from_changes(value: &Value) -> Result<UpdateRelationRequest, AppError> {
    let object = value
        .as_object()
        .ok_or_else(|| AppError::Validation("Cambios propuestos vacios o malformados.".into()))?;
    let mut update = UpdateRelationRequest::default();
    if let Some(value) = object.get("tipo_relacion").and_then(Value::as_str) {
        update.tipo_relacion = Some(parse_relation_type(Some(value))?);
    }
    if let Some(value) = object.get("tipo_elemento").and_then(Value::as_str) {
        update.tipo_elemento = Some(parse_element_type(Some(value))?);
    }
    update.timings_fuente = positive_i32_vec(object.get("timings_fuente"));
    update.timings_destino = positive_i32_vec(object.get("timings_destino"));
    update.verificada = object.get("verificada").and_then(Value::as_bool);
    if update.tipo_relacion.is_none()
        && update.tipo_elemento.is_none()
        && update.timings_fuente.is_none()
        && update.timings_destino.is_none()
        && update.verificada.is_none()
    {
        Err(AppError::Validation(
            "Ningun cambio valido para aplicar.".into(),
        ))
    } else {
        Ok(update)
    }
}

fn extract_timings(value: Option<&Value>) -> (Vec<i32>, Vec<i32>) {
    let Some(object) = value.and_then(Value::as_object) else {
        return (Vec::new(), Vec::new());
    };
    (
        positive_i32_vec(object.get("timings_fuente")).unwrap_or_default(),
        positive_i32_vec(object.get("timings_destino")).unwrap_or_default(),
    )
}

fn positive_i32_array(value: Option<&Value>) -> Option<Value> {
    positive_i32_vec(value).map(|items| Value::Array(items.into_iter().map(Value::from).collect()))
}

fn positive_i32_vec(value: Option<&Value>) -> Option<Vec<i32>> {
    let items: Vec<i32> = value?
        .as_array()?
        .iter()
        .filter_map(Value::as_i64)
        .filter_map(|value| i32::try_from(value).ok())
        .collect();
    (!items.is_empty()).then_some(items)
}

fn extract_youtube_id(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.len() == 11
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        return Some(trimmed.to_string());
    }
    for marker in ["v=", "youtu.be/", "embed/"] {
        if let Some(index) = trimmed.find(marker) {
            let start = index + marker.len();
            let candidate: String = trimmed[start..]
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '-')
                .take(11)
                .collect();
            if candidate.len() == 11 {
                return Some(candidate);
            }
        }
    }
    None
}

fn trim_optional(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}
