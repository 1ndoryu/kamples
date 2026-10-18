use sqlx::PgPool;

use crate::models::User;

/* [174A-18] Repositorio sobre `usuarios_ext`.
 * Las consultas son literales para que `sqlx::query_as!` las valide en compilación. */
pub struct UserRepository;

impl UserRepository {
    pub async fn create_native(
        pool: &PgPool,
        username: &str,
        email: &str,
        password_hash: &str,
        nombre_visible: &str,
    ) -> Result<User, sqlx::Error> {
        sqlx::query_as!(
            User,
            "INSERT INTO usuarios_ext (username, email, password_hash, nombre_visible)
             VALUES ($1, $2, $3, $4)
             RETURNING id, username, email, nombre_visible, password_hash, plan, rol, estado, created_at",
            username,
            email,
            password_hash,
            nombre_visible
        )
        .fetch_one(pool)
        .await
    }

    pub async fn find_by_email(pool: &PgPool, email: &str) -> Result<Option<User>, sqlx::Error> {
        sqlx::query_as!(
            User,
            "SELECT id, username, email, nombre_visible, password_hash, plan, rol, estado, created_at
             FROM usuarios_ext WHERE LOWER(email) = LOWER($1) LIMIT 1",
            email
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_username(
        pool: &PgPool,
        username: &str,
    ) -> Result<Option<User>, sqlx::Error> {
        sqlx::query_as!(
            User,
            "SELECT id, username, email, nombre_visible, password_hash, plan, rol, estado, created_at
             FROM usuarios_ext WHERE username = $1 LIMIT 1",
            username
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_identifier(
        pool: &PgPool,
        identifier: &str,
    ) -> Result<Option<User>, sqlx::Error> {
        if identifier.contains('@') {
            Self::find_by_email(pool, identifier).await
        } else {
            Self::find_by_username(pool, identifier).await
        }
    }

    pub async fn find_by_id(pool: &PgPool, id: i32) -> Result<Option<User>, sqlx::Error> {
        sqlx::query_as!(
            User,
            "SELECT id, username, email, nombre_visible, password_hash, plan, rol, estado, created_at
             FROM usuarios_ext WHERE id = $1",
            id
        )
        .fetch_optional(pool)
        .await
    }

    /* [274A-21] Actualiza email del usuario. Asume que la unicidad la valida el caller. */
    pub async fn update_email(
        pool: &PgPool,
        user_id: i32,
        nuevo_email: &str,
    ) -> Result<User, sqlx::Error> {
        sqlx::query_as!(
            User,
            "UPDATE usuarios_ext SET email = $1 WHERE id = $2
             RETURNING id, username, email, nombre_visible, password_hash, plan, rol, estado, created_at",
            nuevo_email,
            user_id
        )
        .fetch_one(pool)
        .await
    }

    /* [274A-22] Actualiza password_hash del usuario. */
    pub async fn update_password(
        pool: &PgPool,
        user_id: i32,
        nuevo_hash: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE usuarios_ext SET password_hash = $1 WHERE id = $2",
            nuevo_hash,
            user_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /* [174A-21] Crear usuario sin password (OAuth-only). Genera username unico si colisiona. */
    pub async fn create_oauth(
        pool: &PgPool,
        base_username: &str,
        email: Option<&str>,
        nombre_visible: &str,
    ) -> Result<User, sqlx::Error> {
        let mut username = sanitize_username(base_username);
        for attempt in 0..20 {
            let candidate = if attempt == 0 {
                username.clone()
            } else {
                format!("{username}{attempt}")
            };
            let exists = Self::find_by_username(pool, &candidate).await?.is_some();
            if !exists {
                username = candidate;
                break;
            }
            if attempt == 19 {
                username = format!("{username}{}", uuid::Uuid::now_v7().simple());
            }
        }
        sqlx::query_as!(
            User,
            "INSERT INTO usuarios_ext (username, email, nombre_visible)
             VALUES ($1, $2, $3)
             RETURNING id, username, email, nombre_visible, password_hash, plan, rol, estado, created_at",
            username,
            email,
            nombre_visible
        )
        .fetch_one(pool)
        .await
    }
}

fn sanitize_username(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('_');
    let base = if trimmed.len() < 3 {
        format!("user_{trimmed}")
    } else {
        trimmed.to_string()
    };
    base.chars().take(40).collect()
}

pub struct OAuthRepository;

impl OAuthRepository {
    pub async fn find_user_by_provider(
        pool: &PgPool,
        provider: &str,
        sub: &str,
    ) -> Result<Option<User>, sqlx::Error> {
        sqlx::query_as!(
            User,
            "SELECT u.id, u.username, u.email, u.nombre_visible, u.password_hash,
                    u.plan, u.rol, u.estado, u.created_at
             FROM usuarios_ext u
             JOIN usuarios_ext_oauth o ON o.user_id = u.id
             WHERE o.provider = $1 AND o.provider_sub = $2 LIMIT 1",
            provider,
            sub
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn link(
        pool: &PgPool,
        user_id: i32,
        provider: &str,
        sub: &str,
        email: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        /* [Fase3-sqlx] Convertido a query! para validacion compile-time. */
        sqlx::query!(
            "INSERT INTO usuarios_ext_oauth (user_id, provider, provider_sub, email) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (provider, provider_sub) DO UPDATE SET email = EXCLUDED.email",
            user_id,
            provider,
            sub,
            email
        )
        .execute(pool)
        .await?;
        Ok(())
    }
}
