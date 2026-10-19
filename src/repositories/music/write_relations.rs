use sqlx::{PgPool, Postgres, Transaction};

use super::support::{
    dedupe_i32, relation_element_db, relation_source_db, relation_type_db, serialize_json_option,
    RelationLinkContextRecord, SampleOwnerRecord,
};
use super::MusicRepository;
use crate::errors::AppError;
use crate::models::{CreateRelationRequest, SampleRelationType, UpdateRelationRequest};

mod vinculos;

impl MusicRepository {
    pub async fn create_relation(
        pool: &PgPool,
        request: &CreateRelationRequest,
    ) -> Result<i32, AppError> {
        if request.cancion_destino_id == request.cancion_fuente_id {
            return Err(AppError::Validation(
                "Una relacion no puede apuntar a la misma cancion en ambos lados".into(),
            ));
        }

        Self::ensure_songs_exist(
            pool,
            &[request.cancion_destino_id, request.cancion_fuente_id],
        )
        .await?;
        Self::ensure_relation_unique(
            pool,
            request.cancion_destino_id,
            request.cancion_fuente_id,
            request.tipo_relacion,
            None,
        )
        .await?;

        let relation_id = sqlx::query_scalar!(
            r#"INSERT INTO relaciones_sample (
                    cancion_destino_id,
                    cancion_fuente_id,
                    whosampled_id,
                    tipo_relacion,
                    tipo_elemento,
                    timings_destino,
                    timings_fuente,
                    aparece_en_todo,
                    sample_id,
                    sample_fuente_id,
                    sample_destino_id,
                    votos_total,
                    votos_promedio,
                    fuente,
                    contribuidor_id,
                    verificada
               )
               VALUES (
                    $1, $2, $3, $4, $5, COALESCE($6::jsonb, '[]'::jsonb), COALESCE($7::jsonb, '[]'::jsonb),
                    COALESCE($8, FALSE), $9, $10, $11, COALESCE($12, 0), COALESCE(ROUND(CAST($13 AS double precision)::numeric, 2), 0),
                    COALESCE($14, 'comunidad'), $15, COALESCE($16, FALSE)
               )
               RETURNING id AS "id!""#,
            request.cancion_destino_id,
            request.cancion_fuente_id,
            request.whosampled_id,
            relation_type_db(request.tipo_relacion),
            request.tipo_elemento.map(relation_element_db),
            serde_json::to_value(&request.timings_destino)
                .map_err(|error| AppError::Internal(format!("serializar timings_destino: {error}")))?,
            serde_json::to_value(&request.timings_fuente)
                .map_err(|error| AppError::Internal(format!("serializar timings_fuente: {error}")))?,
            request.aparece_en_todo,
            request.sample_id,
            request.sample_fuente_id,
            request.sample_destino_id,
            request.votos_total,
            request.votos_promedio,
            request.fuente.map(relation_source_db),
            request.contribuidor_id,
            request.verificada,
        )
        .fetch_one(pool)
        .await?;

