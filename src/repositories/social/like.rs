use sqlx::PgPool;
use std::str::FromStr;

use crate::errors::AppError;

/* [174A-59] Likes polimórficos. Port directo de `LikesRepository.php` +
 * `SocialController::darLike/quitarLike`.
 *
 * Tabla `likes(usuario_id, tipo, target_id, reaccion, created_at)` con
 * UNIQUE(usuario_id, tipo, target_id) → upsert atómico por reacción.
 * Reacciones: 'like', 'dislike', 'encanta'. El contador `total_likes` cuenta
 * solo reacciones positivas (`like` + `encanta`); los dislikes son privados. */

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LikeKind {
    Sample,
    Publicacion,
    Comentario,
    Cancion,
    Relacion,
}

impl LikeKind {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Sample => "sample",
            Self::Publicacion => "publicacion",
            Self::Comentario => "comentario",
            Self::Cancion => "cancion",
            Self::Relacion => "relacion",
        }
    }
}

impl FromStr for LikeKind {
    type Err = AppError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "sample" => Ok(Self::Sample),
            "publicacion" => Ok(Self::Publicacion),
            "comentario" => Ok(Self::Comentario),
            "cancion" => Ok(Self::Cancion),
            "relacion" => Ok(Self::Relacion),
            other => Err(AppError::Validation(format!(
                "tipo de like inválido: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reaction {
    Like,
    Dislike,
    Encanta,
}

impl Reaction {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Like => "like",
            Self::Dislike => "dislike",
            Self::Encanta => "encanta",
        }
    }

    pub fn is_positive(self) -> bool {
        matches!(self, Self::Like | Self::Encanta)
    }
}

impl FromStr for Reaction {
    type Err = AppError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "like" => Ok(Self::Like),
            "dislike" => Ok(Self::Dislike),
            "encanta" => Ok(Self::Encanta),
            other => Err(AppError::Validation(format!("reacción inválida: {other}"))),
        }
    }
}

pub struct LikeRepository;

impl LikeRepository {
    /// Verifica que el target exista antes de aceptar el like.
    pub async fn target_exists(
        pool: &PgPool,
        kind: LikeKind,
        target_id: i32,
    ) -> Result<bool, AppError> {
        let exists = match kind {
            LikeKind::Sample => sqlx::query_scalar!(
                "SELECT EXISTS(SELECT 1 FROM samples WHERE id = $1) AS \"e!\"",
                target_id
            )
            .fetch_one(pool)
            .await?,
            LikeKind::Publicacion => sqlx::query_scalar!(
                "SELECT EXISTS(SELECT 1 FROM publicaciones WHERE id = $1 AND eliminado_en IS NULL) AS \"e!\"",
                target_id
            )
            .fetch_one(pool)
            .await?,
            LikeKind::Comentario => sqlx::query_scalar!(
                "SELECT EXISTS(SELECT 1 FROM comentarios WHERE id = $1 AND (moderacion_estado IS NULL OR moderacion_estado != 'rechazado')) AS \"e!\"",
                target_id
            )
            .fetch_one(pool)
            .await?,
            LikeKind::Cancion => sqlx::query_scalar!(
                "SELECT EXISTS(SELECT 1 FROM canciones WHERE id = $1) AS \"e!\"",
                target_id
            )
            .fetch_one(pool)
            .await?,
            LikeKind::Relacion => sqlx::query_scalar!(
                "SELECT EXISTS(SELECT 1 FROM relaciones_sample WHERE id = $1) AS \"e!\"",
                target_id
            )
            .fetch_one(pool)
            .await?,
        };
        Ok(exists)
    }

