use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Variable de entorno requerida no encontrada: {0}")]
    MissingEnvVar(String),
    #[error("Número inválido en configuración: {0}")]
    InvalidNumber(#[from] std::num::ParseIntError),
    #[error("Número inválido en configuración: {0}")]
    InvalidFloat(#[from] std::num::ParseFloatError),
}

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub from_email: String,
    pub from_name: String,
    pub secure: String,
}

#[derive(Debug, Clone, Default)]
pub struct StripePricesConfig {
    pub free: Option<String>,
    pub pro: Option<String>,
    pub premium: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct StripeConfig {
    pub secret_key: Option<String>,
    pub publishable_key: Option<String>,
    pub webhook_secret: Option<String>,
    pub connect_webhook_secret: Option<String>,
    pub prices: StripePricesConfig,
}

/* [174A-5] Configuración de la app cargada desde variables de entorno.
 * REDIS_URL es opcional para permitir desarrollo sin Redis (cache en memoria fallback). */
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub redis_url: Option<String>,
    pub jwt_secret: String,
    pub host: String,
    pub port: u16,
    pub db_max_connections: u32,
    pub db_min_connections: u32,
    /// Lista CSV de Google OAuth client_ids aceptados (web/android/ios).
    pub google_client_ids: Vec<String>,
    /// Directorio raiz para storage local (LocalFs). Default: "./uploads"
    pub storage_root: String,
    /// Backend de storage: "local" (default) o "s3" (requiere feature `s3`).
    pub storage_backend: String,
    /// Bucket S3 (requerido si storage_backend == "s3").
    pub s3_bucket: Option<String>,
    /// Endpoint URL custom para S3-compatibles (R2/MinIO). Opcional.
    pub s3_endpoint_url: Option<String>,
    /// Base URL pública opcional para construir enlaces absolutos de uploads.
    pub public_base_url: Option<String>,
    /// Secret HMAC para tickets websocket. Si no existe, cae a JWT_SECRET.
    pub ws_secret: String,
    /// URL websocket pública opcional. Si no existe, se deriva desde PUBLIC_BASE_URL.
    pub ws_public_url: Option<String>,
    /// TTL por defecto de tickets websocket, en segundos.
    pub ws_ticket_ttl_secs: i64,
    /// Clave pública VAPID opcional. Se acepta por compatibilidad con el legado.
    pub vapid_public_key: Option<String>,
    /// Clave privada VAPID opcional. Si existe, habilita Web Push.
    pub vapid_private_key: Option<String>,
    /// Subject VAPID opcional (`mailto:` o URL pública).
    pub vapid_subject: Option<String>,
    /// JSON completo de service-account para FCM HTTP v1.
    pub fcm_service_account_json: Option<String>,
    /// Configuración SMTP opcional para emails transaccionales.
    pub smtp: Option<SmtpConfig>,
    /* [174A-108b] Secret HMAC compartido con el scraper Python. Si está
     * presente, los endpoints /api/admin/scraper/{publicar-auto,reporte-lote}
     * validan el header X-Kamples-Secret (constant-time). Si es None,
     * los endpoints responden 403 — protege contra exposición accidental. */
    pub scraper_secret: Option<String>,
    /* [174A-79] Stripe queda agrupado para no seguir inflando AppConfig con
     * claves sueltas. El runtime decide si la integración queda habilitada
     * según `secret_key`, pero publishable/webhook/precios pueden faltar en local. */
    pub stripe: StripeConfig,
    /* [224A-2] En modo dev/local permite subir audios con el mismo hash sin
     * que check-duplicate devuelva possible_duplicate=true. Nunca habilitar
     * en producción — evita duplicados en el catálogo. */
    pub allow_duplicate_uploads: bool,
    /* [16A-8] Tope por IP de las rutas mutantes. Ver `handlers/rate_limit.rs`. */
    pub rate_limit: RateLimitConfig,
}

/* [16A-8] Cubo de fichas por IP: `rafaga` es cuánto se puede gastar de golpe y
 * `peticiones_por_segundo` la recarga sostenida. Los valores por defecto son
 * deliberadamente holgados — una sesión normal de la app no los roza — y
 * están pensados para frenar fuerza bruta y abuso, no para repartir caudal. */
#[derive(Debug, Clone, Copy)]
pub struct RateLimitConfig {
    pub peticiones_por_segundo: f64,
    pub rafaga: f64,
}

