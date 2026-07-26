use anyhow::Context;
use rmcp::{transport::stdio, ServiceExt};
use tracing_subscriber::{fmt, EnvFilter};
use wps_mcp_rust::server::WpsMcpServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // MCP uses stdout for protocol; logs must go to stderr.
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .with_writer(std::io::stderr)
        .init();

    let service = WpsMcpServer::new()
        .serve(stdio())
        .await
        .context("启动 MCP stdio 服务失败")?;
    service.waiting().await?;
    Ok(())
}
