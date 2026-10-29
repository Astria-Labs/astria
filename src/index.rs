use std::fs;
use crate::config;
use serde::{Serialize, Deserialize};
use std::io::{Result, Error, ErrorKind};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IndexEntry {
    pub path: String,
    pub hash: String,
    pub is_virtual: bool,
    pub mode: u32,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Index {
    pub entries: Vec<IndexEntry>,
}

impl Index {
    /// Carrega o index local (.astria/index)
    pub fn load() -> Result<Self> {
        let current_dir = std::env::current_dir()?;
        let cfg = match config::CONFIG_GLOBAL.get() {
            Some(c) => c,
            None => return Ok(Self::default()),
        };

        let index_path = current_dir.join(&cfg.config_folder).join("index");

        if !index_path.exists() {
            return Ok(Self::default());
        }

        let data = fs::read(&index_path)?;
        let entries: Vec<IndexEntry> = bincode::deserialize(&data)
            .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))?;

        Ok(Self { entries })
    }

    /// Salva o index de volta em disco (.astria/index)
    pub fn save(&self) -> Result<()> {
        let current_dir = std::env::current_dir()?;
        let cfg = config::CONFIG_GLOBAL.get().ok_or_else(|| {
            Error::new(ErrorKind::NotFound, "Astria configuration not loaded.")
        })?;

        let dir = current_dir.join(&cfg.config_folder);
        if !dir.exists() {
            return Err(Error::new(ErrorKind::NotFound, "Astria repository not initialized."));
        }

        let index_path = dir.join("index");
        let encoded = bincode::serialize(&self.entries)
            .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;

        fs::write(&index_path, encoded)?;
        Ok(())
    }

    /// Adiciona ou atualiza uma entrada no índice
    pub fn upsert(&mut self, entry: IndexEntry) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.path == entry.path) {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
    }
}