use crate::config::remotes::RemoteConfig;
use crate::config::user::ClientConfig;
use crate::network::client::AstriaClient;
use std::fs;
use std::io::{Error, ErrorKind, Result};
use std::path::Path;

pub struct CloneCommand;

impl CloneCommand {
    pub async fn execute(url: String, target_dir_str: Option<String>) -> Result<()> {
        let target_name = target_dir_str.unwrap_or_else(|| {
            url.split('/')
                .last()
                .unwrap_or("astria_repo")
                .trim_end_matches(".astria")
                .to_string()
        });

        let target_path = Path::new(&target_name);
        fs::create_dir_all(target_path)?;

        let astria_dir = target_path.join(".astria");
        fs::create_dir_all(&astria_dir)?;
        fs::create_dir_all(astria_dir.join("objects"))?;
        fs::create_dir_all(astria_dir.join("vectors"))?;
        fs::write(astria_dir.join("HEAD"), "ref: refs/heads/main")?;
        fs::write(astria_dir.join("config"), ClientConfig::default_contents())?;

        let repository = target_name.clone();
        let auth_token = std::env::var("ASTRIA_AUTH_TOKEN").unwrap_or_default();
        let remotes =
            RemoteConfig::with_origin(url.clone(), repository.clone(), auth_token.clone());
        remotes.save_to_repository(target_path)?;

        println!("Cloning repository from {} into '{}'...", url, target_name);

        if url.starts_with("local://") || url.starts_with('/') {
            println!("Cloned successfully (local simulation)!");
            return Ok(());
        }

        let addr_str = url.trim_start_matches("quic://");
        let server_addr = addr_str.parse().map_err(|e| {
            Error::new(
                ErrorKind::InvalidData,
                format!("Invalid remote socket address: {}", e),
            )
        })?;

        let client = AstriaClient::new(server_addr);
        if auth_token.trim().is_empty() {
            return Err(Error::new(
                ErrorKind::NotFound,
                "ASTRIA_AUTH_TOKEN is required for QUIC clone.",
            ));
        }
        client
            .clone_repo(target_path, "cli_user", &auth_token, &repository)
            .await
            .map_err(|e| Error::new(ErrorKind::Other, format!("QUIC clone failed: {}", e)))?;

        println!("Clone completed successfully via QUIC!");
        Ok(())
    }
}
