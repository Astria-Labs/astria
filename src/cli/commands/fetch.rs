use crate::config;
use crate::config::remotes::RemoteConfig;
use crate::network::client::AstriaClient;
use std::io::{Error, ErrorKind, Result};

pub struct FetchCommand;

impl FetchCommand {
    pub async fn execute(remote_name: String) -> Result<()> {
        let current_dir = std::env::current_dir()?;
        let cfg = config::CONFIG_GLOBAL.get().unwrap_or_else(|| {
            panic!("Config not initialized");
        });
        let dir = current_dir.join(&cfg.config_folder);

        if !dir.exists() {
            return Err(Error::new(
                ErrorKind::NotFound,
                "Astria repository not initialized.",
            ));
        }

        let remote_config = RemoteConfig::load_from_repository(&current_dir).map_err(|e| {
            Error::new(
                ErrorKind::NotFound,
                format!("Failed to load repository config: {}", e),
            )
        })?;
        let remote = remote_config.get_remote(&remote_name)?;
        let url = &remote.url;

        println!("Fetching from remote '{}' at {}...", remote_name, url);

        if url.starts_with("local://") || url.starts_with('/') {
            println!("Simulating fetch locally...");
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
        let client_id = "cli_user".to_string();
        let (repository, auth_token) = remote_config.get_auth_data(&remote_name)?;
        let known_objects = crate::storage::load_known_commits(&current_dir)?;
        client
            .fetch(
                &current_dir,
                &client_id,
                &auth_token,
                &repository,
                known_objects,
            )
            .await
            .map_err(|e| Error::new(ErrorKind::Other, format!("QUIC fetch failed: {}", e)))?;

        println!("Fetch successful! CAS updated.");
        Ok(())
    }
}
