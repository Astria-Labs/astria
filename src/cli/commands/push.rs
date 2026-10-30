use crate::config;
use crate::config::remotes::RemoteConfig;
use crate::network::client::AstriaClient;
use crate::network::simulation::NetworkSimulation;
use std::io::{Error, ErrorKind, Result};

pub struct PushCommand;

impl PushCommand {
    pub async fn execute(remote_name: String) -> Result<()> {
        let current_dir = std::env::current_dir()?;
        let cfg = config::CONFIG_GLOBAL.get().unwrap();
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

        println!("Pushing to remote '{}' at {}...", remote_name, url);

        if url.starts_with("local://") || url.starts_with('/') {
            println!("Simulating push to local path...");
            let _mock_bytes = NetworkSimulation::generate_mock_metadata()?;
            println!("Push successful (local simulation)!");
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
        let objects_to_send = crate::storage::load_known_commits(&current_dir)?;
        let new_commits = crate::utils::commit::CommitUtils::get_head_commit_hash(
            &current_dir.join(&cfg.config_folder),
        )?
        .into_iter()
        .collect();

        client
            .push(
                &current_dir,
                &client_id,
                &auth_token,
                &repository,
                new_commits,
                objects_to_send,
            )
            .await
            .map_err(|e| Error::new(ErrorKind::Other, format!("QUIC push failed: {}", e)))?;

        println!("Push successful! Objects synchronized via QUIC.");
        Ok(())
    }
}
