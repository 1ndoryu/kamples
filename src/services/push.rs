use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use p256::ecdsa::{signature::Signer, SigningKey};
use reqwest::header::{HeaderValue, AUTHORIZATION};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use utoipa::ToSchema;
use web_push_native::{Auth, WebPushBuilder};

use crate::config::AppConfig;
use crate::errors::AppError;
use crate::repositories::{
    PushSubscriptionRecord, PushSubscriptionRepository, RegisterPushSubscriptionRecord,
};
use crate::AppState;

const DEFAULT_PUSH_TTL_SECONDS: u32 = 300;
// RFC 8292: el JWT VAPID puede valer como máximo 24 h; usamos 12 h.
const VAPID_JWT_TTL_SECONDS: u64 = 12 * 60 * 60;
const FALLBACK_VAPID_SUBJECT: &str = "https://kamples.com";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum PushSubscriptionPlatform {
    #[default]
    Web,
    Android,
    Desktop,
}

impl PushSubscriptionPlatform {
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Android => "android",
            Self::Desktop => "desktop",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PushNotificationPayload {
    pub title: String,
    pub body: String,
    pub data: serde_json::Value,
    pub tag: Option<String>,
    pub icon_url: Option<String>,
    pub badge_url: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PushSendSummary {
    pub attempted: usize,
    pub delivered: usize,
    pub deactivated: usize,
}

#[derive(Clone)]
pub struct PushDeliveryRuntime {
    client: reqwest::Client,
    signing_key: SigningKey,
    public_key: String,
    subject: String,
}

#[derive(Debug, thiserror::Error)]
pub enum PushDeliveryRuntimeError {
    #[error("Configuración VAPID incompleta: define ambas variables o ninguna ({0})")]
    PartialConfig(&'static str),
    #[error("La clave privada VAPID es inválida: {0}")]
    InvalidPrivateKey(String),
    #[error("La clave pública VAPID no coincide con la derivada desde la privada")]
    PublicKeyMismatch,
    #[error("No se pudo inicializar el cliente Web Push: {0}")]
    ClientInitialization(String),
}

#[derive(Debug)]
enum PushSendFailure {
    ExpiredEndpoint,
    Other(String),
}

pub struct PushNotificationService;

/* [174A-75] Runtime y servicio Web Push VAPID.
 * - La inicialización deriva/valida la clave pública a partir de la privada.
 * - El registro conserva el upsert por endpoint del legado.
 * - El envío invalida endpoints 404/410 equivalentes (`EndpointNotFound/NotValid`).
 * - El fanout notify(user, event) queda para 174A-78, pero la pieza de canal
 *   ya queda lista y reusable desde mensajes, likes o notificaciones sistema. */

impl PushDeliveryRuntime {
    pub fn from_config(config: &AppConfig) -> Result<Option<Self>, PushDeliveryRuntimeError> {
        let private_key = if let Some(value) = config.vapid_private_key.as_ref() {
            value.trim()
        } else {
            if config.vapid_public_key.is_some() || config.vapid_subject.is_some() {
                return Err(PushDeliveryRuntimeError::PartialConfig(
                    "VAPID_PRIVATE_KEY/KAMPLES_VAPID_PRIVATE_KEY",
                ));
            }
            return Ok(None);
        };

        let signing_key =
            decode_vapid_private_key(private_key).map_err(PushDeliveryRuntimeError::InvalidPrivateKey)?;
        let derived_public_key =
            URL_SAFE_NO_PAD.encode(signing_key.verifying_key().to_encoded_point(false));

        if let Some(configured_public_key) = config.vapid_public_key.as_ref() {
            if configured_public_key.trim() != derived_public_key {
                return Err(PushDeliveryRuntimeError::PublicKeyMismatch);
            }
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|error| PushDeliveryRuntimeError::ClientInitialization(error.to_string()))?;

        Ok(Some(Self {
            client,
            signing_key,
            public_key: derived_public_key,
            subject: config
                .vapid_subject
                .clone()
                .or_else(|| config.public_base_url.clone())
                .unwrap_or_else(|| FALLBACK_VAPID_SUBJECT.to_string()),
        }))
    }

    pub fn public_key(&self) -> &str {
        &self.public_key
    }

    pub fn subject(&self) -> &str {
        &self.subject
    }
}

impl PushNotificationService {
    pub async fn subscribe(
        pool: &PgPool,
        user_id: i32,
        endpoint: &str,
        p256dh: &str,
        auth: &str,
        platform: PushSubscriptionPlatform,
    ) -> Result<(), AppError> {
        validate_subscription(endpoint, p256dh, auth)?;

        PushSubscriptionRepository::upsert(
            pool,
            RegisterPushSubscriptionRecord {
                user_id,
                endpoint: endpoint.trim(),
                p256dh: p256dh.trim(),
                auth: auth.trim(),
                platform: platform.as_db_str(),
            },
        )
        .await
    }

    pub async fn unsubscribe(
        pool: &PgPool,
        user_id: i32,
        endpoint: &str,
    ) -> Result<bool, AppError> {
        validate_endpoint(endpoint)?;
        PushSubscriptionRepository::delete_for_user_by_endpoint(pool, user_id, endpoint.trim())
            .await
    }

    pub async fn send_to_user(
        state: &AppState,
        user_id: i32,
        payload: PushNotificationPayload,
    ) -> Result<PushSendSummary, AppError> {
        let Some(runtime) = state.push_runtime.as_ref() else {
            tracing::debug!(user_id, "web push deshabilitado por configuración ausente");
            return Ok(PushSendSummary::default());
        };

        let subscriptions =
            PushSubscriptionRepository::list_active_by_user(&state.pool, user_id).await?;
        if subscriptions.is_empty() {
            return Ok(PushSendSummary::default());
        }

        let payload_bytes =
            serde_json::to_vec(&build_delivery_payload(payload)).map_err(|error| {
                AppError::Internal(format!("No se pudo serializar el payload push: {error}"))
            })?;

        let mut summary = PushSendSummary::default();
        for subscription in subscriptions {
            summary.attempted += 1;
            let endpoint = subscription.endpoint.clone();
            let platform = subscription.platform.clone();

            match send_to_subscription(runtime, &subscription, &payload_bytes).await {
                Ok(()) => {
                    summary.delivered += 1;
                }
                Err(PushSendFailure::ExpiredEndpoint) => {
                    PushSubscriptionRepository::mark_inactive(&state.pool, &endpoint).await?;
                    summary.deactivated += 1;
                    tracing::info!(
                        user_id,
                        platform,
                        endpoint = %truncate_endpoint(&endpoint),
                        "endpoint push expirado y desactivado"
                    );
                }
                Err(PushSendFailure::Other(error)) => {
                    tracing::warn!(
                        user_id,
                        platform,
                        endpoint = %truncate_endpoint(&endpoint),
                        error = %error,
                        "falló el envío web push"
                    );
                }
            }
        }

        Ok(summary)
    }
}

async fn send_to_subscription(
    runtime: &PushDeliveryRuntime,
    subscription: &PushSubscriptionRecord,
    payload_bytes: &[u8],
) -> Result<(), PushSendFailure> {
    let endpoint = url::Url::parse(&subscription.endpoint)
        .map_err(|error| PushSendFailure::Other(format!("endpoint inválido: {error}")))?;
    let endpoint_uri = endpoint
        .as_str()
        .parse::<http::Uri>()
        .map_err(|error| PushSendFailure::Other(format!("endpoint inválido: {error}")))?;
    let ua_public = decode_subscription_public_key(&subscription.p256dh)?;
    let ua_auth = decode_subscription_auth(&subscription.auth)?;

    let mut request = WebPushBuilder::new(endpoint_uri, ua_public, ua_auth)
        .with_valid_duration(Duration::from_secs(u64::from(DEFAULT_PUSH_TTL_SECONDS)))
        .build(payload_bytes)
        .map_err(|error| PushSendFailure::Other(error.to_string()))?;

    let jwt = build_vapid_jwt(
        &runtime.signing_key,
        &endpoint.origin().ascii_serialization(),
        runtime.subject(),
    )?;
    let authorization = HeaderValue::from_str(&format!("vapid t={jwt}, k={}", runtime.public_key()))
        .map_err(|error| PushSendFailure::Other(error.to_string()))?;
    request.headers_mut().insert(AUTHORIZATION, authorization);
    request
        .headers_mut()
        .insert("urgency", HeaderValue::from_static("normal"));

    let reqwest_request = reqwest::Request::try_from(request)
        .map_err(|error| PushSendFailure::Other(error.to_string()))?;
    let response = runtime
        .client
        .execute(reqwest_request)
        .await
        .map_err(|error| PushSendFailure::Other(error.to_string()))?;

    let status = response.status();
    if status.is_success() {
        Ok(())
    } else if status == StatusCode::NOT_FOUND || status == StatusCode::GONE {
        // 404/410: el servicio push dio de baja la suscripción (antes EndpointNotFound/NotValid).
        Err(PushSendFailure::ExpiredEndpoint)
    } else {
        Err(PushSendFailure::Other(format!("HTTP {status}")))
    }
}

fn build_delivery_payload(payload: PushNotificationPayload) -> serde_json::Value {
    let mut body = serde_json::Map::new();
    body.insert("title".into(), serde_json::Value::String(payload.title));
    body.insert("body".into(), serde_json::Value::String(payload.body));
    body.insert("data".into(), normalize_payload_data(payload.data));

    if let Some(tag) = payload
        .tag
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        body.insert("tag".into(), serde_json::Value::String(tag));
    }
    if let Some(icon_url) = payload
        .icon_url
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        body.insert("icon".into(), serde_json::Value::String(icon_url));
    }
    if let Some(badge_url) = payload
        .badge_url
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        body.insert("badge".into(), serde_json::Value::String(badge_url));
    }

