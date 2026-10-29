use astria::config;
use astria::config::{Config};
use astria::cli::{Cli, commands::execute};
use clap::Parser;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let config = Config {
        ignore_file: ".astriaignore".into(),
        config_folder: ".astria".into() 
    };

    config::CONFIG_GLOBAL.set(config).ok().expect("Failed to set the inicial global config.");

    let cli = Cli::parse();
    if let Err(e) = execute(cli.command).await {
        eprintln!("Erro: {}", e);
        std::process::exit(1);
    }

    Ok(())
}