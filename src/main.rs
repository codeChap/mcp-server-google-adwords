mod api;
mod auth;
mod config;
mod params;
mod server;

use anyhow::Result;
use rmcp::{ServiceExt, transport::stdio};
use tracing::info;
use tracing_subscriber::EnvFilter;

use api::GoogleAdsClient;
use auth::GoogleAuth;
use server::GoogleAdsServer;

#[tokio::main]
async fn main() -> Result<()> {
    // Tracing writes to stderr so stdout stays clean for MCP JSON-RPC.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    info!("loading config");
    let cfg = config::load()?;

    info!("initializing Google Ads MCP server");
    let http = GoogleAdsClient::build_http_client();
    let auth = GoogleAuth::new(http.clone())?;
    let client = GoogleAdsClient::new(&cfg, auth, http);
    let server = GoogleAdsServer::new(client);

    info!("starting MCP server via stdio");
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
