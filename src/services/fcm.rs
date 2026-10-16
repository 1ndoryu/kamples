use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tokio::sync::Mutex;
use utoipa::ToSchema;

use crate::config::AppConfig;
use crate::errors::AppError;
use crate::repositories::{FcmTokenRecord, FcmTokenRepository, RegisterFcmTokenRecord};
use crate::AppState;

const DEFAULT_GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const FIREBASE_MESSAGING_SCOPE: &str = "https://www.googleapis.com/auth/firebase.messaging";
const DEFAULT_ACCESS_TOKEN_TTL_SECS: u64 = 3600;
const ACCESS_TOKEN_REFRESH_MARGIN: Duration = Duration::from_mins(5);

#[derive(Debug, Clone)]
pub struct FcmNotificationPayload {
    pub title: String,
    pub body: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FcmSendSummary {
    pub attempted: usize,
    pub delivered: usize,
    pub deactivated: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum FcmTokenPlatform {
    #[default]
    Android,
    Web,
    Ios,
}

impl FcmTokenPlatform {
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Android => "android",
            Self::Web => "web",
            Self::Ios => "ios",
        }
    }

    pub fn from_request_str(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "ios" => Self::Ios,
            "web" => Self::Web,
            _ => Self::Android,
        }
    }
}

#[derive(Clone)]
pub struct FcmDeliveryRuntime {
    client: reqwest::Client,
    credentials: FcmServiceAccount,
    encoding_key: Arc<EncodingKey>,
    message_url: String,
    token_cache: Arc<Mutex<Option<CachedAccessToken>>>,
}

#[derive(Debug, thiserror::Error)]
pub enum FcmDeliveryRuntimeError {
    #[error("El JSON de service-account FCM es inválido: {0}")]
    InvalidJson(String),
    #[error("El service-account FCM está incompleto: faltan {0}")]
    MissingFields(&'static str),
    #[error("La clave privada RSA del service-account FCM es inválida: {0}")]
    InvalidPrivateKey(String),
    #[error("No se pudo inicializar el cliente HTTP para FCM: {0}")]
    ClientInitialization(String),
}

#[derive(Debug, Clone, Deserialize)]
struct FcmServiceAccount {
    project_id: String,
    client_email: String,
    private_key: String,
    #[serde(default = "default_google_token_url")]
    token_uri: String,
}

#[derive(Debug, Clone)]
struct CachedAccessToken {
    token: String,
    expires_at: Instant,
}

#[derive(Debug, Serialize)]
struct GoogleServiceAccountClaims {
    iss: String,
    sub: String,
    aud: String,
    iat: i64,
    exp: i64,
    scope: String,
}

#[derive(Debug, Deserialize)]
struct GoogleAccessTokenResponse {
    access_token: Option<String>,
    expires_in: Option<u64>,
    error: Option<String>,
    error_description: Option<String>,
}

struct FreshAccessToken {
    token: String,
    lifetime: Duration,
}

enum FcmSendFailure {
    InvalidToken(String),
    Other(String),
}

pub struct FcmNotificationService;

/* [174A-76] Runtime y servicio FCM Android.
 * - Usa service-account JSON en env y firma JWT RS256 localmente.
 * - Cachea el access token OAuth en memoria con margen de refresco.
 * - Mantiene el upsert por token del legado y desactiva tokens rechazados.
 * - El fanout global `notify(user, event)` queda explícitamente diferido a 174A-78. */

impl FcmDeliveryRuntime {
    pub fn from_config(config: &AppConfig) -> Result<Option<Self>, FcmDeliveryRuntimeError> {
        let Some(raw_json) = config.fcm_service_account_json.as_deref() else {
            return Ok(None);
        };

        let mut credentials: FcmServiceAccount = serde_json::from_str(raw_json)
            .map_err(|error| FcmDeliveryRuntimeError::InvalidJson(error.to_string()))?;
        if credentials.token_uri.trim().is_empty() {
            credentials.token_uri = default_google_token_url();
        }
        validate_service_account(&credentials)?;

        let encoding_key = EncodingKey::from_rsa_pem(credentials.private_key.as_bytes())
            .map_err(|error| FcmDeliveryRuntimeError::InvalidPrivateKey(error.to_string()))?;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|error| FcmDeliveryRuntimeError::ClientInitialization(error.to_string()))?;

