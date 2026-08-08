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
    let cancellation = command.runtime.cancellation.clone();
    let runtime = vuio_core::lifecycle::ApplicationRunner::run(command.runtime);
    tokio::pin!(runtime);
    tokio::select! {
        result = &mut runtime => result,
        result = wait_for_shutdown_signal() => {
            result?;
            cancellation.cancel();
            runtime.await
        }
    }
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
