#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    let options = vuio::lifecycle::CliService::parse_env()?;
    vuio::lifecycle::ApplicationRunner::run(options).await
}
