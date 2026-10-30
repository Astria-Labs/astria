use crate::config;
use std::collections::HashMap;

pub struct ResetCommand;

impl ResetCommand {
    pub fn execute(path: String) -> std::io::Result<()> {
        let current_dir = std::env::current_dir()?;
        let cfg = config::CONFIG_GLOBAL.get().unwrap();
        let dir = current_dir.join(&cfg.config_folder);

        if !dir.exists() {
            println!("Error: Astria repository not initialized.");
            return Ok(());
        }

        let index_path = dir.join("index");
        if !index_path.exists() {
            println!("No changes staged (index is empty).");
            return Ok(());
        }

        let data = std::fs::read(&index_path)?;
        let mut index_map: HashMap<String, String> = bincode::deserialize(&data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

        let rel_path = path.replace('\\', "/");
        if index_map.remove(&rel_path).is_some() {
            let serialized_index = bincode::serialize(&index_map)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

            std::fs::write(&index_path, serialized_index)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

            println!("unstaged '{}'", rel_path);
        } else {
            println!("Error: File '{}' is not staged in index.", rel_path);
        }

        Ok(())
    }
}
