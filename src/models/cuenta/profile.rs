use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;
use utoipa::ToSchema;
use validator::Validate;

/* [174A-24] Modelo extendido y DTOs de perfil sobre `usuarios_ext`. */

#[derive(Debug, Clone, FromRow)]
pub struct UserProfile {
    pub id: i32,
    pub username: String,
    pub email: Option<String>,
    pub nombre_visible: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub portada_url: Option<String>,
    pub sitio_web: Option<String>,
    pub generos_favoritos: Option<JsonValue>,
    pub plan: String,
    pub rol: String,
    pub verificado: Option<bool>,
    pub total_seguidores: Option<i32>,
    pub total_seguidos: Option<i32>,
    pub total_samples: Option<i32>,
    pub total_descargas: Option<i32>,
    pub estado: String,
    pub created_at: Option<DateTime<Utc>>,
}

/* DTO publico (perfil ajeno): sin email, sin paypal, sin estado. */
#[derive(Debug, Serialize, ToSchema)]
pub struct PublicProfileResponse {
    pub id: i32,
    pub username: String,
    pub nombre_visible: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub portada_url: Option<String>,
    pub sitio_web: Option<String>,
    #[schema(value_type = Option<Object>)]
    pub generos_favoritos: Option<JsonValue>,
    pub plan: String,
    pub rol: String,
    pub verificado: bool,
    pub total_seguidores: i32,
    pub total_seguidos: i32,
    pub total_samples: i32,
    pub total_descargas: i32,
    pub created_at: Option<DateTime<Utc>>,
    /* [296A-2] Indica si el viewer autenticado sigue a este usuario.
     * Siempre false para visitantes anonimos o cuando el viewer es el propio usuario. */
    pub siguiendo: bool,
}

/* DTO privado (perfil propio): incluye email y estado. */
#[derive(Debug, Serialize, ToSchema)]
pub struct PrivateProfileResponse {
    #[serde(flatten)]
    pub public: PublicProfileResponse,
    pub email: Option<String>,
    pub estado: String,
}

impl From<UserProfile> for PublicProfileResponse {
    fn from(u: UserProfile) -> Self {
        Self {
            id: u.id,
            username: u.username,
            nombre_visible: u.nombre_visible,
            bio: u.bio,
            avatar_url: u.avatar_url,
            portada_url: u.portada_url,
            sitio_web: u.sitio_web,
            generos_favoritos: u.generos_favoritos,
            plan: u.plan,
            rol: u.rol,
            verificado: u.verificado.unwrap_or(false),
            total_seguidores: u.total_seguidores.unwrap_or(0),
            total_seguidos: u.total_seguidos.unwrap_or(0),
            total_samples: u.total_samples.unwrap_or(0),
            total_descargas: u.total_descargas.unwrap_or(0),
            created_at: u.created_at,
            siguiendo: false,
        }
    }
}

impl PublicProfileResponse {
    /* [296A-2] Construye el perfil con el campo `siguiendo` ya calculado.
     * El caller consulta FollowRepository::is_following() y pasa el resultado. */
    pub fn from_with_follow(u: UserProfile, is_following: bool) -> Self {
        let mut resp = Self::from(u);
        resp.siguiendo = is_following;
        resp
    }
}

impl From<UserProfile> for PrivateProfileResponse {
    fn from(u: UserProfile) -> Self {
        let email = u.email.clone();
        let estado = u.estado.clone();
        Self {
            public: u.into(),
            email,
            estado,
        }
    }
}

#[derive(Debug, Deserialize, Validate, ToSchema, Default)]
pub struct UpdateProfileRequest {
    #[validate(length(max = 100))]
    pub nombre_visible: Option<String>,
    #[validate(length(max = 1000))]
    pub bio: Option<String>,
    #[validate(url)]
    pub avatar_url: Option<String>,
    #[validate(url)]
    pub portada_url: Option<String>,
    #[validate(url)]
    pub sitio_web: Option<String>,
    #[schema(value_type = Option<Object>)]
    pub generos_favoritos: Option<JsonValue>,
}
