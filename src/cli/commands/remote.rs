use crate::config;
use crate::config::remotes::{RemoteConfig, RemoteSection};
use std::io::{Error, ErrorKind, Result};

pub struct RemoteCommand;

impl RemoteCommand {
    pub fn add(name: String, url: String) -> Result<()> {
        Self::save_remote(&name, &url, false)
    }

    pub fn set_url(name: String, url: String) -> Result<()> {
        Self::save_remote(&name, &url, true)
    }

    fn save_remote(name: &str, url: &str, is_update: bool) -> Result<()> {
        let current_dir = std::env::current_dir()?;
        let cfg = config::CONFIG_GLOBAL.get().unwrap();
        let dir = current_dir.join(&cfg.config_folder);

        if !dir.exists() {
            println!("Error: Astria repository not initialized.");
            return Ok(());
        }

        let mut config = RemoteConfig::load_from_repository(&current_dir)?;

        let existing_is_empty = config
            .remotes
            .get(name)
            .map(|remote| {
                remote.url.trim().is_empty()
                    && remote.repository.trim().is_empty()
                    && remote.auth_token.trim().is_empty()
            })
            .unwrap_or(false);

        if config.remotes.contains_key(name) && !is_update && !existing_is_empty {
            return Err(Error::new(
                ErrorKind::AlreadyExists,
                format!("Remote '{}' already exists. Use 'set-url' to update.", name),
            ));
        }

        if !config.remotes.contains_key(name) && is_update {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("Remote '{}' does not exist. Use 'add' first.", name),
            ));
        }

        let remote = config
            .remotes
            .entry(name.to_string())
            .or_insert_with(RemoteSection::default);
        remote.url = url.to_string();

        config.save_to_repository(&current_dir)?;

        println!(
            "Remote '{}' configured successfully with URL: {}",
            name, url
        );
        Ok(())
    }

    pub fn list() -> Result<()> {
        let current_dir = std::env::current_dir()?;
        let cfg = config::CONFIG_GLOBAL.get().unwrap();
        let dir = current_dir.join(&cfg.config_folder);

        if !dir.exists() {
            println!("Error: Astria repository not initialized.");
            return Ok(());
        }

        let config = RemoteConfig::load_from_repository(&current_dir)?;

        if config.remotes.is_empty() {
            println!("No remotes configured.");
        } else {
            println!("Configured remotes:");
            let mut remotes: Vec<_> = config.remotes.iter().collect();
            remotes.sort_by(|a, b| a.0.cmp(b.0));
            for (name, remote) in remotes {
                println!("  {} \t{}", name, remote.url);
            }
        }

        Ok(())
    }

    pub fn remove(name: String) -> Result<()> {
        let current_dir = std::env::current_dir()?;
        let cfg = config::CONFIG_GLOBAL.get().unwrap();
        let dir = current_dir.join(&cfg.config_folder);

        if !dir.exists() {
            println!("Error: Astria repository not initialized.");
            return Ok(());
        }

        let mut config = RemoteConfig::load_from_repository(&current_dir)?;

        if config.remotes.remove(&name).is_some() {
            config.save_to_repository(&current_dir)?;
            println!("Remote '{}' removed successfully.", name);
        } else {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("Remote '{}' does not exist.", name),
            ));
        }

        Ok(())
    }
}