        Ok(Some(Self {
            client,
            message_url: format!(
                "https://fcm.googleapis.com/v1/projects/{}/messages:send",
                credentials.project_id
            ),
            credentials,
            encoding_key: Arc::new(encoding_key),
            token_cache: Arc::new(Mutex::new(None)),
        }))
    }

    pub fn project_id(&self) -> &str {
        &self.credentials.project_id
    }

    async fn access_token(&self) -> Result<String, String> {
        {
            let cache = self.token_cache.lock().await;
            if let Some(entry) = cache.as_ref() {
                if entry.expires_at.saturating_duration_since(Instant::now())
                    > ACCESS_TOKEN_REFRESH_MARGIN
                {
                    return Ok(entry.token.clone());
                }
            }
        }

        let fresh = self.request_access_token().await?;
        let expires_at = Instant::now() + fresh.lifetime;
        let token = fresh.token;

        let mut cache = self.token_cache.lock().await;
        *cache = Some(CachedAccessToken {
            token: token.clone(),
            expires_at,
        });

        Ok(token)
    }

    async fn request_access_token(&self) -> Result<FreshAccessToken, String> {
        let now = Utc::now();
        let claims = GoogleServiceAccountClaims {
            iss: self.credentials.client_email.clone(),
            sub: self.credentials.client_email.clone(),
            aud: self.credentials.token_uri.clone(),
            iat: now.timestamp(),
            exp: (now + chrono::Duration::minutes(50)).timestamp(),
            scope: FIREBASE_MESSAGING_SCOPE.to_string(),
        };

        let assertion =
            jsonwebtoken::encode(&Header::new(Algorithm::RS256), &claims, &self.encoding_key)
                .map_err(|error| format!("No se pudo firmar el JWT OAuth de Google: {error}"))?;

        let response = self
            .client
            .post(&self.credentials.token_uri)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", assertion.as_str()),
            ])
            .send()
            .await
            .map_err(|error| format!("No se pudo solicitar el access token de Google: {error}"))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| format!("No se pudo leer la respuesta OAuth de Google: {error}"))?;

        if !status.is_success() {
            return Err(format!(
                "Google OAuth devolvió {}: {}",
                status.as_u16(),
                truncate_body(&body)
            ));
        }

        let parsed: GoogleAccessTokenResponse = serde_json::from_str(&body)
            .map_err(|error| format!("La respuesta OAuth de Google no es JSON válido: {error}"))?;

        if let Some(error) = parsed.error {
            let description = parsed.error_description.unwrap_or_default();
            return Err(format!("Google OAuth respondió {error}: {description}"));
        }

        let token = parsed
            .access_token
            .ok_or_else(|| "La respuesta OAuth de Google no incluye access_token".to_string())?;
        let lifetime =
            Duration::from_secs(parsed.expires_in.unwrap_or(DEFAULT_ACCESS_TOKEN_TTL_SECS));

        Ok(FreshAccessToken { token, lifetime })
    }
}

impl FcmNotificationService {
    pub async fn register(
        pool: &PgPool,
        user_id: i32,
        token: &str,
        platform: FcmTokenPlatform,
    ) -> Result<(), AppError> {
        validate_token(token)?;

        FcmTokenRepository::upsert(
            pool,
            RegisterFcmTokenRecord {
                user_id,
                token: token.trim(),
                platform: platform.as_db_str(),
            },
        )
        .await
    }

    pub async fn delete(pool: &PgPool, user_id: i32, token: &str) -> Result<bool, AppError> {
        validate_token(token)?;
        FcmTokenRepository::delete_for_user_by_token(pool, user_id, token.trim()).await
    }

