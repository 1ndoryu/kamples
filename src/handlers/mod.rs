/* Cableado del router: `create_router` monta las capas y `api_routes` el
 * registro de endpoints. El documento OpenAPI vive en `handlers/openapi.rs`. */

mod administracion;
mod comunidad;
mod cuenta;
mod muestras;
mod publicaciones;
mod sistema;
pub use administracion::*;
pub use comunidad::*;
pub use cuenta::*;
pub use muestras::*;
pub use publicaciones::*;
pub use sistema::*;
mod music;
pub mod openapi;

use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::AppState;

#[derive(Clone)]
pub struct AppRuntimes {
    pub push: Option<crate::services::PushDeliveryRuntime>,
    pub fcm: Option<crate::services::FcmDeliveryRuntime>,
    pub email: Option<crate::services::EmailDeliveryRuntime>,
    pub stripe: Option<crate::services::StripeRuntime>,
}

/// Crea el router principal con CORS, tracing, Swagger UI y todas las rutas.
pub fn create_router(
    pool: sqlx::PgPool,
    redis: Option<deadpool_redis::Pool>,
    config: crate::config::AppConfig,
    storage: std::sync::Arc<dyn crate::services::FileStorage>,
    runtimes: AppRuntimes,
    algo_planner: std::sync::Arc<crate::algorithm::AlgoPlanner>,
) -> Router {
    let public_base_url = config.public_base_url.clone();
    let ws_public_url = config.ws_public_url.clone();
    let redis_url = config.redis_url.clone();
    let storage_root = config.storage_root.clone();
    let limitador = rate_limit::Limitador::desde(&config.rate_limit);
    let ws_hub = std::sync::Arc::new(glory_rs::websocket::WebSocketHub::new(
        glory_rs::websocket::HubConfig::default(),
    ));
    let ws_node_id = uuid::Uuid::new_v4();
    if redis.is_some() {
        if let Some(redis_url) = redis_url {
            crate::ws::spawn_pubsub_bridge(&redis_url, std::sync::Arc::clone(&ws_hub), ws_node_id);
        }
    }
    let state = AppState {
        pool,
        redis,
        jwt_secret: config.jwt_secret,
        ws_secret: config.ws_secret,
        google: std::sync::Arc::new(crate::services::GoogleVerifier::new(
            config.google_client_ids,
        )),
        storage,
        public_base_url,
        push_runtime: runtimes.push.map(std::sync::Arc::new),
        fcm_runtime: runtimes.fcm.map(std::sync::Arc::new),
        email_runtime: runtimes.email.map(std::sync::Arc::new),
        stripe_runtime: runtimes.stripe.map(std::sync::Arc::new),
        ws_public_url,
        ws_ticket_ttl_secs: config.ws_ticket_ttl_secs,
        ws_hub,
        ws_node_id,
        algo_planner,
        scraper_secret: config.scraper_secret,
        allow_duplicate_uploads: config.allow_duplicate_uploads,
        static_dir: Some(
            std::env::var("STATIC_DIR")
                .unwrap_or_else(|_| "frontend/public".to_string()),
        ),
        moderation: match crate::services::ModerationService::from_env() {
            Ok(svc) => Some(std::sync::Arc::new(svc)),
            Err(err) => {
                tracing::warn!(error = %err, "ModerationService not available");
                None
            }
        },
        /* [18AA-2] El mismo limitador de la capa, para que los handlers puedan
         * aplicar topes propios (p. ej. `/play` por usuario) sobre el mismo
         * estado compartido en vez de crear un contador paralelo. */
        limitador: std::sync::Arc::clone(&limitador),
    };

    /* CORS: en desarrollo se permite todo. En producción, restringir orígenes */
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    /* SPA fallback: servir archivos estáticos del frontend y fallback a index.html
     * para rutas de cliente (React Router). STATIC_DIR env var o "frontend/public". */
    let static_dir =
        std::env::var("STATIC_DIR").unwrap_or_else(|_| "frontend/public".to_string());
    let index_path = format!("{static_dir}/index.html");

    Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", openapi::ApiDoc::openapi()))
        /* [174A-6] Alias /docs → /swagger-ui/ para acceso más natural. */
        .route(
            "/docs",
            axum::routing::get(|| async { axum::response::Redirect::permanent("/swagger-ui/") }),
        )
        .nest_service("/uploads", ServeDir::new(storage_root))
        .merge(seo::routes())
        .nest("/api", api_routes(limitador))
        .fallback_service(ServeDir::new(&static_dir).fallback(ServeFile::new(index_path)))
        .layer(axum::middleware::from_fn(
            crate::middleware::request_id_middleware,
        ))
        .layer(axum::middleware::from_fn(
            crate::middleware::wp_rewrite_middleware,
        ))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}

fn api_routes(limitador: std::sync::Arc<rate_limit::Limitador>) -> Router<AppState> {
    Router::new()
        .merge(image_proxy::routes())
        .merge(health::routes())
        .merge(auth::routes())
        .merge(app_versions::routes())
        .merge(articles::routes())
        .merge(biblioteca::routes())
        .merge(feed::routes())
        .merge(sample_catalog::routes())
        .merge(samples::routes())
        .merge(extension_recorte::routes())
        .merge(plays::routes())
        .merge(likes::routes())
        .merge(messages::routes())
        .merge(notifications::routes())
        .merge(free_codes::routes())
        .merge(connect::routes())
        .merge(dashboard::routes())
        .merge(sync::routes())
        .merge(payments::routes())
        .merge(reports::routes())
        .merge(search::routes())
        .merge(seo::routes_api())
        .merge(music::routes())
        .merge(fcm::routes())
        .merge(push::routes())
        .merge(comments::routes())
        .merge(posts::routes())
        .merge(social::routes())
        .merge(downloads::routes())
        .merge(suggestions::routes())
        .merge(ws::routes())
        .merge(colecciones::routes())
        .merge(users::routes())
        .merge(metrics::routes())
        .merge(scraper_admin::routes())
        .merge(admin_config::routes())
        .merge(dev_tools::routes())
        .merge(admin::routes())
        .merge(admin_ia_queue::routes())
        .merge(admin_automatizacion::routes())
        .merge(admin_contribuciones::routes())
        .merge(admin_duplicados::routes())
        .merge(admin_experiments::routes())
        .merge(admin_moderacion::routes())
        .layer(axum::middleware::from_fn_with_state(limitador, rate_limit::limitar))
}