        Self::recount_song_sampling_totals(
            pool,
            &[request.cancion_destino_id, request.cancion_fuente_id],
        )
        .await?;
        Ok(relation_id)
    }

    pub async fn update_relation(
        pool: &PgPool,
        relation_id: i32,
        request: &UpdateRelationRequest,
    ) -> Result<bool, AppError> {
        let existing = Self::find_relation_link_context(pool, relation_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("relacion {relation_id}")))?;
        let current_detail = Self::find_relation_by_id(pool, relation_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("relacion {relation_id}")))?;

        let final_destino = request
            .cancion_destino_id
            .unwrap_or(existing.cancion_destino_id);
        let final_fuente = request
            .cancion_fuente_id
            .unwrap_or(existing.cancion_fuente_id);
        let final_tipo = request
            .tipo_relacion
            .unwrap_or(current_detail.tipo_relacion);

        if final_destino == final_fuente {
            return Err(AppError::Validation(
                "Una relacion no puede apuntar a la misma cancion en ambos lados".into(),
            ));
        }

        Self::ensure_songs_exist(pool, &[final_destino, final_fuente]).await?;
        Self::ensure_relation_unique(
            pool,
            final_destino,
            final_fuente,
            final_tipo,
            Some(relation_id),
        )
        .await?;

        let updated = sqlx::query_scalar!(
            r#"UPDATE relaciones_sample
               SET cancion_destino_id = COALESCE($2, cancion_destino_id),
                   cancion_fuente_id = COALESCE($3, cancion_fuente_id),
                   whosampled_id = COALESCE($4, whosampled_id),
                   tipo_relacion = COALESCE($5, tipo_relacion),
                   tipo_elemento = COALESCE($6, tipo_elemento),
                   timings_destino = COALESCE($7::jsonb, timings_destino),
                   timings_fuente = COALESCE($8::jsonb, timings_fuente),
                   aparece_en_todo = COALESCE($9, aparece_en_todo),
                   sample_id = COALESCE($10, sample_id),
                   sample_fuente_id = COALESCE($11, sample_fuente_id),
                   sample_destino_id = COALESCE($12, sample_destino_id),
                   votos_total = COALESCE($13, votos_total),
                   votos_promedio = COALESCE(ROUND(CAST($14 AS double precision)::numeric, 2), votos_promedio),
                   fuente = COALESCE($15, fuente),
                   contribuidor_id = COALESCE($16, contribuidor_id),
                   verificada = COALESCE($17, verificada),
                   updated_at = NOW()
               WHERE id = $1
               RETURNING id AS "id!""#,
            relation_id,
            request.cancion_destino_id,
            request.cancion_fuente_id,
            request.whosampled_id,
            request.tipo_relacion.map(relation_type_db),
            request.tipo_elemento.map(relation_element_db),
            serialize_json_option(request.timings_destino.as_ref())?,
            serialize_json_option(request.timings_fuente.as_ref())?,
            request.aparece_en_todo,
            request.sample_id,
            request.sample_fuente_id,
            request.sample_destino_id,
            request.votos_total,
            request.votos_promedio,
            request.fuente.map(relation_source_db),
            request.contribuidor_id,
            request.verificada,
        )
        .fetch_optional(pool)
        .await?;

        if updated.is_none() {
            return Ok(false);
        }

        Self::recount_song_sampling_totals(
            pool,
            &[
                existing.cancion_fuente_id,
                existing.cancion_destino_id,
                final_fuente,
                final_destino,
            ],
        )
        .await?;
        Ok(true)
    }

    pub async fn delete_relation(pool: &PgPool, relation_id: i32) -> Result<bool, AppError> {
        let context = Self::find_relation_link_context(pool, relation_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("relacion {relation_id}")))?;

        let mut tx = pool.begin().await?;
        sqlx::query!(
            r#"DELETE FROM likes WHERE tipo = 'relacion' AND target_id = $1"#,
            relation_id
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            r#"DELETE FROM comentarios WHERE tipo = 'relacion' AND target_id = $1"#,
            relation_id,
        )
        .execute(&mut *tx)
        .await?;

        let deleted = sqlx::query!(
            r#"DELETE FROM relaciones_sample WHERE id = $1"#,
            relation_id
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();

        if deleted == 0 {
            return Ok(false);
        }

        Self::recount_song_sampling_totals_tx(
            &mut tx,
            &[context.cancion_fuente_id, context.cancion_destino_id],
        )
        .await?;
        tx.commit().await?;
        Ok(true)
    }
}

impl MusicRepository {
    async fn ensure_songs_exist(pool: &PgPool, song_ids: &[i32]) -> Result<(), AppError> {
        let unique_ids = dedupe_i32(song_ids);
        let found = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!" FROM canciones WHERE id = ANY($1)"#,
            &unique_ids,
        )
        .fetch_one(pool)
        .await?;