    /// Upsert atómico: si el usuario ya reaccionó al target, actualiza la
    /// reacción; si no, la crea. Devuelve `true` siempre (operación idempotente).
    pub async fn upsert_reaction(
        pool: &PgPool,
        user_id: i32,
        kind: LikeKind,
        target_id: i32,
        reaction: Reaction,
    ) -> Result<(), AppError> {
        sqlx::query!(
            "INSERT INTO likes (usuario_id, tipo, target_id, reaccion) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (usuario_id, tipo, target_id) \
             DO UPDATE SET reaccion = EXCLUDED.reaccion, created_at = NOW()",
            user_id,
            kind.as_db_str(),
            target_id,
            reaction.as_db_str(),
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Elimina la reacción del usuario al target (si existía).
    pub async fn delete_reaction(
        pool: &PgPool,
        user_id: i32,
        kind: LikeKind,
        target_id: i32,
    ) -> Result<(), AppError> {
        sqlx::query!(
            "DELETE FROM likes WHERE usuario_id = $1 AND tipo = $2 AND target_id = $3",
            user_id,
            kind.as_db_str(),
            target_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Recalcula `total_likes` en la tabla destino contando reacciones
    /// positivas (`like` + `encanta`). NO cuenta `dislike`.
    pub async fn recount_target(
        pool: &PgPool,
        kind: LikeKind,
        target_id: i32,
    ) -> Result<(), AppError> {
        match kind {
            LikeKind::Sample => {
                sqlx::query!(
                    "UPDATE samples SET total_likes = ( \
                        SELECT COUNT(*) FROM likes \
                        WHERE tipo = 'sample' AND target_id = $1 \
                          AND reaccion IN ('like', 'encanta') \
                    ) WHERE id = $1",
                    target_id,
                )
                .execute(pool)
                .await?;
            }
            LikeKind::Publicacion => {
                sqlx::query!(
                    "UPDATE publicaciones SET total_likes = ( \
                        SELECT COUNT(*) FROM likes \
                        WHERE tipo = 'publicacion' AND target_id = $1 \
                          AND reaccion IN ('like', 'encanta') \
                    ) WHERE id = $1",
                    target_id,
                )
                .execute(pool)
                .await?;
            }
            LikeKind::Comentario => {
                sqlx::query!(
                    "UPDATE comentarios SET total_likes = ( \
                        SELECT COUNT(*) FROM likes \
                        WHERE tipo = 'comentario' AND target_id = $1 \
                          AND reaccion IN ('like', 'encanta') \
                    ) WHERE id = $1",
                    target_id,
                )
                .execute(pool)
                .await?;
            }
            LikeKind::Cancion => {
                sqlx::query!(
                    "UPDATE canciones SET total_likes = ( \
                        SELECT COUNT(*) FROM likes \
                        WHERE tipo = 'cancion' AND target_id = $1 \
                          AND reaccion IN ('like', 'encanta') \
                    ) WHERE id = $1",
                    target_id,
                )
                .execute(pool)
                .await?;
            }
            LikeKind::Relacion => {
                sqlx::query!(
                    "UPDATE relaciones_sample SET total_likes = ( \
                        SELECT COUNT(*) FROM likes \
                        WHERE tipo = 'relacion' AND target_id = $1 \
                          AND reaccion IN ('like', 'encanta') \
                    ) WHERE id = $1",
                    target_id,
                )
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_kinds() {
        assert_eq!(LikeKind::from_str("sample").unwrap(), LikeKind::Sample);
        assert_eq!(
            LikeKind::from_str("publicacion").unwrap(),
            LikeKind::Publicacion
        );
        assert_eq!(
            LikeKind::from_str("comentario").unwrap(),
            LikeKind::Comentario
        );
        assert!(LikeKind::from_str("desconocido").is_err());
    }

    #[test]
    fn parses_reactions() {
        assert_eq!(Reaction::from_str("like").unwrap(), Reaction::Like);
        assert_eq!(Reaction::from_str("encanta").unwrap(), Reaction::Encanta);
        assert!(Reaction::from_str("rage").is_err());
    }

    #[test]
    fn positive_reaction_set() {
        assert!(Reaction::Like.is_positive());
        assert!(Reaction::Encanta.is_positive());
        assert!(!Reaction::Dislike.is_positive());
    }
}