    pub async fn send_to_user(
        state: &AppState,
        user_id: i32,
        payload: FcmNotificationPayload,
    ) -> Result<FcmSendSummary, AppError> {
        let Some(runtime) = state.fcm_runtime.as_ref() else {
            tracing::debug!(user_id, "FCM deshabilitado por configuración ausente");
            return Ok(FcmSendSummary::default());
        };

        let tokens = FcmTokenRepository::list_active_by_user(&state.pool, user_id).await?;
        if tokens.is_empty() {
            return Ok(FcmSendSummary::default());
        }

        let access_token =
            runtime
                .access_token()
                .await
                .map_err(|message| AppError::ExternalService {
                    service: "fcm_auth".into(),
                    message,
                })?;

        let mut summary = FcmSendSummary::default();
        for device in tokens {
            summary.attempted += 1;
            let token = device.token.clone();
            let platform = device.platform.clone();

            match send_to_device(runtime, &access_token, &device, &payload).await {
                Ok(()) => {
                    summary.delivered += 1;
                }
                Err(FcmSendFailure::InvalidToken(error)) => {
                    FcmTokenRepository::mark_inactive(&state.pool, &token).await?;
                    summary.deactivated += 1;
                    tracing::info!(
                        user_id,
                        platform,
                        token = %truncate_token(&token),
                        error = %error,
                        "token FCM desactivado"
                    );
                }
                Err(FcmSendFailure::Other(error)) => {
                    tracing::warn!(
                        user_id,
                        platform,
                        token = %truncate_token(&token),
                        error = %error,
                        "falló el envío FCM"
                    );
                }
            }
        }

        Ok(summary)
    }
}

async fn send_to_device(
    runtime: &FcmDeliveryRuntime,
    access_token: &str,
    device: &FcmTokenRecord,
    payload: &FcmNotificationPayload,
) -> Result<(), FcmSendFailure> {
    let request_body = build_message_request(&device.token, payload);
    let response = runtime
        .client
        .post(&runtime.message_url)
        .bearer_auth(access_token)
        .json(&request_body)
        .send()
        .await
        .map_err(|error| FcmSendFailure::Other(error.to_string()))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| FcmSendFailure::Other(error.to_string()))?;

    if status.is_success() {
        return Ok(());
    }

    let message = format!("status {}: {}", status.as_u16(), truncate_body(&body));
    if is_invalid_token_response(status, &body) {
        Err(FcmSendFailure::InvalidToken(message))
    } else {
        Err(FcmSendFailure::Other(message))
    }
}

fn build_message_request(token: &str, payload: &FcmNotificationPayload) -> serde_json::Value {
    let data = stringify_data_payload(&payload.data);
    let image = actor_avatar_url(&payload.data);

    serde_json::json!({
        "message": {
            "token": token,
            "data": merge_string_payload(data, &payload.title, &payload.body),
            "notification": {
                "title": payload.title,
                "body": payload.body,
            },
            "android": {
                "priority": "high",
                "notification": android_notification_payload(&payload.data, image),
            }
        }
    })
}

fn android_notification_payload(
    data: &serde_json::Value,
    image: Option<String>,
) -> serde_json::Value {
    let mut notification = serde_json::Map::new();
    notification.insert(
        "channel_id".into(),
        serde_json::Value::String(channel_id_for_payload(data).to_string()),
    );
    notification.insert(
        "icon".into(),
        serde_json::Value::String("ic_notification".into()),
    );
    notification.insert("default_sound".into(), serde_json::Value::Bool(true));
    notification.insert(
        "notification_count".into(),
        serde_json::Value::Number(1.into()),
    );

    if let Some(image) = image {
        notification.insert("image".into(), serde_json::Value::String(image));
    }

    serde_json::Value::Object(notification)
}

fn merge_string_payload(
    mut data: BTreeMap<String, String>,
    title: &str,
    body: &str,
) -> BTreeMap<String, String> {
    data.insert("titulo".into(), title.to_string());
    data.insert("cuerpo".into(), body.to_string());
    data
}

fn stringify_data_payload(data: &serde_json::Value) -> BTreeMap<String, String> {
    match data {
        serde_json::Value::Object(map) => map
            .iter()
            .map(|(key, value)| {
                let string_value = value
                    .as_str()
                    .map_or_else(|| value.to_string(), ToString::to_string);
                (key.clone(), string_value)
            })
            .collect(),
        serde_json::Value::Null => BTreeMap::new(),
        value => {
            let mut map = BTreeMap::new();
            map.insert("payload".into(), value.to_string());
            map
        }
    }
}

fn actor_avatar_url(data: &serde_json::Value) -> Option<String> {
    data.get("actorAvatarUrl")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn channel_id_for_payload(data: &serde_json::Value) -> &'static str {
    match data.get("tipo").and_then(serde_json::Value::as_str) {
        Some("mensaje_nuevo") => "mensajes",
        _ => "notificaciones",
    }
}

