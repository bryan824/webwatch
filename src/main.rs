use std::{net::SocketAddr, sync::Arc};

use snafu::ResultExt;
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};
use webwatch::{
    config::AppConfig,
    db,
    error::{BindListenerSnafu, BuildHttpClientSnafu, ParseBindAddrSnafu, ServeSnafu},
    http::HttpState,
    scheduler, Result,
};

#[tokio::main]
async fn main() {
    init_tracing();

    if let Err(error) = run().await {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let config_path =
        std::env::var("WEBWATCH_CONFIG").unwrap_or_else(|_| "config.toml".to_string());
    let (config, targets_file) = AppConfig::load(&config_path)?;
    let targets = Arc::new(targets_file.targets);
    let config = Arc::new(config);
    let persistence: Arc<dyn db::Persistence> = Arc::from(db::connect(&config.sqlite_path).await?);
    persistence.migrate().await?;
    persistence.sync_targets(&targets).await?;
    info!(backend = db::backend_name(), "persistence backend selected");

    let client = reqwest::Client::builder()
        .user_agent(config.user_agent.clone())
        .timeout(config.http_timeout())
        .build()
        .context(BuildHttpClientSnafu)?;

    let scheduler = Arc::new(scheduler::Scheduler::new(
        config.clone(),
        persistence.clone(),
        client.clone(),
    ));
    scheduler.start(&targets).await;

    let state = HttpState {
        config: config.clone(),
        scheduler,
        db: persistence,
        client: client.clone(),
    };
    let app = webwatch::http::router(state);
    let addr: SocketAddr = config.server.bind.parse().context(ParseBindAddrSnafu {
        addr: config.server.bind.clone(),
    })?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .context(BindListenerSnafu { addr })?;
    info!(bind = %config.server.bind, ip = %addr.ip(), port = addr.port(), health = %format!("http://{addr}/health"), "webwatch listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            info!("shutdown requested");
        })
        .await
        .context(ServeSnafu)?;

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("webwatch=info,tower_http=info"));
    fmt().with_env_filter(filter).init();
}
