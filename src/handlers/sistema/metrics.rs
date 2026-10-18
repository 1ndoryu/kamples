/* [174A-97] /metrics endpoint Prometheus text. */

use crate::repositories::AdminPanelRepository;
use crate::AppState;
use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use std::fmt::Write;

pub async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    let pool = &state.pool;
    let pool_size = i64::from(pool.size());
    let pool_idle = i64::try_from(pool.num_idle()).unwrap_or(i64::MAX);
    let pool_in_use = pool_size - pool_idle;
    let redis_enabled = i64::from(state.redis.is_some());
    let stripe_enabled = i64::from(state.stripe_runtime.is_some());

    let conteos = AdminPanelRepository::conteos_metricas(pool).await;
    let ia_pending = conteos.ia_pendientes;
    let scraping_pending = conteos.scraping_pendientes;
    let suscripciones_activas = conteos.suscripciones_activas;

    let mut body = String::with_capacity(1024);
    let version = env!("CARGO_PKG_VERSION");
    let _ = writeln!(body, "# TYPE kamples_build_info gauge");
    let _ = writeln!(body, "kamples_build_info{{version=\"{version}\"}} 1");
    let _ = writeln!(body, "# TYPE kamples_db_pool_size gauge");
    let _ = writeln!(body, "kamples_db_pool_size {pool_size}");
    let _ = writeln!(body, "# TYPE kamples_db_pool_idle gauge");
    let _ = writeln!(body, "kamples_db_pool_idle {pool_idle}");
    let _ = writeln!(body, "# TYPE kamples_db_pool_in_use gauge");
    let _ = writeln!(body, "kamples_db_pool_in_use {pool_in_use}");
    let _ = writeln!(body, "# TYPE kamples_redis_enabled gauge");
    let _ = writeln!(body, "kamples_redis_enabled {redis_enabled}");
    let _ = writeln!(body, "# TYPE kamples_stripe_enabled gauge");
    let _ = writeln!(body, "kamples_stripe_enabled {stripe_enabled}");
    let _ = writeln!(body, "# TYPE kamples_ia_queue_pending gauge");
    let _ = writeln!(body, "kamples_ia_queue_pending {ia_pending}");
    let _ = writeln!(body, "# TYPE kamples_scraping_pending gauge");
    let _ = writeln!(body, "kamples_scraping_pending {scraping_pending}");
    let _ = writeln!(body, "# TYPE kamples_subscriptions_active gauge");
    let _ = writeln!(body, "kamples_subscriptions_active {suscripciones_activas}");

    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4")], body)
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/metrics", get(metrics))
}
