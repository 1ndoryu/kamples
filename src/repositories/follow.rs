use sqlx::PgPool;

use crate::errors::AppError;

/* [174A-60] FollowRepository — port de FollowsRepository.php (seguir,
 * dejarDeSeguir, actualizarContadores, estaSiguiendo, idsSeguidos).
 *
 * Tabla `follows(seguidor_id, seguido_id, created_at)` PK compuesta.
 * Idempotente: ON CONFLICT DO NOTHING. */

pub struct FollowRepository;

impl FollowRepository {
    pub async fn follow(pool: &PgPool, follower_id: i32, target_id: i32) -> Result<(), AppError> {
        sqlx::query!(
            "INSERT INTO follows (seguidor_id, seguido_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            follower_id,
            target_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn unfollow(pool: &PgPool, follower_id: i32, target_id: i32) -> Result<(), AppError> {
        sqlx::query!(
            "DELETE FROM follows WHERE seguidor_id = $1 AND seguido_id = $2",
            follower_id,
            target_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Recalcula `total_seguidores` del target y `total_seguidos` del follower.
    pub async fn recount(pool: &PgPool, follower_id: i32, target_id: i32) -> Result<(), AppError> {
        sqlx::query!(
            "UPDATE usuarios_ext SET total_seguidores = \
             (SELECT COUNT(*) FROM follows WHERE seguido_id = $1) WHERE id = $1",
            target_id
        )
        .execute(pool)
        .await?;
        sqlx::query!(
            "UPDATE usuarios_ext SET total_seguidos = \
             (SELECT COUNT(*) FROM follows WHERE seguidor_id = $1) WHERE id = $1",
            follower_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn is_following(
        pool: &PgPool,
        follower_id: i32,
        target_id: i32,
    ) -> Result<bool, AppError> {
        let r = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM follows WHERE seguidor_id = $1 AND seguido_id = $2) AS \"e!\"",
            follower_id,
            target_id,
        )
        .fetch_one(pool)
        .await?;
        Ok(r)
    }

    pub async fn user_exists(pool: &PgPool, user_id: i32) -> Result<bool, AppError> {
        let r = sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM usuarios_ext WHERE id = $1) AS \"e!\"",
            user_id
        )
        .fetch_one(pool)
        .await?;
        Ok(r)
    }

    /* [274A-16] IDs de usuarios que el `follower_id` sigue. Migrado desde
     * FollowsRepository::idsSeguidos (PHP). */
    pub async fn ids_seguidos(pool: &PgPool, follower_id: i32) -> Result<Vec<i32>, AppError> {
        let ids = sqlx::query_scalar!(
            "SELECT seguido_id FROM follows WHERE seguidor_id = $1",
            follower_id
        )
        .fetch_all(pool)
        .await?;
        Ok(ids)
    }

    /* [296A-3] IDs de usuarios que SIGUEN al target (seguidores).
     * Necesario para endpoint GET /usuarios/{username}/seguidores. */
    pub async fn ids_seguidores(
        pool: &PgPool,
        target_id: i32,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<i32>, AppError> {
        let ids = sqlx::query_scalar!(
            "SELECT seguidor_id FROM follows WHERE seguido_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
            target_id,
            limit,
            offset
        )
        .fetch_all(pool)
        .await?;
        Ok(ids)
    }
}