    serde_json::Value::Object(body)
}

fn normalize_payload_data(data: serde_json::Value) -> serde_json::Value {
    if data.is_null() {
        serde_json::json!({})
    } else {
        data
    }
}

fn validate_subscription(endpoint: &str, p256dh: &str, auth: &str) -> Result<(), AppError> {
    if endpoint.trim().is_empty() || p256dh.trim().is_empty() || auth.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Campos endpoint, keys.p256dh y keys.auth son obligatorios".into(),
        ));
    }

    validate_endpoint(endpoint)
}

fn validate_endpoint(endpoint: &str) -> Result<(), AppError> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() {
        return Err(AppError::BadRequest("Campo endpoint es obligatorio".into()));
    }

    let parsed = url::Url::parse(endpoint)
        .map_err(|_| AppError::BadRequest("Endpoint debe ser una URL HTTPS válida".into()))?;
    if parsed.scheme() != "https" {
        return Err(AppError::BadRequest(
            "Endpoint debe ser una URL HTTPS válida".into(),
        ));
    }

    Ok(())
}

/// La clave privada VAPID es un escalar P-256 en base64url sin relleno (RFC 8292).
fn decode_vapid_private_key(private_key: &str) -> Result<SigningKey, String> {
    let bytes = URL_SAFE_NO_PAD
        .decode(private_key.trim_end_matches('='))
        .map_err(|error| error.to_string())?;
    SigningKey::from_slice(&bytes).map_err(|error| error.to_string())
}

