mod api;
mod cache;
mod config;
mod ctx;
mod embed;
mod meta;
mod model;
mod sources;
mod util;

use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "newsnow_server=info,info".into()),
        )
        .init();

    let cfg = config::Config::from_env();
    let meta = meta::Meta::load()?;
    let source_count = meta.sources.len();
    let cache = cache::Cache::open(&cfg.db_path, cfg.enable_cache)?;
    let ctx = ctx::Ctx::new();

    let app = api::router(Arc::new(api::AppState { ctx, cache, meta }));
    let addr = format!("{}:{}", cfg.host, cfg.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(
        "newsnow-server v{} listening on {addr}, {} sources, cache={} db={}",
        env!("CARGO_PKG_VERSION"),
        source_count,
        cfg.enable_cache,
        cfg.db_path
    );
    axum::serve(listener, app).await?;
    Ok(())
}
