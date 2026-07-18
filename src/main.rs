#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let options = vuio::lifecycle::CliService::parse_env()?;
    vuio::lifecycle::ApplicationRunner::run(options).await
}
