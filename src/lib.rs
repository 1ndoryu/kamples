#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::ref_option)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::doc_markdown)]

pub mod algorithm;
pub mod audio;
pub mod config;
pub mod domain;
pub mod errors;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod repositories;
pub mod services;
pub mod workers;
pub mod ws;

use sqlx::PgPool;
use std::sync::Arc;

/* [174A-5+174A-21+174A-26] Estado compartido de la aplicación.
 * - `pool`: PostgreSQL (siempre presente).
 * - `redis`: pool opcional. Cuando es None, los servicios deben tener fallback
 *   en memoria (DashMap/parking_lot) o degradarse limpiamente.
 * - `jwt_secret`: clave HMAC para firmar/verificar tokens.
 * - `ws_secret`: clave HMAC dedicada para tickets websocket.
 * - `google`: verificador OAuth Google ID-token (compartido).
 * - `storage`: backend de almacenamiento (LocalFs/S3) detrás de trait.
 * - `public_base_url`: prefijo opcional para construir URLs absolutas. */
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub redis: Option<deadpool_redis::Pool>,
    pub jwt_secret: String,
    pub ws_secret: String,
    pub google: Arc<services::GoogleVerifier>,
    pub storage: Arc<dyn services::FileStorage>,
    pub public_base_url: Option<String>,
    pub push_runtime: Option<Arc<services::PushDeliveryRuntime>>,
    pub fcm_runtime: Option<Arc<services::FcmDeliveryRuntime>>,
    pub email_runtime: Option<Arc<services::EmailDeliveryRuntime>>,
    pub stripe_runtime: Option<Arc<services::StripeRuntime>>,
    pub ws_public_url: Option<String>,
    pub ws_ticket_ttl_secs: i64,
    pub ws_hub: Arc<glory_rs::websocket::WebSocketHub>,
    pub ws_node_id: uuid::Uuid,
    /* [174A-58] Planificador del algoritmo (umbrales fast/precise + recálculos). */
    pub algo_planner: Arc<algorithm::AlgoPlanner>,
    /* [174A-108b] Secret compartido con el scraper Python para autenticar
     * llamadas a /api/admin/scraper/{publicar-auto,reporte-lote}
     * sin sesión de usuario. */
    pub scraper_secret: Option<String>,
    /* [224A-2] Flag de dev: bypasa check-duplicate para permitir re-subida
     * del mismo audio. Solo activar con ALLOW_DUPLICATE_UPLOADS=true en local. */
    pub allow_duplicate_uploads: bool,
    /* [104A-5] Directorio raíz de assets estáticos (frontend/public).
     * El proxy de imágenes lo usa para servir archivos fuera de uploads/
     * (assets/, legacy-assets/). */
    pub static_dir: Option<String>,
    /* [276A-3] Servicio de moderación con visión (Groq Vision + texto).
     * Usado para moderación asíncrona de publicaciones con imágenes. */
    pub moderation: Option<Arc<services::ModerationService>>,
    /* [18AA-2] Limitador de la capa (mismo Arc que la capa por IP): los
     * handlers lo reutilizan para topes propios, p. ej. el de `/play` por
     * usuario. Ver `handlers/sistema/rate_limit.rs`. */
    pub limitador: Arc<handlers::rate_limit::Limitador>,
}
