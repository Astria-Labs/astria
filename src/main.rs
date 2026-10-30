use astria::cli::{commands::execute, Cli};
use astria::config::{self, Config};
use clap::Parser;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config {
        ignore_file: ".astriaignore".into(),
        config_folder: ".astria".into(),
    };

    config::CONFIG_GLOBAL
        .set(config)
        .ok()
        .expect("Failed to set the inicial global config.");
    let _ = rustls::crypto::ring::default_provider().install_default();

    let cli = Cli::parse();
    if let Err(e) = execute(cli.command).await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
