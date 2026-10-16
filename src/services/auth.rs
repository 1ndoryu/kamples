use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use deadpool_redis::Pool as RedisPool;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::AppError;
use crate::models::{AuthResponse, LoginRequest, RegisterRequest, UserResponse};
use crate::repositories::{OAuthRepository, UserRepository};
use crate::services::{GoogleVerifier, TokenStore};

/* [174A-18+174A-20] JWT claims sobre `usuarios_ext`.
 * jti = identificador unico del access token (para revocacion via blacklist). */
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i32,
    pub exp: i64,
    pub iat: i64,
    pub jti: String,
    pub plan: String,
    pub rol: String,
}

pub struct AuthService;

impl AuthService {
    pub async fn register(
        pool: &PgPool,
        redis: &Option<RedisPool>,
        req: RegisterRequest,
        jwt_secret: &str,
    ) -> Result<AuthResponse, AppError> {
        if UserRepository::find_by_email(pool, &req.email)
            .await?
            .is_some()
        {
            return Err(AppError::Conflict("Email ya registrado".into()));
        }
        if UserRepository::find_by_username(pool, &req.username)
            .await?
            .is_some()
        {
            return Err(AppError::Conflict("Username ya registrado".into()));
        }
        let salt = SaltString::generate(&mut OsRng);
        let password_hash = Argon2::default()
            .hash_password(req.password.as_bytes(), &salt)
            .map_err(|e| AppError::Internal(format!("Hash error: {e}")))?
            .to_string();
        let nombre = req
            .nombre_visible
            .clone()
            .unwrap_or_else(|| req.username.clone());
        let user =
            UserRepository::create_native(pool, &req.username, &req.email, &password_hash, &nombre)
                .await?;
        Self::issue_pair(redis, &user, jwt_secret).await
    }

    pub async fn login(
        pool: &PgPool,
        redis: &Option<RedisPool>,
        req: LoginRequest,
        jwt_secret: &str,
    ) -> Result<AuthResponse, AppError> {
        let user = UserRepository::find_by_identifier(pool, &req.identifier)
            .await?
            .ok_or(AppError::Unauthorized)?;
        if user.estado != "activo" {
            return Err(AppError::Forbidden(format!("Cuenta {}", user.estado)));
        }
        let stored = user
            .password_hash
            .as_deref()
            .ok_or(AppError::Unauthorized)?;
        let parsed = PasswordHash::new(stored)
            .map_err(|e| AppError::Internal(format!("Hash invalido: {e}")))?;
        Argon2::default()
            .verify_password(req.password.as_bytes(), &parsed)
            .map_err(|_| AppError::Unauthorized)?;
        Self::issue_pair(redis, &user, jwt_secret).await
    }

    /// Refresca tokens consumiendo el refresh anterior (rotacion).
    pub async fn refresh(
        pool: &PgPool,
        redis: &Option<RedisPool>,
        refresh_token: &str,
        jwt_secret: &str,
    ) -> Result<AuthResponse, AppError> {
        let user_id = TokenStore::consume_refresh(redis, refresh_token).await?;
        let user = UserRepository::find_by_id(pool, user_id)
            .await?
            .ok_or(AppError::Unauthorized)?;
        if user.estado != "activo" {
            return Err(AppError::Forbidden(format!("Cuenta {}", user.estado)));
        }
        Self::issue_pair(redis, &user, jwt_secret).await
    }

    /// Revoca el access token (jti) y el refresh token asociado.
    pub async fn logout(
        redis: &Option<RedisPool>,
        access_jti: &str,
        refresh_token: Option<&str>,
    ) -> Result<(), AppError> {
        TokenStore::revoke_access(redis, access_jti).await?;
        if let Some(rt) = refresh_token {
            TokenStore::revoke_refresh(redis, rt).await?;
        }
        Ok(())
    }

