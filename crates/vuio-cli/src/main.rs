mod cli;
mod mcp;
mod update;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    let (runtime, update) = match cli::Command::parse_env()? {
        // The proxy owns no listener and no database, so it neither starts the
        // runtime nor waits for a shutdown signal: it ends when its client
        // closes the pipe.
        cli::Command::Mcp(options) => return mcp::run(options).await,
        cli::Command::Serve { runtime, update } => (runtime, update),
    };
    if update {
        return update::update_binary().await;
    }
    // The handle owns shutdown now, so the signal path no longer needs a
    // cancellation token threaded in from outside.
    let handle = vuio_core::Runtime::start(runtime);
    tokio::select! {
        result = handle.wait() => result?,
        result = wait_for_shutdown_signal() => {
            result?;
            handle.shutdown().await?;
        }
    }
    Ok(())
}

async fn wait_for_shutdown_signal() -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut terminate = signal(SignalKind::terminate())?;
        let mut interrupt = signal(SignalKind::interrupt())?;
        tokio::select! {
            _ = terminate.recv() => {},
            _ = interrupt.recv() => {},
        }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c().await?;
    Ok(())
}
