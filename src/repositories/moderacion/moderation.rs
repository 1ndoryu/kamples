use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;

use crate::errors::AppError;

/* [174A-25] Repositorio de moderacion: suspension/eliminacion + bloqueos. */

pub struct ModerationRepository;

impl ModerationRepository {
    pub async fn suspend(
        pool: &PgPool,
        user_id: i32,
        razon: &str,
        hasta: Option<DateTime<Utc>>,
    ) -> Result<(), AppError> {
        /* [Fase3-sqlx] Convertido a query! para validacion compile-time. */
        let res = sqlx::query!(
            "UPDATE usuarios_ext SET estado = 'suspendido', suspendido_hasta = $2, suspension_razon = $3 \
             WHERE id = $1 AND estado != 'en_eliminacion'",
            user_id, hasta, razon
        )
        .execute(pool).await?;
        if res.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("usuario {user_id}")));
        }
        Ok(())
    }

    pub async fn activate(pool: &PgPool, user_id: i32) -> Result<(), AppError> {
        /* [Fase3-sqlx] Convertido a query! para validacion compile-time. */
        let res = sqlx::query!(
            "UPDATE usuarios_ext SET estado = 'activo', suspendido_hasta = NULL, \
             suspension_razon = NULL, marcado_eliminacion_en = NULL, sera_eliminado_en = NULL \
             WHERE id = $1",
            user_id
        )
        .execute(pool)
        .await?;
        if res.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("usuario {user_id}")));
        }
        Ok(())
    }

    pub async fn mark_for_deletion(
        pool: &PgPool,
        user_id: i32,
        dias_gracia: i32,
    ) -> Result<(), AppError> {
        let now = Utc::now();
        let sera = now + Duration::days(i64::from(dias_gracia));
        /* [Fase3-sqlx] Convertido a query! para validacion compile-time. */
        let res = sqlx::query!(
            "UPDATE usuarios_ext SET estado = 'en_eliminacion', \
             marcado_eliminacion_en = $2, sera_eliminado_en = $3 WHERE id = $1",
            user_id,
            now,
            sera
        )
        .execute(pool)
        .await?;
        if res.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("usuario {user_id}")));
        }
        Ok(())
    }

    pub async fn block(
        pool: &PgPool,
        bloqueador_id: i32,
        target_id: i32,
        razon: &str,
    ) -> Result<(), AppError> {
        if bloqueador_id == target_id {
            return Err(AppError::BadRequest(
                "No puedes bloquearte a ti mismo".into(),
            ));
        }
        /* [Fase3-sqlx] Convertido a query! para validacion compile-time. */
        sqlx::query!(
            "INSERT INTO bloqueos (bloqueador_id, bloqueado_id, razon) \
             VALUES ($1, $2, $3) ON CONFLICT (bloqueador_id, bloqueado_id) DO UPDATE SET razon = EXCLUDED.razon",
            bloqueador_id, target_id, razon
        )
        .execute(pool).await?;
        Ok(())
    }

    pub async fn unblock(
        pool: &PgPool,
        bloqueador_id: i32,
        target_id: i32,
    ) -> Result<(), AppError> {
        /* [Fase3-sqlx] Convertido a query! para validacion compile-time. */
        sqlx::query!(
            "DELETE FROM bloqueos WHERE bloqueador_id = $1 AND bloqueado_id = $2",
            bloqueador_id,
            target_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn list_blocked(pool: &PgPool, bloqueador_id: i32) -> Result<Vec<i32>, AppError> {
        let ids = sqlx::query_scalar!(
            "SELECT bloqueado_id FROM bloqueos WHERE bloqueador_id = $1 ORDER BY created_at DESC",
            bloqueador_id
        )
        .fetch_all(pool)
        .await?;
        Ok(ids)
    }

    /* [174A-51] Lista de usuarios que bloquearon a `target_id`. Necesario para
     * el filtrado bidireccional del feed: el algoritmo no debe mostrar samples
     * de creadores que han bloqueado al usuario, ni de creadores bloqueados
     * por el usuario. Equivale a `BloqueosRepository::idsBloqueadores` legacy. */
    pub async fn list_blockers(pool: &PgPool, target_id: i32) -> Result<Vec<i32>, AppError> {
        let ids = sqlx::query_scalar!(
            "SELECT bloqueador_id FROM bloqueos WHERE bloqueado_id = $1 ORDER BY created_at DESC",
            target_id
        )
        .fetch_all(pool)
        .await?;
        Ok(ids)
    }
}
