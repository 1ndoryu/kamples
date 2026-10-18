use sqlx::PgPool;

use crate::errors::AppError;
use crate::models::{CreateRelationRequest, CreateSongRequest, SampleRelationSource};
use crate::repositories::{
    ContribucionModeracion, ContribucionesRepository, CrearContribucionRecord, MusicRepository,
};

use super::{
    build_timings_patch, ensure_no_pending_relation_proposal, ensure_relation_exists,
    extract_timings, extract_youtube_id, non_empty, normalize_element_type,
    normalize_new_song_side, normalize_relation_type, parse_element_type, parse_relation_type,
    sanitize_edit_changes, trim_optional, update_relation_from_changes, ContribucionesService,
    CrearContribucionInput, ModerarContribucionInput, ModerarContribucionOutput,
    ProponerEdicionInput, ProponerEliminacionInput,
};

impl ContribucionesService {
    pub async fn crear(pool: &PgPool, input: CrearContribucionInput) -> Result<i32, AppError> {
        let tipo_relacion = normalize_relation_type(&input.tipo_relacion)?;
        let tipo_elemento = normalize_element_type(input.tipo_elemento.as_deref())?;
        let nueva_titulo = trim_optional(input.cancion_nueva_titulo);
        let nueva_artista = trim_optional(input.cancion_nueva_artista);
        let nueva_url = trim_optional(input.cancion_nueva_youtube_url);
        let nueva_lado = normalize_new_song_side(input.cancion_nueva_lado.as_deref())?;

        let hay_ambas_existentes =
            input.cancion_destino_id.is_some() && input.cancion_fuente_id.is_some();
        if !hay_ambas_existentes && nueva_titulo.is_none() {
            return Err(AppError::Validation("Faltan datos de canciones.".into()));
        }
        if nueva_titulo.is_some() && nueva_artista.is_none() {
            return Err(AppError::Validation(
                "Falta artista para la cancion nueva.".into(),
            ));
        }

        if let (Some(destino_id), Some(fuente_id)) =
            (input.cancion_destino_id, input.cancion_fuente_id)
        {
            if ContribucionesRepository::existe_duplicado_nueva(
                pool,
                destino_id,
                fuente_id,
                &tipo_relacion,
            )
            .await?
            {
                return Err(AppError::Conflict(
                    "Ya existe una contribucion pendiente para esta relacion.".into(),
                ));
            }
            if ContribucionesRepository::existe_relacion(
                pool,
                destino_id,
                fuente_id,
                &tipo_relacion,
            )
            .await?
            {
                return Err(AppError::Conflict(
                    "Esta relacion ya esta registrada.".into(),
                ));
            }
        }

        let cambios_propuestos = build_timings_patch(input.timing_fuente, input.timing_destino);
        ContribucionesRepository::crear_nueva(
            pool,
            CrearContribucionRecord {
                contribuidor_id: input.contribuidor_id,
                cancion_destino_id: input.cancion_destino_id,
                cancion_fuente_id: input.cancion_fuente_id,
                cancion_nueva_titulo: nueva_titulo,
                cancion_nueva_artista: nueva_artista,
                cancion_nueva_youtube_url: nueva_url,
                cancion_nueva_lado: nueva_lado,
                tipo_relacion,
                tipo_elemento,
                cambios_propuestos,
            },
        )
        .await
    }

    pub async fn proponer_edicion(
        pool: &PgPool,
        input: ProponerEdicionInput,
    ) -> Result<i32, AppError> {
        ensure_relation_exists(pool, input.relacion_id).await?;
        ensure_no_pending_relation_proposal(pool, input.relacion_id, input.contribuidor_id).await?;
        let cambios = sanitize_edit_changes(&input.cambios)?;
        ContribucionesRepository::crear_edicion(
            pool,
            input.contribuidor_id,
            input.relacion_id,
            cambios,
        )
        .await
    }

    pub async fn proponer_eliminacion(
        pool: &PgPool,
        input: ProponerEliminacionInput,
    ) -> Result<i32, AppError> {
        let razon = input.razon.trim().to_string();
        if razon.chars().count() < 10 {
            return Err(AppError::Validation(
                "La razon debe tener al menos 10 caracteres.".into(),
            ));
        }
        ensure_relation_exists(pool, input.relacion_id).await?;
        ensure_no_pending_relation_proposal(pool, input.relacion_id, input.contribuidor_id).await?;
        ContribucionesRepository::crear_eliminacion(
            pool,
            input.contribuidor_id,
            input.relacion_id,
            razon,
        )
        .await
    }

