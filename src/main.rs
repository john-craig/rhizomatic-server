use rhizomatic_server::{store::Store, web::serve};
use std::{env, net::SocketAddr};
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("rhizomatic_server=info,tower_http=info")),
        )
        .init();

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:rhizomatic-server.db?mode=rwc".to_owned());
    let bind_address: SocketAddr = env::var("BIND_ADDRESS")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_owned())
        .parse()?;

    let store = Store::new(&database_url).await?;
    serve(store, bind_address).await?;
    Ok(())
}