fn decode_subscription_public_key(value: &str) -> Result<p256::PublicKey, PushSendFailure> {
    let bytes = decode_subscription_bytes(value, "p256dh")?;
    p256::PublicKey::from_sec1_bytes(&bytes)
        .map_err(|error| PushSendFailure::Other(format!("p256dh inválido: {error}")))
}

fn decode_subscription_auth(value: &str) -> Result<Auth, PushSendFailure> {
    let bytes = decode_subscription_bytes(value, "auth")?;
    // `clone_from_slice` entra en pánico con otra longitud: validar antes.
    if bytes.len() != 16 {
        return Err(PushSendFailure::Other(format!(
            "auth debe medir 16 bytes, mide {}",
            bytes.len()
        )));
    }
    Ok(Auth::clone_from_slice(&bytes))
}

fn decode_subscription_bytes(value: &str, field: &str) -> Result<Vec<u8>, PushSendFailure> {
    URL_SAFE_NO_PAD
        .decode(value.trim().trim_end_matches('='))
        .map_err(|error| PushSendFailure::Other(format!("{field} inválido: {error}")))
}

/// JWT VAPID (RFC 8292): ES256 sobre `aud` (origen del endpoint), `exp` (≤ 24 h) y `sub`.
fn build_vapid_jwt(
    signing_key: &SigningKey,
    audience: &str,
    subject: &str,
) -> Result<String, PushSendFailure> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| PushSendFailure::Other(format!("reloj del sistema inválido: {error}")))?
        .as_secs();
    let header = URL_SAFE_NO_PAD.encode(r#"{"typ":"JWT","alg":"ES256"}"#);
    let claims = URL_SAFE_NO_PAD.encode(
        serde_json::json!({
            "aud": audience,
            "exp": now + VAPID_JWT_TTL_SECONDS,
            "sub": subject,
        })
        .to_string(),
    );
    let signing_input = format!("{header}.{claims}");
    // ECDSA P-256 con SHA-256; `to_bytes` da r||s de 64 bytes, el formato JWS ES256.
    let signature: p256::ecdsa::Signature = signing_key.sign(signing_input.as_bytes());
    Ok(format!(
        "{signing_input}.{}",
        URL_SAFE_NO_PAD.encode(signature.to_bytes())
    ))
}

