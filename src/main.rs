use rhizomatic_server::{config::Config, store::Store, web::serve};
use std::{env, path::PathBuf};
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("rhizomatic_server=info,tower_http=info")),
        )
        .init();

    let config_path = parse_config_path(env::args_os())?;
    let config = Config::from_file(&config_path)?;

    let store = Store::new(&config.database_url).await?;
    serve(store, config).await?;
    Ok(())
}

fn parse_config_path(
    mut args: impl Iterator<Item = impl Into<std::ffi::OsString>>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let _program = args.next();
    let Some(flag) = args.next() else {
        return Err("usage: rhizomatic-server --config <path-to-config.toml>".into());
    };
    let flag = PathBuf::from(flag.into());
    if flag != PathBuf::from("--config") {
        return Err("usage: rhizomatic-server --config <path-to-config.toml>".into());
    }
    let Some(path) = args.next() else {
        return Err("missing config path after --config".into());
    };
    Ok(PathBuf::from(path.into()))
}
