use sqlx::PgPool;

use crate::errors::AppError;

/* [174A-60] BlockRepository — port de BloqueosRepository.php (bloquear,
 * desbloquear, listarBloqueados, estaBloqueado).
 *
 * Tabla `bloqueos(id, bloqueador_id, bloqueado_id, razon, created_at)`
 * con UNIQUE(bloqueador_id, bloqueado_id) y CHECK no-autobloqueo. */

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct BlockedUser {
    pub id: i32,
    pub bloqueado_id: i32,
    pub username: Option<String>,
    pub razon: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct BlockRepository;

impl BlockRepository {
    pub async fn block(
        pool: &PgPool,
        blocker_id: i32,
        target_id: i32,
        razon: &str,
    ) -> Result<(), AppError> {
        sqlx::query!(
            "INSERT INTO bloqueos (bloqueador_id, bloqueado_id, razon) VALUES ($1, $2, $3) \
             ON CONFLICT (bloqueador_id, bloqueado_id) DO UPDATE SET razon = EXCLUDED.razon",
            blocker_id,
            target_id,
            razon,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn unblock(pool: &PgPool, blocker_id: i32, target_id: i32) -> Result<(), AppError> {
        sqlx::query!(
            "DELETE FROM bloqueos WHERE bloqueador_id = $1 AND bloqueado_id = $2",
            blocker_id,
            target_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn list(pool: &PgPool, blocker_id: i32) -> Result<Vec<BlockedUser>, AppError> {
        let rows = sqlx::query_as!(
            BlockedUser,
            "SELECT b.id, b.bloqueado_id, u.username AS \"username?\", b.razon, b.created_at AS \"created_at!\" \
             FROM bloqueos b \
             LEFT JOIN usuarios_ext u ON u.id = b.bloqueado_id \
             WHERE b.bloqueador_id = $1 \
             ORDER BY b.created_at DESC",
            blocker_id
        )
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }
}
