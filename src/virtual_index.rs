use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Error, ErrorKind, Result};
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VirtualEntry {
    pub path: String,
    pub hash: String,
    pub is_virtual: bool,
    pub mode: u32,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct VirtualIndex {
    pub entries: Vec<VirtualEntry>,
}

impl VirtualIndex {
    pub fn load(repo_dir: &Path) -> Result<Self> {
        let path = repo_dir.join("virtual_index");
        if !path.exists() {
            return Ok(Self::default());
        }

        let data = fs::read(path)?;
        bincode::deserialize(&data).map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))
    }

    pub fn save(&self, repo_dir: &Path) -> Result<()> {
        let encoded = bincode::serialize(self)
            .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))?;
        fs::write(repo_dir.join("virtual_index"), encoded)
    }

    pub fn upsert(&mut self, entry: VirtualEntry) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.path == entry.path) {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
    }
}

pub fn is_critical_path(path: &str) -> bool {
    path == "Cargo.toml"
        || path == "Cargo.lock"
        || path == "build.rs"
        || path == ".astriaignore"
        || path == "AGENTS.md"
        || path.eq_ignore_ascii_case("readme.md")
        || path.starts_with("rust-toolchain")
        || path.starts_with(".cargo/")
        || (path.starts_with("src/") && path.ends_with(".rs"))
        || (path.starts_with("tests/") && path.ends_with(".rs"))
}
