use sqlx::PgPool;

use crate::errors::AppError;
use crate::models::{UpdateProfileRequest, UserProfile};

/* [174A-24] Repositorio de perfil sobre `usuarios_ext`. */
pub struct ProfileRepository;

impl ProfileRepository {
    /// Perfiles básicos (id, username, nombre visible, avatar) de varios usuarios
    /// en una sola consulta. Se usa para listas de seguidores.
    pub async fn perfiles_basicos(
        pool: &PgPool,
        ids: &[i32],
    ) -> Result<Vec<(i32, String, String, Option<String>)>, sqlx::Error> {
        let rows = sqlx::query!(
            "SELECT id, username, nombre_visible, avatar_url FROM usuarios_ext WHERE id = ANY($1)",
            ids,
        )
        .fetch_all(pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| (r.id, r.username, r.nombre_visible, r.avatar_url))
            .collect())
    }

    pub async fn find_by_id(pool: &PgPool, id: i32) -> Result<Option<UserProfile>, sqlx::Error> {
        sqlx::query_as!(
            UserProfile,
            "SELECT id, username, email, nombre_visible, bio, avatar_url, portada_url, sitio_web,
                    generos_favoritos, plan, rol, verificado, total_seguidores, total_seguidos,
                    total_samples, total_descargas, estado, created_at
             FROM usuarios_ext WHERE id = $1",
            id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_username(
        pool: &PgPool,
        username: &str,
    ) -> Result<Option<UserProfile>, sqlx::Error> {
        sqlx::query_as!(
            UserProfile,
            "SELECT id, username, email, nombre_visible, bio, avatar_url, portada_url, sitio_web,
                    generos_favoritos, plan, rol, verificado, total_seguidores, total_seguidos,
                    total_samples, total_descargas, estado, created_at
             FROM usuarios_ext WHERE username = $1",
            username
        )
        .fetch_optional(pool)
        .await
    }

    /// Aplica patch parcial. Solo actualiza columnas con `Some(_)`: `COALESCE($n, col)`
    /// conserva el valor actual cuando el campo llega vacío, sin SQL dinámico.
    pub async fn update(
        pool: &PgPool,
        user_id: i32,
        patch: &UpdateProfileRequest,
    ) -> Result<UserProfile, AppError> {
        // Sin campos que cambiar no hay UPDATE: se devuelve el perfil tal cual.
        if sin_cambios(patch) {
            return Self::find_by_id(pool, user_id)
                .await?
                .ok_or(AppError::NotFound("Usuario".into()));
        }

        let perfil = sqlx::query_as!(
            UserProfile,
            "UPDATE usuarios_ext SET
                nombre_visible    = COALESCE($1, nombre_visible),
                bio               = COALESCE($2, bio),
                avatar_url        = COALESCE($3, avatar_url),
                portada_url       = COALESCE($4, portada_url),
                sitio_web         = COALESCE($5, sitio_web),
                generos_favoritos = COALESCE($6, generos_favoritos)
             WHERE id = $7
             RETURNING id, username, email, nombre_visible, bio, avatar_url, portada_url, sitio_web,
                       generos_favoritos, plan, rol, verificado, total_seguidores, total_seguidos,
                       total_samples, total_descargas, estado, created_at",
            patch.nombre_visible.as_deref(),
            patch.bio.as_deref(),
            patch.avatar_url.as_deref(),
            patch.portada_url.as_deref(),
            patch.sitio_web.as_deref(),
            patch.generos_favoritos.clone(),
            user_id
        )
        .fetch_one(pool)
        .await?;
        Ok(perfil)
    }
}

/// `true` cuando el patch no trae ningún campo: no hace falta tocar la fila.
fn sin_cambios(patch: &UpdateProfileRequest) -> bool {
    patch.nombre_visible.is_none()
        && patch.bio.is_none()
        && patch.avatar_url.is_none()
        && patch.portada_url.is_none()
        && patch.sitio_web.is_none()
        && patch.generos_favoritos.is_none()
}
