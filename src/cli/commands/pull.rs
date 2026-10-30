use crate::config;
use crate::config::remotes::RemoteConfig;
use crate::config::user::ClientConfig;
use crate::network::client::AstriaClient;
use crate::network::protocol::SyncMode;
use crate::network::simulation::NetworkSimulation;
use crate::queue::TaskQueue;
use crate::virtual_index::VirtualIndex;
use crate::worker::BackgroundWorker;
use std::io::{Error, ErrorKind, Result};

pub struct PullCommand;

impl PullCommand {
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

        println!("Connecting to remote '{}' at {}...", remote_name, url);

        if url.starts_with("local://") || url.starts_with('/') {
            // Mocks a "remote repository" in local disk
            println!("Simulating fetch from local path...");
            let mock_bytes = NetworkSimulation::generate_mock_metadata()?;
            NetworkSimulation::update_local_index_with_virtuals(&mock_bytes)?;
            println!("Pull successful (local simulation)!");
            return Ok(());
        }

        let addr_str = url.trim_start_matches("quic://");
        let server_addr = addr_str.parse().map_err(|e| {
            Error::new(
                ErrorKind::InvalidData,
                format!("Invalid remote socket address: {}", e),
            )
        })?;

        let client_id = "cli_user".to_string();
        let (repository, auth_token) = remote_config.get_auth_data(&remote_name)?;
        let mode = ClientConfig::load_from_repository(&current_dir)
            .map(|config| config.checkout_mode())
            .unwrap_or_else(|_| "full".to_string());
        let sync_mode = match mode.as_str() {
            "lazy" => SyncMode::Lazy,
            "ondemand" => SyncMode::OnDemand,
            _ => SyncMode::Full,
        };

        let known_commits = crate::storage::load_known_commits(&current_dir)?;

        let client = AstriaClient::new(server_addr);
        client
            .pull(
                &current_dir,
                &client_id,
                &auth_token,
                &repository,
                sync_mode,
                known_commits,
            )
            .await
            .map_err(|e| Error::new(ErrorKind::Other, format!("QUIC pull failed: {}", e)))?;

        if sync_mode == SyncMode::Lazy {
            let index = VirtualIndex::load(&dir)?;
            for entry in index.entries.iter().filter(|entry| entry.is_virtual) {
                TaskQueue::push_hydrate(
                    &dir,
                    &remote_name,
                    url,
                    &repository,
                    &auth_token,
                    &entry.path,
                    &entry.hash,
                )?;
            }
            BackgroundWorker::spawn(&dir);
        }

        println!("Pull successful! Objects synchronized via QUIC to CAS.");

        Ok(())
    }
}