fn truncate_endpoint(endpoint: &str) -> String {
    endpoint.chars().take(80).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        build_vapid_jwt, decode_subscription_auth, decode_vapid_private_key,
        normalize_payload_data, validate_endpoint, PushDeliveryRuntime, PushDeliveryRuntimeError,
    };
    use crate::config::AppConfig;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    use p256::ecdsa::{signature::Verifier, Signature};
    use std::time::{SystemTime, UNIX_EPOCH};

    const TEST_PRIVATE_KEY: &str = "IQ9Ur0ykXoHS9gzfYX0aBjy9lvdrjx_PFUXmie9YRcY";

    fn test_config(public_key: Option<&str>) -> AppConfig {
        AppConfig {
            database_url: "postgres://unused".into(),
            redis_url: None,
            jwt_secret: "secret".into(),
            host: "127.0.0.1".into(),
            port: 3000,
            db_max_connections: 10,
            db_min_connections: 2,
            google_client_ids: Vec::new(),
            storage_root: "./uploads".into(),
            storage_backend: "local".into(),
            s3_bucket: None,
            s3_endpoint_url: None,
            public_base_url: Some("https://kamples.com".into()),
            ws_secret: "secret".into(),
            ws_public_url: None,
            ws_ticket_ttl_secs: 60,
            vapid_public_key: public_key.map(str::to_string),
            vapid_private_key: Some(TEST_PRIVATE_KEY.into()),
            vapid_subject: None,
            fcm_service_account_json: None,
            smtp: None,
            scraper_secret: None,
            stripe: crate::config::StripeConfig::default(),
            allow_duplicate_uploads: false,
        }
    }

    #[test]
    fn null_payload_becomes_object() {
        assert_eq!(
            normalize_payload_data(serde_json::Value::Null),
            serde_json::json!({})
        );
    }

    #[test]
    fn endpoint_requires_https() {
        assert!(validate_endpoint("https://push.example.test/subscription").is_ok());
        assert!(validate_endpoint("http://push.example.test/subscription").is_err());
        assert!(validate_endpoint("notaurl").is_err());
    }

    #[test]
    fn runtime_derives_public_key_from_private_key() {
        let runtime = PushDeliveryRuntime::from_config(&test_config(None))
            .expect("runtime válido")
            .expect("runtime habilitado");

        // Clave pública P-256 sin comprimir: 0x04 || X || Y (65 bytes).
        let public_key = URL_SAFE_NO_PAD
            .decode(runtime.public_key())
            .expect("clave pública base64url");
        assert_eq!(public_key.len(), 65);
        assert_eq!(public_key[0], 0x04);
        assert_eq!(runtime.subject(), "https://kamples.com");
    }

    #[test]
    fn runtime_rejects_public_key_that_does_not_match_private_key() {
        assert!(matches!(
            PushDeliveryRuntime::from_config(&test_config(Some("AAAA"))),
            Err(PushDeliveryRuntimeError::PublicKeyMismatch)
        ));
    }

    #[test]
    fn vapid_jwt_is_es256_and_verifies_with_derived_key() {
        let signing_key = decode_vapid_private_key(TEST_PRIVATE_KEY).expect("clave válida");
        let jwt = build_vapid_jwt(
            &signing_key,
            "https://push.example.test",
            "https://kamples.com",
        )
        .expect("jwt");
        let parts: Vec<&str> = jwt.split('.').collect();
        assert_eq!(parts.len(), 3);

        let header: serde_json::Value = serde_json::from_slice(
            &URL_SAFE_NO_PAD.decode(parts[0]).expect("header base64url"),
        )
        .expect("header json");
        assert_eq!(header, serde_json::json!({"typ": "JWT", "alg": "ES256"}));

        let claims: serde_json::Value = serde_json::from_slice(
            &URL_SAFE_NO_PAD.decode(parts[1]).expect("claims base64url"),
        )
        .expect("claims json");
        assert_eq!(claims["aud"], "https://push.example.test");
        assert_eq!(claims["sub"], "https://kamples.com");
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("reloj")
            .as_secs();
        let exp = claims["exp"].as_u64().expect("exp numérico");
        assert!(exp > now && exp <= now + 24 * 60 * 60);

        let signature = Signature::from_slice(
            &URL_SAFE_NO_PAD.decode(parts[2]).expect("firma base64url"),
        )
        .expect("firma r||s de 64 bytes");
        let signing_input = format!("{}.{}", parts[0], parts[1]);
        signing_key
            .verifying_key()
            .verify(signing_input.as_bytes(), &signature)
            .expect("firma ES256 válida");
    }

    #[test]
    fn subscription_auth_must_be_16_bytes() {
        assert!(decode_subscription_auth(&URL_SAFE_NO_PAD.encode([0u8; 16])).is_ok());
        assert!(decode_subscription_auth(&URL_SAFE_NO_PAD.encode([0u8; 15])).is_err());
    }
}
