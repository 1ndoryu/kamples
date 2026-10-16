use glory_backend::config::AppConfig;
use glory_backend::handlers;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use utoipa::OpenApi;

type AppError = Box<dyn std::error::Error>;

type DeliveryRuntimes = (
    Option<glory_backend::services::PushDeliveryRuntime>,
    Option<glory_backend::services::FcmDeliveryRuntime>,
    Option<glory_backend::services::EmailDeliveryRuntime>,
);

type BackgroundWorkerHandles = (
    Vec<JoinHandle<()>>,
    Vec<JoinHandle<()>>,
    JoinHandle<()>,
    JoinHandle<()>,
    JoinHandle<()>,
    JoinHandle<()>,
);

#[tokio::main]
#[allow(clippy::too_many_lines)] // bootstrap secuencial: env + pool + servicios + router
async fn main() -> Result<(), AppError> {
    dotenvy::dotenv().ok();

    /* [174A-108b] rustls 0.23 ya no auto-instala un CryptoProvider cuando hay
     * múltiples backends compilados (aws-lc-rs vs ring) en el grafo de deps.
     * Este proyecto usa aws-lc-rs vía async-stripe; lo registramos
     * explícitamente como provider por defecto del proceso para que SQLx,
     * Redis, reqwest, lettre y AWS SDK puedan negociar TLS. Si alguno de
     * estos clientes ya inicializó otro provider, ignoramos el error. */
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    if emit_openapi_if_requested()? {
        return Ok(());
    }

    init_tracing();

    let config = AppConfig::from_env()?;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(config.db_max_connections)
        .min_connections(config.db_min_connections)
        .connect(&config.database_url)
        .await?;

    // Migrations se ejecutan por separado (ya aplicadas en la DB de producción)
    // sqlx::migrate!().run(&pool).await?;

    /* [174A-5] Redis opcional. Si REDIS_URL está definido, creamos pool deadpool
     * y testeamos conexión con PING; si falla, abortamos al arrancar. */
    let redis = if let Some(url) = config.redis_url.clone() {
        let cfg = deadpool_redis::Config::from_url(url);
        let pool = cfg.create_pool(Some(deadpool_redis::Runtime::Tokio1))?;
        let mut conn = pool.get().await?;
        let _: String = redis::cmd("PING").query_async(&mut conn).await?;
        tracing::info!("Redis conectado");
        Some(pool)
    } else {
        tracing::warn!("REDIS_URL no definido — operando sin cache distribuido");
        None
    };

    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("Servidor iniciando en {addr}");
    tracing::info!("Swagger UI disponible en http://{addr}/swagger-ui/");

    /* [174A-26+174A-27] Storage backend. STORAGE_BACKEND="local" usa LocalFs (default).
     * STORAGE_BACKEND="s3" requiere compilar con `--features s3` y env S3_BUCKET. */
    let storage: Arc<dyn glory_backend::services::FileStorage> = if config.storage_backend == "s3" {
        #[cfg(feature = "s3")]
        {
            let bucket = config
                .s3_bucket
                .clone()
                .ok_or("S3_BUCKET requerido cuando STORAGE_BACKEND=s3")?;
            let endpoint = config.s3_endpoint_url.clone();
            tracing::info!("Storage S3 bucket={bucket} endpoint={endpoint:?}");
            std::sync::Arc::new(glory_backend::services::S3Storage::new(bucket, endpoint).await?)
        }
        #[cfg(not(feature = "s3"))]
        {
            return Err("STORAGE_BACKEND=s3 pero binario compilado sin feature `s3`".into());
        }
    } else {
        tracing::info!("Storage LocalFs en {}", config.storage_root);
        Arc::new(glory_backend::services::LocalFs::new(&config.storage_root).await?)
    };

    let _background_workers = spawn_background_workers(&pool, &storage, &config.storage_root);

    /* [174A-93+174A-96] AlgoPlanner periodic loop: refresca mv_trending_samples
     * (precompute_feeds) y recalcula user_tag_scores activos
     * (recompute_user_profiles). Compartido con el AppState para que el path
     * caliente (handlers de like/play/etc.) reuse el mismo planner. */
    let algo_planner = glory_backend::algorithm::AlgoPlanner::new(
        glory_backend::algorithm::AlgoPlannerConfig::legacy_defaults(),
    );
    let _algo_planner_loop = Arc::clone(&algo_planner).spawn_periodic_loop(
        pool.clone(),
        redis.clone(),
        tokio_util::sync::CancellationToken::new(),
    );

    let (push_runtime, fcm_runtime, email_runtime) = init_delivery_runtimes(&config)?;
    let stripe_runtime = glory_backend::services::StripeRuntime::from_config(&config)?;
    if let Some(runtime) = stripe_runtime.as_ref() {
        tracing::info!(
            publishable_key = runtime.publishable_key().is_some(),
            pro_price = runtime
                .price_id_for_plan(glory_backend::domain::KamplesPlanId::Pro)
                .is_some(),
            premium_price = runtime
                .price_id_for_plan(glory_backend::domain::KamplesPlanId::Premium)
                .is_some(),
            "Stripe habilitado"
        );
    } else {
        tracing::warn!("Stripe no configurado — pagos deshabilitados");
    }

    let app = handlers::create_router(
        pool,
        redis,
        config,
        storage,
        handlers::AppRuntimes {
            push: push_runtime,
            fcm: fcm_runtime,
            email: email_runtime,
            stripe: stripe_runtime,
        },
        algo_planner,
    );
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    /* [16A-8] `with_connect_info` es lo que da al rate limit la IP del peer
     * cuando el proxy no deja cabecera (ver `handlers/rate_limit.rs`). */
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    Ok(())
}

