use tactica_server::{AppState, Config, router};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn")),
        )
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()))
        .init();

    let config = Config::from_env();
    let state = AppState::new(config.clone()).await?;
    if let Some(dir) = config.backup_dir.clone() {
        let (db, keep) = (state.db.clone(), config.backup_keep);
        let every = std::time::Duration::from_secs(config.backup_every_hours * 3600);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(every);
            loop {
                tick.tick().await;
                match tactica_server::db::backup(&db, &dir, keep).await {
                    Ok(p) => tracing::info!(path = %p, "backup completato"),
                    Err(e) => tracing::error!("backup fallito: {e}"),
                }
            }
        });
    }
    let listener = TcpListener::bind(&config.addr).await?;
    tracing::info!(addr = %listener.local_addr()?, db = %config.db_url, "server in ascolto");

    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown())
        .await?;
    tracing::info!("arresto completato");
    Ok(())
}

async fn shutdown() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut term = signal(SignalKind::terminate()).expect("gestore SIGTERM");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = term.recv() => {}
    }
}
