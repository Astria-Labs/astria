use std::fs;
use crate::config;
use std::io::{Result, Error, ErrorKind};

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

        // Vamos armazenar as configurações de remote em um arquivo `.astria/remotes` ou dentro do config
        let remotes_path = dir.join("remotes");
        
        // Ler remotes existentes ou criar novo mapa
        let mut remotes: std::collections::HashMap<String, String> = if remotes_path.exists() {
            let data = fs::read(&remotes_path)?;
            bincode::deserialize(&data)
                .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))?
        } else {
            std::collections::HashMap::new()
        };

        if remotes.contains_key(name) && !is_update {
            return Err(Error::new(
                ErrorKind::AlreadyExists,
                format!("Remote '{}' already exists. Use 'set-url' to update.", name)
            ));
        }

        if !remotes.contains_key(name) && is_update {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("Remote '{}' does not exist. Use 'add' first.", name)
            ));
        }

        remotes.insert(name.to_string(), url.to_string());

        // Serializar e salvar de volta no disco
        let encoded = bincode::serialize(&remotes)
            .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;
        fs::write(&remotes_path, encoded)?;

        println!("Remote '{}' configured successfully with URL: {}", name, url);
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

        let remotes_path = dir.join("remotes");
        if !remotes_path.exists() {
            println!("No remotes configured.");
            return Ok(());
        }

        let data = fs::read(&remotes_path)?;
        let remotes: std::collections::HashMap<String, String> = bincode::deserialize(&data)
            .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))?;

        if remotes.is_empty() {
            println!("No remotes configured.");
        } else {
            println!("Configured remotes:");
            for (name, url) in remotes {
                println!("  {} \t{}", name, url);
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

        let remotes_path = dir.join("remotes");
        if !remotes_path.exists() {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("Remote '{}' not found.", name)
            ));
        }

        let data = fs::read(&remotes_path)?;
        let mut remotes: std::collections::HashMap<String, String> = bincode::deserialize(&data)
            .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))?;

        if remotes.remove(&name).is_some() {
            let encoded = bincode::serialize(&remotes)
                .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;
            fs::write(&remotes_path, encoded)?;
            println!("Remote '{}' removed successfully.", name);
        } else {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("Remote '{}' does not exist.", name)
            ));
        }

        Ok(())
    }
}