fn emit_openapi_if_requested() -> Result<bool, AppError> {
    /* [174A-6] CLI mínima: --emit-openapi <ruta> escribe el schema a disco
     * y termina sin arrancar el servidor. Usado por el frontend (Orval)
     * para regenerar el cliente sin necesidad de un backend corriendo. */
    let args: Vec<String> = std::env::args().collect();
    let Some(idx) = args.iter().position(|a| a == "--emit-openapi") else {
        return Ok(false);
    };

    let path = args
        .get(idx + 1)
        .cloned()
        .unwrap_or_else(|| "openapi.json".to_string());
    let doc = handlers::openapi::ApiDoc::openapi();
    let json = serde_json::to_string_pretty(&doc)?;
    std::fs::write(&path, json)?;
    println!("OpenAPI schema escrito en {path}");
    Ok(true)
}

fn init_tracing() {
    /* [174A-4] Tracing: EnvFilter (RUST_LOG) + formato. JSON si LOG_FORMAT=json,
     * si no formato compacto con spans para correlacionar request_id. */
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new("glory_backend=debug,tower_http=info,sqlx=warn")
    });

    let registry = tracing_subscriber::registry().with(env_filter);

    if std::env::var("LOG_FORMAT").as_deref() == Ok("json") {
        registry
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_current_span(true)
                    .with_span_list(false),
            )
            .init();
    } else {
        registry
            .with(
                tracing_subscriber::fmt::layer()
                    .compact()
                    .with_target(false),
            )
            .init();
    }
}

fn spawn_background_workers(
    pool: &sqlx::PgPool,
    storage: &Arc<dyn glory_backend::services::FileStorage>,
    storage_root: &str,
) -> BackgroundWorkerHandles {
    /* [A] Workers pesados controlados por flags:
     * - KAMPLES_WORKERS_ENABLED: audio pipeline + scraping queue (procesamiento de audio)
     * - KAMPLES_AUTOMATION_ENABLED: automation worker (scraping + extraccion)
     * En modo dev, automation OFF por defecto; audio pipeline ON. */
    let workers_enabled = std::env::var("KAMPLES_WORKERS_ENABLED")
        .ok()
        .as_deref()
        .is_none_or(|v| v != "false" && v != "0");

    let automation_enabled = std::env::var("KAMPLES_AUTOMATION_ENABLED")
        .ok()
        .as_deref()
        .is_some_and(|v| v == "true" || v == "1");

    if workers_enabled {
        (
            glory_backend::workers::spawn_audio_pipeline_workers(pool, storage),
            glory_backend::workers::spawn_ia_queue_workers(pool),
            glory_backend::workers::spawn_billing_cleanup_worker(pool),
            if automation_enabled {
                glory_backend::workers::spawn_automation_worker(pool)
            } else {
                tracing::info!("KAMPLES_AUTOMATION_ENABLED no set — automation worker desactivado");
                tokio::spawn(async {})
            },
            glory_backend::workers::spawn_scraping_queue_worker(pool),
            glory_backend::workers::spawn_cancion_image_enricher_worker(pool, storage_root),
        )
    } else {
        tracing::info!("KAMPLES_WORKERS_ENABLED=false — workers pesados desactivados (audio, scraping)");
        (
            Vec::new(),
            glory_backend::workers::spawn_ia_queue_workers(pool),
            glory_backend::workers::spawn_billing_cleanup_worker(pool),
            tokio::spawn(async {}),
            tokio::spawn(async {}),
            glory_backend::workers::spawn_cancion_image_enricher_worker(pool, storage_root),
        )
    }
}

fn init_delivery_runtimes(config: &AppConfig) -> Result<DeliveryRuntimes, AppError> {
    let push_runtime = glory_backend::services::PushDeliveryRuntime::from_config(config)?;
    if let Some(runtime) = push_runtime.as_ref() {
        tracing::info!(
            vapid_subject = %runtime.subject(),
            "Web Push VAPID habilitado"
        );
    } else {
        tracing::warn!("VAPID no configurado — Web Push deshabilitado");
    }

    let fcm_runtime = glory_backend::services::FcmDeliveryRuntime::from_config(config)?;
    if let Some(runtime) = fcm_runtime.as_ref() {
        tracing::info!(
            project_id = %runtime.project_id(),
            "FCM Android habilitado"
        );
    } else {
        tracing::warn!("FCM no configurado — Android push deshabilitado");
    }

    let email_runtime = glory_backend::services::EmailDeliveryRuntime::from_config(config)?;
    if let Some(runtime) = email_runtime.as_ref() {
        tracing::info!(
            from_email = %runtime.from_email(),
            secure = %runtime.secure_mode(),
            "SMTP habilitado"
        );
    } else {
        tracing::warn!("SMTP no configurado — emails deshabilitados");
    }

    Ok((push_runtime, fcm_runtime, email_runtime))
}