    pub async fn moderar(
        pool: &PgPool,
        input: ModerarContribucionInput,
    ) -> Result<ModerarContribucionOutput, AppError> {
        let contribucion = ContribucionesRepository::buscar_para_moderar(pool, input.id)
            .await?
            .ok_or_else(|| AppError::NotFound("Contribucion no encontrada.".into()))?;
        if contribucion.estado != "pendiente" {
            return Err(AppError::Conflict(
                "Esta contribucion ya fue moderada.".into(),
            ));
        }
        match input.accion.as_str() {
            "rechazada" => {
                ContribucionesRepository::marcar_moderada(
                    pool,
                    input.id,
                    "rechazada",
                    input.moderador_id,
                    input.nota,
                    None,
                )
                .await?;
                Ok(ModerarContribucionOutput { relacion_id: None })
            }
            "aprobada" => {
                let relacion_id = match contribucion.tipo_contribucion.as_deref().unwrap_or("nueva")
                {
                    "edicion" => {
                        Self::aplicar_edicion(pool, &contribucion).await?;
                        None
                    }
                    "eliminacion" => {
                        Self::aplicar_eliminacion(pool, &contribucion).await?;
                        None
                    }
                    _ => Some(Self::aprobar_nueva(pool, &contribucion).await?),
                };
                ContribucionesRepository::marcar_moderada(
                    pool,
                    input.id,
                    "aprobada",
                    input.moderador_id,
                    input.nota,
                    relacion_id,
                )
                .await?;
                Ok(ModerarContribucionOutput { relacion_id })
            }
            _ => Err(AppError::Validation(
                "Accion de moderacion invalida.".into(),
            )),
        }
    }

    async fn aprobar_nueva(
        pool: &PgPool,
        contribucion: &ContribucionModeracion,
    ) -> Result<i32, AppError> {
        let mut destino_id = contribucion.cancion_destino_id;
        let mut fuente_id = contribucion.cancion_fuente_id;

        if let Some(titulo) = contribucion
            .cancion_nueva_titulo
            .as_deref()
            .and_then(non_empty)
        {
            let artista = contribucion
                .cancion_nueva_artista
                .as_deref()
                .and_then(non_empty)
                .ok_or_else(|| {
                    AppError::Validation("Falta artista para la cancion nueva.".into())
                })?;
            let artista_id =
                ContribucionesRepository::upsert_artista_por_nombre(pool, artista).await?;
            let cancion_id = MusicRepository::create_song(
                pool,
                &CreateSongRequest {
                    titulo: titulo.to_string(),
                    slug: None,
                    artista_id,
                    album: None,
                    sello: None,
                    anio: None,
                    duracion_segundos: None,
                    genero: None,
                    youtube_id: contribucion
                        .cancion_nueva_youtube_url
                        .as_deref()
                        .and_then(extract_youtube_id),
                    spotify_id: None,
                    imagen_url: None,
                    whosampled_url: None,
                    bpm: None,
                    tonalidad: None,
                    metadata: None,
                    artistas: Vec::new(),
                },
            )
            .await?;
            if contribucion.cancion_nueva_lado.as_deref() == Some("destino") {
                destino_id = Some(cancion_id);
            } else {
                fuente_id = Some(cancion_id);
            }
        }

        let destino_id =
            destino_id.ok_or_else(|| AppError::Validation("Falta cancion destino.".into()))?;
        let fuente_id =
            fuente_id.ok_or_else(|| AppError::Validation("Falta cancion fuente.".into()))?;
        let (timings_fuente, timings_destino) =
            extract_timings(contribucion.cambios_propuestos.as_ref());
        MusicRepository::create_relation(
            pool,
            &CreateRelationRequest {
                cancion_destino_id: destino_id,
                cancion_fuente_id: fuente_id,
                whosampled_id: None,
                tipo_relacion: parse_relation_type(contribucion.tipo_relacion.as_deref())?,
                tipo_elemento: Some(parse_element_type(contribucion.tipo_elemento.as_deref())?),
                timings_destino,
                timings_fuente,
                aparece_en_todo: Some(false),
                sample_id: None,
                sample_fuente_id: None,
                sample_destino_id: None,
                votos_total: Some(0),
                votos_promedio: Some(0.0),
                fuente: Some(SampleRelationSource::Comunidad),
                contribuidor_id: Some(contribucion.contribuidor_id),
                verificada: Some(false),
            },
        )
        .await
    }

    async fn aplicar_edicion(
        pool: &PgPool,
        contribucion: &ContribucionModeracion,
    ) -> Result<(), AppError> {
        let relacion_id = contribucion.relacion_existente_id.ok_or_else(|| {
            AppError::Validation("Contribucion de edicion sin relacion_existente_id.".into())
        })?;
        ensure_relation_exists(pool, relacion_id).await?;
        let cambios = contribucion.cambios_propuestos.clone().ok_or_else(|| {
            AppError::Validation("Cambios propuestos vacios o malformados.".into())
        })?;
        let update = update_relation_from_changes(&cambios)?;
        let updated = MusicRepository::update_relation(pool, relacion_id, &update).await?;
        if updated {
            Ok(())
        } else {
            Err(AppError::NotFound("Relacion no encontrada.".into()))
        }
    }

    async fn aplicar_eliminacion(
        pool: &PgPool,
        contribucion: &ContribucionModeracion,
    ) -> Result<(), AppError> {
        let relacion_id = contribucion.relacion_existente_id.ok_or_else(|| {
            AppError::Validation("Contribucion de eliminacion sin relacion_existente_id.".into())
        })?;
        let deleted = MusicRepository::delete_relation(pool, relacion_id).await?;
        if deleted {
            Ok(())
        } else {
            Err(AppError::NotFound("Relacion no encontrada.".into()))
        }
    }
}