        if found != i64::try_from(unique_ids.len()).unwrap_or(i64::MAX) {
            return Err(AppError::Validation(
                "Una o mas canciones no existen".into(),
            ));
        }

        Ok(())
    }

    async fn ensure_relation_unique(
        pool: &PgPool,
        destino_id: i32,
        fuente_id: i32,
        relation_type: SampleRelationType,
        exclude_id: Option<i32>,
    ) -> Result<(), AppError> {
        let exists = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!"
               FROM relaciones_sample
               WHERE cancion_destino_id = $1
                 AND cancion_fuente_id = $2
                 AND tipo_relacion = $3
                 AND ($4::int IS NULL OR id <> $4)"#,
            destino_id,
            fuente_id,
            relation_type_db(relation_type),
            exclude_id,
        )
        .fetch_one(pool)
        .await?;

        if exists > 0 {
            return Err(AppError::Conflict(
                "La relacion ya existe con ese tipo".into(),
            ));
        }

        Ok(())
    }

    async fn recount_song_sampling_totals(pool: &PgPool, song_ids: &[i32]) -> Result<(), AppError> {
        let mut tx = pool.begin().await?;
        Self::recount_song_sampling_totals_tx(&mut tx, song_ids).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn recount_song_sampling_totals_tx(
        tx: &mut Transaction<'_, Postgres>,
        song_ids: &[i32],
    ) -> Result<(), AppError> {
        let unique_ids = dedupe_i32(song_ids);
        if unique_ids.is_empty() {
            return Ok(());
        }

        sqlx::query!(
            r#"UPDATE canciones c
               SET total_samplea = (
                        SELECT COUNT(*)::int
                        FROM relaciones_sample r
                        WHERE r.cancion_destino_id = c.id
                          AND r.tipo_relacion = 'sample'
                   ),
                   total_sampleada = (
                        SELECT COUNT(*)::int
                        FROM relaciones_sample r
                        WHERE r.cancion_fuente_id = c.id
                          AND r.tipo_relacion = 'sample'
                   ),
                   updated_at = NOW()
               WHERE c.id = ANY($1)"#,
            &unique_ids,
        )
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    async fn find_relation_link_context(
        pool: &PgPool,
        relation_id: i32,
    ) -> Result<Option<RelationLinkContextRecord>, AppError> {
        let row = sqlx::query_as!(
            RelationLinkContextRecord,
            r#"SELECT cancion_fuente_id,
                      cancion_destino_id,
                      sample_fuente_id,
                      sample_destino_id
               FROM relaciones_sample
               WHERE id = $1
               LIMIT 1"#,
            relation_id,
        )
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    async fn find_relation_link_context_tx(
        tx: &mut Transaction<'_, Postgres>,
        relation_id: i32,
    ) -> Result<Option<RelationLinkContextRecord>, AppError> {
        let row = sqlx::query_as!(
            RelationLinkContextRecord,
            r#"SELECT cancion_fuente_id,
                      cancion_destino_id,
                      sample_fuente_id,
                      sample_destino_id
               FROM relaciones_sample
               WHERE id = $1
               LIMIT 1"#,
            relation_id,
        )
        .fetch_optional(&mut **tx)
        .await?;
        Ok(row)
    }

    async fn find_sample_owner_tx(
        tx: &mut Transaction<'_, Postgres>,
        sample_id: i32,
    ) -> Result<Option<SampleOwnerRecord>, AppError> {
        let row = sqlx::query_as!(
            SampleOwnerRecord,
            r#"SELECT creador_id,
                      estado AS "estado!",
                      relacion_sampleo_id,
                      cancion_origen_id
               FROM samples
               WHERE id = $1
               LIMIT 1"#,
            sample_id,
        )
        .fetch_optional(&mut **tx)
        .await?;
        Ok(row)
    }
}
