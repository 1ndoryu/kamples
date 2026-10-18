use serde_json::Value as JsonValue;
use sqlx::PgPool;

use crate::errors::AppError;
use crate::models::{UpdateProfileRequest, UserProfile};

/* [174A-24] Repositorio de perfil sobre `usuarios_ext`. */
pub struct ProfileRepository;

const PROFILE_COLS: &str = "id, username, email, nombre_visible, bio, avatar_url, portada_url, \
    sitio_web, generos_favoritos, plan, rol, verificado, total_seguidores, total_seguidos, \
    total_samples, total_descargas, estado, created_at";

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
        let sql = format!("SELECT {PROFILE_COLS} FROM usuarios_ext WHERE id = $1");
        sqlx::query_as::<_, UserProfile>(&sql)
            .bind(id)
            .fetch_optional(pool)
            .await
    }

    pub async fn find_by_username(
        pool: &PgPool,
        username: &str,
    ) -> Result<Option<UserProfile>, sqlx::Error> {
        let sql = format!("SELECT {PROFILE_COLS} FROM usuarios_ext WHERE username = $1");
        sqlx::query_as::<_, UserProfile>(&sql)
            .bind(username)
            .fetch_optional(pool)
            .await
    }

    /// Aplica patch parcial. Solo actualiza columnas con `Some(_)`.
    pub async fn update(
        pool: &PgPool,
        user_id: i32,
        patch: &UpdateProfileRequest,
    ) -> Result<UserProfile, AppError> {
        let mut idx = 1usize;
        let mut clauses: Vec<String> = Vec::new();
        let mut binders: Vec<Binder> = Vec::new();
        if let Some(v) = &patch.nombre_visible {
            clauses.push(format!("nombre_visible = ${}", {
                idx += 1;
                idx - 1
            }));
            binders.push(Binder::Str(v.clone()));
        }
        if let Some(v) = &patch.bio {
            clauses.push(format!("bio = ${}", {
                idx += 1;
                idx - 1
            }));
            binders.push(Binder::Str(v.clone()));
        }
        if let Some(v) = &patch.avatar_url {
            clauses.push(format!("avatar_url = ${}", {
                idx += 1;
                idx - 1
            }));
            binders.push(Binder::Str(v.clone()));
        }
        if let Some(v) = &patch.portada_url {
            clauses.push(format!("portada_url = ${}", {
                idx += 1;
                idx - 1
            }));
            binders.push(Binder::Str(v.clone()));
        }
        if let Some(v) = &patch.sitio_web {
            clauses.push(format!("sitio_web = ${}", {
                idx += 1;
                idx - 1
            }));
            binders.push(Binder::Str(v.clone()));
        }
        if let Some(v) = &patch.generos_favoritos {
            clauses.push(format!("generos_favoritos = ${}", {
                idx += 1;
                idx - 1
            }));
            binders.push(Binder::Json(v.clone()));
        }

        if clauses.is_empty() {
            return Self::find_by_id(pool, user_id)
                .await?
                .ok_or(AppError::NotFound("Usuario".into()));
        }

        let sql = format!(
            "UPDATE usuarios_ext SET {set_clause} WHERE id = ${id_pos} RETURNING {PROFILE_COLS}",
            set_clause = clauses.join(", "),
            id_pos = idx,
        );
        let mut q = sqlx::query_as::<_, UserProfile>(&sql);
        for b in &binders {
            q = match b {
                Binder::Str(s) => q.bind(s),
                Binder::Json(j) => q.bind(j),
            };
        }
        q = q.bind(user_id);
        Ok(q.fetch_one(pool).await?)
    }
}

enum Binder {
    Str(String),
    Json(JsonValue),
}
