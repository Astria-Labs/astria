mod parser;
mod storage;
mod vector;
mod config;
mod objects;
mod worker;
mod queue;
mod utils;
mod cli;
pub mod index;

use clap::Parser;
use crate::config::{Config};
use cli::{Cli, commands::execute};

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