fn is_invalid_token_response(status: StatusCode, body: &str) -> bool {
    if matches!(status, StatusCode::NOT_FOUND | StatusCode::GONE) {
        return true;
    }

    body.to_ascii_uppercase().contains("UNREGISTERED")
}

fn validate_service_account(
    credentials: &FcmServiceAccount,
) -> Result<(), FcmDeliveryRuntimeError> {
    if credentials.project_id.trim().is_empty() {
        return Err(FcmDeliveryRuntimeError::MissingFields("project_id"));
    }
    if credentials.client_email.trim().is_empty() {
        return Err(FcmDeliveryRuntimeError::MissingFields("client_email"));
    }
    if credentials.private_key.trim().is_empty() {
        return Err(FcmDeliveryRuntimeError::MissingFields("private_key"));
    }

    Ok(())
}

fn validate_token(token: &str) -> Result<(), AppError> {
    if token.trim().is_empty() {
        return Err(AppError::BadRequest("Campo token es obligatorio".into()));
    }

    Ok(())
}

fn truncate_body(body: &str) -> String {
    body.chars().take(240).collect()
}

fn truncate_token(token: &str) -> String {
    token.chars().take(24).collect()
}

fn default_google_token_url() -> String {
    DEFAULT_GOOGLE_TOKEN_URL.to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        channel_id_for_payload, stringify_data_payload, FcmDeliveryRuntime, FcmDeliveryRuntimeError,
    };
    use crate::config::AppConfig;

    const TEST_PRIVATE_KEY_PEM: &str = concat!(
        "-----BEGIN PRIVATE KEY-----\n",
        "MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQDZnLNBqzew7LBR\n",
        "OnptvsTx9OuWogJy9keZOp/ujIMrAh5juwJp16jEQDzEAPcnVi0WGkc2oMAZXtAJ\n",
        "u1VH1SVuFAvjGOFTMJ2JBgv1IgL7V+J3sdYCHIZk5HssZQ8fFeGIaDX1ep4bAtBs\n",
        "d4PCnYpzIcrUS5yky3sFqPI16N5R34kWJnjSFD0/Y6r1KkQO70SofO1Ruor8AtDV\n",
        "A2w1n/N2IyD9pIYD8Z5l3MCrjh/5DixXbj0rwEiUOD2uZe+YZMb6Hi4iGTxhFdpF\n",
        "lJAvAns2zro0Lg8p6DSUvhaLU6668LLZTI9KmbmWX9R5BOzFyYrAeGvmTE+PGaJB\n",
        "DuUnibzNAgMBAAECggEAKDKEAltoXCw8naSZvPACXVeKtTaUETxhGXL03BHkoOsx\n",
        "RebjmT2XFTlwgBxVi1Sl23FbOkITehxDfai3Jh+/XEgjsf2EkeNnFkqhptRzjI49\n",
        "bTLSf21ZfgWeoyK/2lQmZxYo8YGG9yJb8c1Z73c+fen+F50oAGD+Bpugskij2KeE\n",
        "58n8D6kYGaOPfPzjym7lnYEGuGWKpq9kldnyISSv0rLqsIfOw+cYH0af7+zzyEGR\n",
        "SFFbzBYCtWQ7qA4RF63RGQyY9c18KTqUTqUs983Hgb8x3pGmxy9ngGSk0FFTJrGT\n",
        "q2y3aoyQUhttEas7K6YhP7NDepkufgbcrksgQlaSmQKBgQDxv4UYAh47sG3T90y4\n",
        "YyF7/YVdWxu50AKiTBWioW2xu2So9ENCqPhFWimkqkK1+JqGSx3Kn746y4nDUd+d\n",
        "3iIaOyRNE7OGleiEaj3bnjHrZXslEPJqMFWyGFXK8am2vpxSdWRKtuYNU1qtvUF5\n",
        "lcZG84I5BnlHr7I+21h36LfnTwKBgQDmcOr4pumI4Xs1Vf8GdlSyn+o8PDqv7Lq1\n",
        "rV/PWZpnd8dHXnaNKbSe/ZmqZOsTnR566V3aF5RJWF+fG6coHddnMqdzIMkOrbm3\n",
        "Ri6Tz2SEfj+AYHDykKOqirMLQIyruxkolaLDN8ymU5q3kGgfFx3uy44iAJZm1l3m\n",
        "T++NkZHTIwKBgACszztU7i6ufHAGFcHCDRrih1fOZFJtgURgwAK3Pq4rXsmV/QYX\n",
        "oLHY4ZrjGtKVQiEz3n5tWcOiQ902wlAXibLXDW/lqS+sBX0xKsENPQhyPRjKZlLj\n",
        "lamspbiuWhH3kEoup7wJrLTG0c8AY0lqoKYcEfYEzZvkorPIOwQCs1jDAoGAM2Vx\n",
        "8t1/bskjqsSwaaeQwnpKSv7/8+bvyb+Og/evKW6cor1d4aQwpdlYIZn6mFhNyQot\n",
        "pYvmxekRArKvOJJXTawNju78COsUZd0gXFVATRC/Zwmbh25dIpdm0ZanCVJkjRm6\n",
        "wKG8Ykh5VIG/x1dnlLAP1mOdJ/id3tVrT37tMFMCgYANtERi61VbMiNu2GwQcLhj\n",
        "SFaM7CL1Z2g9SmpBwaW0qjxl3AzYv4EIvRIbsmc0Nzhv6YnZY6rDing57je7VRO6\n",
        "djrEHzlUfGF2niJK+F65V7oU/OxHdKiQqXrIxifK61THe3oHH1MmkGIvD+lghHS0\n",
        "06m/+UmQqDcz9hICMChP/A==\n",
        "-----END PRIVATE KEY-----\n",
    );

    #[test]
    fn stringify_payload_preserves_legacy_string_rules() {
        let data = stringify_data_payload(&serde_json::json!({
            "tipo": "mensaje_nuevo",
            "count": 3,
            "nested": { "id": 7 },
            "flag": true,
            "empty": null,
        }));

        assert_eq!(data.get("tipo").map(String::as_str), Some("mensaje_nuevo"));
        assert_eq!(data.get("count").map(String::as_str), Some("3"));
        assert_eq!(data.get("nested").map(String::as_str), Some("{\"id\":7}"));
        assert_eq!(data.get("flag").map(String::as_str), Some("true"));
        assert_eq!(data.get("empty").map(String::as_str), Some("null"));
    }

    #[test]
    fn message_type_switches_android_channel() {
        assert_eq!(
            channel_id_for_payload(&serde_json::json!({ "tipo": "mensaje_nuevo" })),
            "mensajes"
        );
        assert_eq!(
            channel_id_for_payload(&serde_json::json!({ "tipo": "like" })),
            "notificaciones"
        );
    }

    #[test]
    fn runtime_accepts_valid_service_account_json() {
        let config = AppConfig {
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
            vapid_public_key: None,
            vapid_private_key: None,
            vapid_subject: None,
            fcm_service_account_json: Some(
                serde_json::json!({
                    "project_id": "test-project",
                    "client_email": "bot@test-project.iam.gserviceaccount.com",
                    "private_key": TEST_PRIVATE_KEY_PEM,
                    "token_uri": "https://oauth2.googleapis.com/token",
                })
                .to_string(),
            ),
            smtp: None,
            scraper_secret: None,
            stripe: crate::config::StripeConfig::default(),
            allow_duplicate_uploads: false,
            rate_limit: crate::config::RateLimitConfig {
                peticiones_por_segundo: 10.0,
                rafaga: 40.0,
            },
        };

        let runtime = FcmDeliveryRuntime::from_config(&config)
            .expect("runtime válido")
            .expect("runtime habilitado");

        assert_eq!(runtime.project_id(), "test-project");
    }

    #[test]
    fn runtime_rejects_invalid_json() {
        let config = AppConfig {
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
            vapid_public_key: None,
            vapid_private_key: None,
            vapid_subject: None,
            fcm_service_account_json: Some("{".into()),
            smtp: None,
            scraper_secret: None,
            stripe: crate::config::StripeConfig::default(),
            allow_duplicate_uploads: false,
            rate_limit: crate::config::RateLimitConfig {
                peticiones_por_segundo: 10.0,
                rafaga: 40.0,
            },
        };

        let error = FcmDeliveryRuntime::from_config(&config)
            .err()
            .expect("debe fallar");
        assert!(matches!(error, FcmDeliveryRuntimeError::InvalidJson(_)));
    }
}
