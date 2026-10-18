use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use validator::Validate;

/* [174A-18+174A-20] Modelo Usuario sobre `usuarios_ext`. */
#[derive(Debug, Clone, FromRow)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub email: Option<String>,
    pub nombre_visible: String,
    pub password_hash: Option<String>,
    pub plan: String,
    pub rol: String,
    pub estado: String,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, ToSchema, Clone)]
pub struct UserResponse {
    pub id: i32,
    pub username: String,
    pub email: Option<String>,
    pub nombre_visible: String,
    pub plan: String,
    pub rol: String,
}

impl From<User> for UserResponse {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            username: u.username,
            email: u.email,
            nombre_visible: u.nombre_visible,
            plan: u.plan,
            rol: u.rol,
        }
    }
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct RegisterRequest {
    #[validate(length(min = 3, max = 50, message = "Username 3-50 caracteres"))]
    pub username: String,
    #[validate(email(message = "Email invalido"))]
    pub email: String,
    #[validate(length(min = 4, message = "Contrasena minimo 4 caracteres"))]
    pub password: String,
    #[validate(length(max = 100))]
    pub nombre_visible: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct LoginRequest {
    /* [256A-1k] alias="email" para compatibilidad con frontend legacy
     * que envía { email, password } en vez de { identifier, password } */
    #[validate(length(min = 3))]
    #[serde(alias = "email")]
    pub identifier: String,
    pub password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize, ToSchema, Default)]
pub struct LogoutRequest {
    pub refresh_token: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct GoogleAuthRequest {
    #[validate(length(min = 10))]
    pub id_token: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct GooglePkceRequest {
    #[validate(length(min = 5))]
    pub code: String,
    #[validate(length(min = 32))]
    pub code_verifier: String,
    #[validate(length(min = 5))]
    pub redirect_uri: String,
    /// Opcional: usar uno especifico de los configurados; si se omite, se usa el primero.
    pub client_id: Option<String>,
}

/* [274A-21] Cambiar email del usuario autenticado. Requiere password actual. */
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ChangeEmailRequest {
    #[validate(email(message = "Email invalido"))]
    #[serde(alias = "nuevoEmail")]
    pub nuevo_email: String,
    #[validate(length(min = 1, message = "La contrasena actual es requerida"))]
    #[serde(alias = "passwordActual")]
    pub password_actual: String,
}

/* [274A-22] Cambiar password del usuario autenticado. Requiere password actual. */
#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ChangePasswordRequest {
    #[validate(length(min = 1, message = "La contrasena actual es requerida"))]
    #[serde(alias = "passwordActual")]
    pub password_actual: String,
    #[validate(length(
        min = 8,
        message = "La nueva contrasena debe tener al menos 8 caracteres"
    ))]
    #[serde(alias = "nuevaPassword")]
    pub nueva_password: String,
    #[serde(alias = "confirmarPassword")]
    pub confirmar_password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SimpleOkResponse {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    pub token: String,
    pub refresh_token: String,
    pub user: UserResponse,
}