impl AppConfig {
    /// Carga la configuración desde variables de entorno.
    /// Requiere `DATABASE_URL` y `JWT_SECRET`. `REDIS_URL`, `HOST`, `PORT`,
    /// `DB_MAX_CONNECTIONS` y `DB_MIN_CONNECTIONS` son opcionales.
    pub fn from_env() -> Result<Self, ConfigError> {
        let jwt_secret = std::env::var("JWT_SECRET")
            .map_err(|_| ConfigError::MissingEnvVar("JWT_SECRET".into()))?;

        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .map_err(|_| ConfigError::MissingEnvVar("DATABASE_URL".into()))?,
            redis_url: std::env::var("REDIS_URL").ok(),
            jwt_secret: jwt_secret.clone(),
            host: std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "3001".to_string())
                .parse()?,
            db_max_connections: std::env::var("DB_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()?,
            db_min_connections: std::env::var("DB_MIN_CONNECTIONS")
                .unwrap_or_else(|_| "2".to_string())
                .parse()?,
            google_client_ids: std::env::var("GOOGLE_CLIENT_IDS")
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect(),
            storage_root: std::env::var("STORAGE_ROOT").unwrap_or_else(|_| "./uploads".to_string()),
            storage_backend: std::env::var("STORAGE_BACKEND")
                .unwrap_or_else(|_| "local".to_string()),
            s3_bucket: std::env::var("S3_BUCKET").ok(),
            s3_endpoint_url: std::env::var("S3_ENDPOINT_URL").ok(),
            public_base_url: std::env::var("PUBLIC_BASE_URL").ok(),
            ws_secret: std::env::var("WS_SECRET").unwrap_or(jwt_secret),
            ws_public_url: std::env::var("WS_PUBLIC_URL").ok(),
            ws_ticket_ttl_secs: std::env::var("WS_TICKET_TTL_SECS")
                .unwrap_or_else(|_| "60".to_string())
                .parse()?,
            vapid_public_key: first_env(&["VAPID_PUBLIC_KEY", "KAMPLES_VAPID_PUBLIC_KEY"]),
            vapid_private_key: first_env(&["VAPID_PRIVATE_KEY", "KAMPLES_VAPID_PRIVATE_KEY"]),
            vapid_subject: first_env(&["VAPID_SUBJECT", "KAMPLES_VAPID_SUBJECT"]),
            fcm_service_account_json: first_env(&[
                "FCM_SERVICE_ACCOUNT_JSON",
                "KAMPLES_FCM_SERVICE_ACCOUNT_JSON",
            ]),
            smtp: load_optional_smtp()?,
            stripe: load_optional_stripe(),
            scraper_secret: first_env(&["SCRAPER_SECRET", "KAMPLES_CRON_SECRET"]),
            allow_duplicate_uploads: std::env::var("ALLOW_DUPLICATE_UPLOADS").is_ok_and(|v| {
                matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes")
            }),
            rate_limit: RateLimitConfig {
                peticiones_por_segundo: env_f64("RATE_LIMIT_RPS", 10.0)?,
                rafaga: env_f64("RATE_LIMIT_RAFAGA", 40.0)?,
            },
        })
    }
}

/* [16A-8] Número decimal opcional por entorno, con defecto. */
fn env_f64(nombre: &str, defecto: f64) -> Result<f64, ConfigError> {
    match first_env(&[nombre]) {
        Some(valor) => Ok(valor.parse()?),
        None => Ok(defecto),
    }
}

fn first_env(names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        std::env::var(name)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn has_any_env(names: &[&str]) -> bool {
    names.iter().any(|name| std::env::var_os(name).is_some())
}

fn load_optional_smtp() -> Result<Option<SmtpConfig>, ConfigError> {
    let smtp_keys = [
        "SMTP_HOST",
        "SMTP_PORT",
        "SMTP_USER",
        "SMTP_PASS",
        "SMTP_PASSWORD",
        "SMTP_FROM",
        "SMTP_FROM_EMAIL",
        "SMTP_FROM_NAME",
        "SMTP_SECURE",
    ];

    if !has_any_env(&smtp_keys) {
        return Ok(None);
    }

    let host =
        first_env(&["SMTP_HOST"]).ok_or_else(|| ConfigError::MissingEnvVar("SMTP_HOST".into()))?;
    let user =
        first_env(&["SMTP_USER"]).ok_or_else(|| ConfigError::MissingEnvVar("SMTP_USER".into()))?;
    let password = first_env(&["SMTP_PASSWORD", "SMTP_PASS"])
        .ok_or_else(|| ConfigError::MissingEnvVar("SMTP_PASSWORD / SMTP_PASS".into()))?;
    let port = first_env(&["SMTP_PORT"])
        .unwrap_or_else(|| "587".to_string())
        .parse()?;
    let from_email = first_env(&["SMTP_FROM_EMAIL", "SMTP_FROM"]).unwrap_or_else(|| user.clone());
    let from_name = first_env(&["SMTP_FROM_NAME"]).unwrap_or_else(|| "Kamples".to_string());
    let secure = first_env(&["SMTP_SECURE"]).unwrap_or_else(|| "tls".to_string());

    Ok(Some(SmtpConfig {
        host,
        port,
        user,
        password,
        from_email,
        from_name,
        secure,
    }))
}

fn load_optional_stripe() -> StripeConfig {
    StripeConfig {
        secret_key: first_env(&["GLORY_STRIPE_SECRET_KEY", "STRIPE_SECRET_KEY"]),
        publishable_key: first_env(&["GLORY_STRIPE_PUBLISHABLE_KEY", "STRIPE_PUBLISHABLE_KEY"]),
        webhook_secret: first_env(&["GLORY_STRIPE_WEBHOOK_SECRET", "STRIPE_WEBHOOK_SECRET"]),
        connect_webhook_secret: first_env(&[
            "GLORY_STRIPE_CONNECT_WEBHOOK_SECRET",
            "STRIPE_CONNECT_WEBHOOK_SECRET",
        ]),
        prices: StripePricesConfig {
            free: first_env(&["GLORY_STRIPE_PRICE_FREE", "STRIPE_PRICE_FREE"]),
            pro: first_env(&["GLORY_STRIPE_PRICE_PRO", "STRIPE_PRICE_PRO"]),
            premium: first_env(&["GLORY_STRIPE_PRICE_PREMIUM", "STRIPE_PRICE_PREMIUM"]),
        },
    }
}
