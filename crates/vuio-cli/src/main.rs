mod cli;
mod update;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    let command = cli::Command::parse_env()?;
    if command.update {
        return update::update_binary().await;
    }
    // The handle owns shutdown now, so the signal path no longer needs a
    // cancellation token threaded in from outside.
    let handle = vuio_core::Runtime::start(command.runtime);
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
