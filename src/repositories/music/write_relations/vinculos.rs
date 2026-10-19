use sqlx::PgPool;

use super::super::MusicRepository;
use crate::errors::AppError;
use crate::models::RelationSampleSide;

impl MusicRepository {
    pub async fn link_sample(
        pool: &PgPool,
        relation_id: i32,
        sample_id: i32,
        side: RelationSampleSide,
        owner_id: i32,
    ) -> Result<(), AppError> {
        let mut tx = pool.begin().await?;
        let relation = Self::find_relation_link_context_tx(&mut tx, relation_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("relacion {relation_id}")))?;
        let sample = Self::find_sample_owner_tx(&mut tx, sample_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("sample {sample_id}")))?;

        if sample.creador_id != owner_id {
            return Err(AppError::Forbidden(
                "Solo puedes vincular samples propios".into(),
            ));
        }
        if sample.estado != "activo" {
            return Err(AppError::Conflict(
                "El sample no esta disponible para vincularse".into(),
            ));
        }

        match side {
            RelationSampleSide::Fuente => {
                if relation.sample_fuente_id.is_some() {
                    return Err(AppError::Conflict(
                        "Ese lado de la relacion ya tiene un sample vinculado".into(),
                    ));
                }
                sqlx::query!(
                    r#"UPDATE relaciones_sample SET sample_fuente_id = $2, updated_at = NOW() WHERE id = $1"#,
                    relation_id,
                    sample_id,
                )
                .execute(&mut *tx)
                .await?;
                sqlx::query!(
                    r#"UPDATE samples
                       SET relacion_sampleo_id = $2,
                           cancion_origen_id = $3,
                           updated_at = NOW()
                       WHERE id = $1"#,
                    sample_id,
                    relation_id,
                    relation.cancion_fuente_id,
                )
                .execute(&mut *tx)
                .await?;
            }
            RelationSampleSide::Destino => {
                if relation.sample_destino_id.is_some() {
                    return Err(AppError::Conflict(
                        "Ese lado de la relacion ya tiene un sample vinculado".into(),
                    ));
                }
                sqlx::query!(
                    r#"UPDATE relaciones_sample SET sample_destino_id = $2, updated_at = NOW() WHERE id = $1"#,
                    relation_id,
                    sample_id,
                )
                .execute(&mut *tx)
                .await?;
                sqlx::query!(
                    r#"UPDATE samples
                       SET relacion_sampleo_id = $2,
                           cancion_origen_id = $3,
                           updated_at = NOW()
                       WHERE id = $1"#,
                    sample_id,
                    relation_id,
                    relation.cancion_destino_id,
                )
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn unlink_sample(
        pool: &PgPool,
        relation_id: i32,
        side: RelationSampleSide,
        owner_id: i32,
    ) -> Result<(), AppError> {
        let mut tx = pool.begin().await?;
        let relation = Self::find_relation_link_context_tx(&mut tx, relation_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("relacion {relation_id}")))?;

        let (sample_id, song_origin_id) = match side {
            RelationSampleSide::Fuente => (relation.sample_fuente_id, relation.cancion_fuente_id),
            RelationSampleSide::Destino => {
                (relation.sample_destino_id, relation.cancion_destino_id)
            }
        };
        let Some(sample_id) = sample_id else {
            return Ok(());
        };

        let sample = Self::find_sample_owner_tx(&mut tx, sample_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("sample {sample_id}")))?;

        if sample.creador_id != owner_id {
            return Err(AppError::Forbidden(
                "Solo puedes desvincular samples propios".into(),
            ));
        }

        match side {
            RelationSampleSide::Fuente => {
                sqlx::query!(
                    r#"UPDATE relaciones_sample SET sample_fuente_id = NULL, updated_at = NOW() WHERE id = $1"#,
                    relation_id,
                )
                .execute(&mut *tx)
                .await?;
            }
            RelationSampleSide::Destino => {
                sqlx::query!(
                    r#"UPDATE relaciones_sample SET sample_destino_id = NULL, updated_at = NOW() WHERE id = $1"#,
                    relation_id,
                )
                .execute(&mut *tx)
                .await?;
            }
        }

        if sample.relacion_sampleo_id == Some(relation_id) {
            let clear_song_origin = if sample.cancion_origen_id == Some(song_origin_id) {
                None
            } else {
                sample.cancion_origen_id
            };

            sqlx::query!(
                r#"UPDATE samples
                   SET relacion_sampleo_id = NULL,
                       cancion_origen_id = $2,
                       updated_at = NOW()
                   WHERE id = $1"#,
                sample_id,
                clear_song_origin,
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn verify_relation(
        pool: &PgPool,
        relation_id: i32,
        verificada: bool,
    ) -> Result<bool, AppError> {
        let updated = sqlx::query_scalar!(
            r#"UPDATE relaciones_sample
               SET verificada = $2,
                   updated_at = NOW()
               WHERE id = $1
               RETURNING id AS "id!""#,
            relation_id,
            verificada,
        )
        .fetch_optional(pool)
        .await?;
        Ok(updated.is_some())
    }
}
