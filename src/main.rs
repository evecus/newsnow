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
    let cfg = config::Config::from_args();
    // 日志默认关闭，-log 开启；不读取 RUST_LOG
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(if cfg.log {
            "newsnow=info,info"
        } else {
            "off"
        }))
        .init();

    let meta = meta::Meta::load()?;
    let source_count = meta.sources.len();
    // 指定了 -db 才启用缓存
    let cache = cache::Cache::open(cfg.db_path.as_deref())?;
    let ctx = ctx::Ctx::new(cfg.token.clone());

    let app = api::router(Arc::new(api::AppState { ctx, cache, meta }));
    let addr = &cfg.addr;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(
        "newsnow-server v{} listening on {addr}, {} sources, cache={} db={}",
        env!("CARGO_PKG_VERSION"),
        source_count,
        cfg.db_path.is_some(),
        cfg.db_path.as_deref().unwrap_or("-")
    );
    axum::serve(listener, app).await?;
    Ok(())
}
