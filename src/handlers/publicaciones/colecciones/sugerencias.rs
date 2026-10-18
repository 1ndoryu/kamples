use super::*;

/* [274A-9] GET /api/colecciones/:id/combinacion-pendiente
 * El feature de combinar+undo (legacy `combinar` / `deshacer-combinacion` /
 * tabla undo_combinaciones) NO esta portado a Rust todavia. Por contrato
 * estable, retornamos siempre `hayCombinacion: false` para colecciones que
 * el usuario puede ver. Esto evita 404 en cada carga del detalle.
 *
 * Cuando se implemente combinar/deshacer, este handler debera consultar la
 * tabla de undos pendientes y poblar undoId/origenNombre/combinadoEn/expiraEn. */
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CombinacionPendienteResponse {
    #[serde(rename = "hayCombinacion")]
    pub hay_combinacion: bool,
    #[serde(rename = "undoId", skip_serializing_if = "Option::is_none")]
    pub undo_id: Option<i64>,
    #[serde(rename = "origenNombre", skip_serializing_if = "Option::is_none")]
    pub origen_nombre: Option<String>,
    #[serde(rename = "combinadoEn", skip_serializing_if = "Option::is_none")]
    pub combinado_en: Option<String>,
    #[serde(rename = "expiraEn", skip_serializing_if = "Option::is_none")]
    pub expira_en: Option<String>,
}

#[utoipa::path(
    get, path = "/api/colecciones/{id}/combinacion-pendiente", tag = "colecciones",
    params(("id" = i64, Path, description = "ID de la coleccion")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, body = CombinacionPendienteResponse),
        (status = 403, description = "Privada y no es del usuario"),
        (status = 404, description = "No encontrada"),
    )
)]
pub async fn combinacion_pendiente(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Path(id): Path<i64>,
) -> Result<Json<CombinacionPendienteResponse>, AppError> {
    let col = ColeccionesRepository::fetch(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("coleccion {id} no existe")))?;
    let viewer_id = user.as_ref().map(|u| u.user_id);
    if !col.publica && viewer_id != Some(col.usuario_id) {
        return Err(AppError::Forbidden("coleccion privada".into()));
    }
    Ok(Json(CombinacionPendienteResponse {
        hay_combinacion: false,
        undo_id: None,
        origen_nombre: None,
        combinado_en: None,
        expira_en: None,
    }))
}

/* [274A-10] GET /api/colecciones/:id/sugerencias
 * "Mas Ideas" para una coleccion: scoring agregado por tags+BPM+key de los
 * samples ya incluidos, excluyendo esos mismos. Reusa el motor de
 * SampleRepository::find_samples_by_aggregated_scoring (creado en 274A-6)
 * y el agregador de contexto del handler de sugerencias de descargas. */
#[derive(Debug, Clone, Deserialize, IntoParams, Default)]
#[into_params(parameter_in = Query)]
pub struct ColeccionSugerenciasQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ColeccionSugerenciasResponse {
    pub data: Vec<SampleSummary>,
}

const SUGERENCIAS_DEFAULT_BPM: i32 = 120;
const SUGERENCIAS_TOP_TAGS: usize = 10;
const SUGERENCIAS_MAX_PER_PAGE: i64 = 50;

#[utoipa::path(
    get, path = "/api/colecciones/{id}/sugerencias", tag = "colecciones",
    params(
        ("id" = i64, Path, description = "ID de la coleccion"),
        ColeccionSugerenciasQuery,
    ),
    responses(
        (status = 200, body = ColeccionSugerenciasResponse),
        (status = 403, description = "Privada y no es del usuario"),
        (status = 404, description = "No encontrada"),
    )
)]
pub async fn sugerencias_coleccion(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Path(id): Path<i64>,
    Query(q): Query<ColeccionSugerenciasQuery>,
) -> Result<Json<ColeccionSugerenciasResponse>, AppError> {
    let col = ColeccionesRepository::fetch(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("coleccion {id} no existe")))?;
    let viewer_id = user.as_ref().map(|u| u.user_id);
    if !col.publica && viewer_id != Some(col.usuario_id) {
        return Err(AppError::Forbidden("coleccion privada".into()));
    }

    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(20).clamp(1, SUGERENCIAS_MAX_PER_PAGE);
    let offset = (page - 1) * per_page;

    let contexto = ColeccionesRepository::contexto_muestras(&state.pool, id).await?;
    if contexto.is_empty() {
        return Ok(Json(ColeccionSugerenciasResponse { data: Vec::new() }));
    }

    let exclude = ColeccionesRepository::ids_muestras(&state.pool, id).await?;
    let (top_tags, avg_bpm, dominant_key) = aggregate_coleccion_context(&contexto);

    let criterio = CriterioScoring {
        top_tags: &top_tags,
        avg_bpm,
        dominant_key: dominant_key.as_deref(),
        exclude_ids: &exclude,
    };
    let records = SampleRepository::find_samples_by_aggregated_scoring(
        &state.pool,
        criterio,
        per_page,
        offset,
        viewer_id,
    )
    .await?;

    let public_base = state.public_base_url.as_deref();
    let data: Vec<SampleSummary> = records
        .into_iter()
        .map(|r| build_sample_summary(r, public_base))
        .collect();

    Ok(Json(ColeccionSugerenciasResponse { data }))
}

fn aggregate_coleccion_context(
    contexto: &[SampleContextRow],
) -> (Vec<String>, i32, Option<String>) {
    use std::collections::HashMap;
    let mut tag_counts: HashMap<&str, usize> = HashMap::new();
    let mut bpm_sum: i64 = 0;
    let mut bpm_n: i64 = 0;
    let mut key_counts: HashMap<&str, usize> = HashMap::new();

    for row in contexto {
        for tag in &row.tags {
            *tag_counts.entry(tag.as_str()).or_insert(0) += 1;
        }
        if let Some(b) = row.bpm {
            bpm_sum += i64::from(b);
            bpm_n += 1;
        }
        if let Some(k) = &row.music_key {
            if !k.is_empty() {
                *key_counts.entry(k.as_str()).or_insert(0) += 1;
            }
        }
    }

    let mut tag_vec: Vec<(&str, usize)> = tag_counts.into_iter().collect();
    tag_vec.sort_by_key(|a| std::cmp::Reverse(a.1));
    let top_tags: Vec<String> = tag_vec
        .into_iter()
        .take(SUGERENCIAS_TOP_TAGS)
        .map(|(t, _)| t.to_string())
        .collect();

    let avg_bpm = if bpm_n > 0 {
        i32::try_from(bpm_sum / bpm_n).unwrap_or(SUGERENCIAS_DEFAULT_BPM)
    } else {
        SUGERENCIAS_DEFAULT_BPM
    };

    let dominant_key = key_counts
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(k, _)| k.to_string());

    (top_tags, avg_bpm, dominant_key)
}
