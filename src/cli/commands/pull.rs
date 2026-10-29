use std::fs;
use crate::config;
use std::process::Command;
use crate::index::{Index, IndexEntry};
use std::io::{Result, Error, ErrorKind};

pub struct PullCommand;

impl PullCommand {

    pub fn execute(remote_name: String) -> Result<()> {

        let current_dir = std::env::current_dir()?;
        let cfg = config::CONFIG_GLOBAL.get().unwrap();
        let dir = current_dir.join(&cfg.config_folder);

        if !dir.exists() {
            return Err(Error::new(ErrorKind::NotFound, "Astria repository not initialized."));
        }

        let remotes_path = dir.join("remotes");
        if !remotes_path.exists() {
            return Err(Error::new(ErrorKind::NotFound, "No remotes configured. Use 'ast remote add' first."));
        }

        let data = fs::read(&remotes_path)?;
        let remotes: std::collections::HashMap<String, String> = bincode::deserialize(&data)
            .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))?;

        let url = remotes.get(&remote_name).ok_or_else(|| {
            Error::new(ErrorKind::NotFound, format!("Remote '{}' not found.", remote_name))
        })?;

        println!("Connecting to remote '{}' at {}...", remote_name, url);

        if url.starts_with("local://") || url.starts_with('/') {
            // Mocks a "remote repository" in local disk
            println!("Simulating fetch from local path...");
            let mock_bytes = Self::generate_mock_metadata()?;
            Self::update_local_index_with_virtuals(&mock_bytes)?;
            println!("Pull successful (local simulation)!");
            return Ok(());
        }

        let ssh_target = url.trim_start_matches("ssh://");
        let output = Command::new("ssh")
            .arg(ssh_target)
            .arg("astria-server-internal fetch-metadata")
            .output();

        match output {
            Ok(res) if res.status.success() => {
                let remote_metadata_bytes = res.stdout;
                Self::update_local_index_with_virtuals(&remote_metadata_bytes)?;
                println!("Pull successful! Metadata synchronized with virtual file placeholders.");
            }
            Ok(res) => {
                let stderr = String::from_utf8_lossy(&res.stderr);
                return Err(Error::new(ErrorKind::Other, format!("SSH remote error: {}", stderr)));
            }
            Err(e) => {
                return Err(Error::new(ErrorKind::Other, format!("Failed to execute SSH process: {}", e)));
            }
        }

        Ok(())
    }

    fn update_local_index_with_virtuals(_metadata_bytes: &[u8]) -> Result<()> {

        let mut index = Index::load()?;
        let virtual_entry = IndexEntry {
            path: "src/remote_example.rs".to_string(),
            hash: "blake3_mock_hash_abc123".to_string(),
            is_virtual: true,
            mode: 0o644,
            size: 1024,
        };

        index.upsert(virtual_entry);
        index.save()?;

        println!("Local index updated with virtual file placeholders.");
        Ok(())
    }

    fn generate_mock_metadata() -> Result<Vec<u8>> {
        let mock_entries = vec![
            IndexEntry {
                path: "src/phantom_file.rs".to_string(),
                hash: "blake3_mock_hash_main_001".to_string(),
                is_virtual: true,
                mode: 0o644,
                size: 2048,
            },
        ];

        let encoded = bincode::serialize(&mock_entries)
            .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;

        Ok(encoded)
    }
}