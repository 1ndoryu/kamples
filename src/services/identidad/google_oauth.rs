use std::sync::Arc;
use std::time::{Duration, Instant};

use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::errors::AppError;

/* [174A-21] Verificador de Google ID tokens (JWKS RS256).
 * Cache en memoria de keys con TTL 1h. */

const GOOGLE_JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";
const GOOGLE_ISSUERS: &[&str] = &["https://accounts.google.com", "accounts.google.com"];
const JWKS_TTL: Duration = Duration::from_hours(1);

#[derive(Debug, Deserialize, Clone)]
struct Jwk {
    kid: String,
    n: String,
    e: String,
    #[serde(default)]
    alg: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
pub struct GoogleIdClaims {
    pub iss: String,
    pub sub: String,
    pub aud: String,
    pub exp: i64,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub email_verified: Option<bool>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub picture: Option<String>,
    #[serde(default, alias = "given_name")]
    pub given_name: Option<String>,
}

struct CacheEntry {
    keys: Vec<Jwk>,
    fetched: Instant,
}

pub struct GoogleVerifier {
    client_ids: Vec<String>,
    cache: Arc<RwLock<Option<CacheEntry>>>,
    http: reqwest::Client,
}

impl GoogleVerifier {
    pub fn new(client_ids: Vec<String>) -> Self {
        Self {
            client_ids,
            cache: Arc::new(RwLock::new(None)),
            /* La config es estatica, asi que solo falla si el backend TLS del
             * proceso esta roto. Se sigue con un cliente sin timeout antes que
             * tumbar el arranque, y queda en el log. */
            http: match reqwest::Client::builder()
                .timeout(Duration::from_secs(8))
                .build()
            {
                Ok(cliente) => cliente,
                Err(error) => {
                    tracing::error!(error = %error, "no se pudo construir el cliente HTTP de Google");
                    reqwest::Client::new()
                }
            },
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.client_ids.is_empty()
    }

    pub fn client_ids(&self) -> &[String] {
        &self.client_ids
    }

    /// [174A-22] Intercambia (code, code_verifier) por tokens via PKCE en
    /// https://oauth2.googleapis.com/token. Devuelve el id_token (string).
    pub async fn exchange_pkce(
        &self,
        code: &str,
        code_verifier: &str,
        redirect_uri: &str,
        client_id_hint: Option<&str>,
    ) -> Result<String, AppError> {
        #[derive(Deserialize)]
        struct TokenResp {
            id_token: Option<String>,
            error: Option<String>,
        }
        let client_id =
            match client_id_hint {
                Some(id) if self.client_ids.iter().any(|c| c == id) => id.to_string(),
                Some(_) => return Err(AppError::Unauthorized),
                None => self.client_ids.first().cloned().ok_or_else(|| {
                    AppError::Internal("GOOGLE_CLIENT_IDS no configurados".into())
                })?,
            };
        let form = [
            ("grant_type", "authorization_code"),
            ("client_id", client_id.as_str()),
            ("code", code),
            ("code_verifier", code_verifier),
            ("redirect_uri", redirect_uri),
        ];
        let resp = self
            .http
            .post("https://oauth2.googleapis.com/token")
            .form(&form)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("Google token exchange: {e}")))?;
        let parsed: TokenResp = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("Google token parse: {e}")))?;
        if let Some(err) = parsed.error {
            return Err(AppError::Forbidden(format!("Google PKCE: {err}")));
        }
        parsed.id_token.ok_or(AppError::Unauthorized)
    }

    async fn keys(&self) -> Result<Vec<Jwk>, AppError> {
        {
            let g = self.cache.read().await;
            if let Some(e) = g.as_ref() {
                if e.fetched.elapsed() < JWKS_TTL {
                    return Ok(e.keys.clone());
                }
            }
        }
        let resp = self
            .http
            .get(GOOGLE_JWKS_URL)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("JWKS fetch: {e}")))?;
        let jwks: Jwks = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("JWKS parse: {e}")))?;
        let mut w = self.cache.write().await;
        *w = Some(CacheEntry {
            keys: jwks.keys.clone(),
            fetched: Instant::now(),
        });
        Ok(jwks.keys)
    }

    pub async fn verify(&self, id_token: &str) -> Result<GoogleIdClaims, AppError> {
        if self.client_ids.is_empty() {
            return Err(AppError::Internal(
                "GOOGLE_CLIENT_IDS no configurados".into(),
            ));
        }
        let header = decode_header(id_token).map_err(|_| AppError::Unauthorized)?;
        let kid = header.kid.ok_or(AppError::Unauthorized)?;
        let keys = self.keys().await?;
        let jwk = keys
            .iter()
            .find(|k| k.kid == kid)
            .ok_or(AppError::Unauthorized)?;
        let dk =
            DecodingKey::from_rsa_components(&jwk.n, &jwk.e).map_err(|_| AppError::Unauthorized)?;
        let mut val = Validation::new(
            jwk.alg
                .as_deref()
                .and_then(|a| match a {
                    "RS256" => Some(Algorithm::RS256),
                    _ => None,
                })
                .unwrap_or(Algorithm::RS256),
        );
        val.set_audience(&self.client_ids);
        val.set_issuer(GOOGLE_ISSUERS);
        let data =
            decode::<GoogleIdClaims>(id_token, &dk, &val).map_err(|_| AppError::Unauthorized)?;
        Ok(data.claims)
    }
}