    /// [174A-21] Login/registro con Google ID token.
    /// Si existe link en `usuarios_ext_oauth` -> login.
    /// Si no, busca por email; si existe, vincula. Si no, crea usuario nuevo y vincula.
    pub async fn google_login(
        pool: &PgPool,
        redis: &Option<RedisPool>,
        google: &GoogleVerifier,
        id_token: &str,
        jwt_secret: &str,
    ) -> Result<AuthResponse, AppError> {
        let claims = google.verify(id_token).await?;
        if !claims.email_verified.unwrap_or(false) {
            return Err(AppError::Forbidden("Email no verificado por Google".into()));
        }
        let provider = "google";
        let user = if let Some(u) =
            OAuthRepository::find_user_by_provider(pool, provider, &claims.sub).await?
        {
            u
        } else {
            let email = claims.email.as_deref();
            let by_email = if let Some(e) = email {
                UserRepository::find_by_email(pool, e).await?
            } else {
                None
            };
            let user = if let Some(u) = by_email {
                u
            } else {
                let base_username = email
                    .and_then(|e| e.split('@').next().map(String::from))
                    .or(claims.given_name.clone())
                    .unwrap_or_else(|| format!("g{}", &claims.sub[..claims.sub.len().min(8)]));
                let nombre = claims.name.clone().unwrap_or_else(|| base_username.clone());
                UserRepository::create_oauth(pool, &base_username, email, &nombre).await?
            };
            OAuthRepository::link(pool, user.id, provider, &claims.sub, email).await?;
            user
        };
        if user.estado != "activo" {
            return Err(AppError::Forbidden(format!("Cuenta {}", user.estado)));
        }
        Self::issue_pair(redis, &user, jwt_secret).await
    }

    /// [174A-22] PKCE flow para clientes desktop: intercambia code+verifier por
    /// id_token con Google, lo verifica y reusa la logica de google_login.
    #[allow(clippy::too_many_arguments)]
    pub async fn google_pkce_login(
        pool: &PgPool,
        redis: &Option<RedisPool>,
        google: &GoogleVerifier,
        code: &str,
        code_verifier: &str,
        redirect_uri: &str,
        client_id_hint: Option<&str>,
        jwt_secret: &str,
    ) -> Result<AuthResponse, AppError> {
        let id_token = google
            .exchange_pkce(code, code_verifier, redirect_uri, client_id_hint)
            .await?;
        Self::google_login(pool, redis, google, &id_token, jwt_secret).await
    }

    async fn issue_pair(
        redis: &Option<RedisPool>,
        user: &crate::models::User,
        jwt_secret: &str,
    ) -> Result<AuthResponse, AppError> {
        let access = Self::generate_access(user, jwt_secret)?;
        let refresh = TokenStore::generate_token();
        TokenStore::save_refresh(redis, &refresh, user.id).await?;
        Ok(AuthResponse {
            token: access,
            refresh_token: refresh,
            user: UserResponse::from(user.clone()),
        })
    }

    pub fn generate_access(user: &crate::models::User, secret: &str) -> Result<String, AppError> {
        let now = chrono::Utc::now();
        let exp = now
            .checked_add_signed(chrono::Duration::hours(24))
            .ok_or_else(|| AppError::Internal("exp overflow".into()))?
            .timestamp();
        let claims = Claims {
            sub: user.id,
            exp,
            iat: now.timestamp(),
            jti: Uuid::now_v7().to_string(),
            plan: user.plan.clone(),
            rol: user.rol.clone(),
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .map_err(|e| AppError::Internal(format!("Token gen: {e}")))
    }

    pub fn verify_token(token: &str, secret: &str) -> Result<Claims, AppError> {
        decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &Validation::default(),
        )
        .map(|d| d.claims)
        .map_err(|_| AppError::Unauthorized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /*
     * [15A-28] Regresión: `jsonwebtoken` 10 ya no trae crypto propio. Sin elegir
     * proveedor en `Cargo.toml`, `encode`/`decode` hacen panic y la autenticación
     * entera queda rota, aunque `cargo check` pase. Este test firma y valida un
     * token de verdad; es el testigo que faltaba al subir de la 9 a la 10.
     */
    #[test]
    fn firma_y_valida_un_token() {
        let secret = "secreto-de-prueba";
        let ahora = chrono::Utc::now().timestamp();
        let claims = Claims {
            sub: 1,
            exp: ahora + 3600,
            iat: ahora,
            jti: Uuid::now_v7().to_string(),
            plan: "free".into(),
            rol: "usuario".into(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .expect("firmar el token no debe fallar");

        let verificado = AuthService::verify_token(&token, secret).expect("el token debe validar");
        assert_eq!(verificado.sub, claims.sub);
        assert_eq!(verificado.jti, claims.jti);
        assert_eq!(verificado.rol, claims.rol);
    }

    /* Un secreto distinto no debe validar: si esto pasara, la firma no se estaría
     * comprobando de verdad y el test anterior no probaría nada. */
    #[test]
    fn un_secreto_distinto_no_valida() {
        let claims = Claims {
            sub: 1,
            exp: chrono::Utc::now().timestamp() + 3600,
            iat: chrono::Utc::now().timestamp(),
            jti: Uuid::now_v7().to_string(),
            plan: "free".into(),
            rol: "usuario".into(),
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(b"secreto-bueno"),
        )
        .expect("firmar el token no debe fallar");

        assert!(AuthService::verify_token(&token, "secreto-malo").is_err());
    }
}
