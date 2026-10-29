use std::fs;
use std::io;
use std::path::Path;
use std::collections::HashMap;
use std::io::{Error, ErrorKind};

pub struct IndexUtils;

impl IndexUtils {

    pub fn read_index(dir: &Path) -> io::Result<HashMap<String, String>> {
        let index_path = dir.join("index");
        if !index_path.exists() {
            return Ok(HashMap::new());
        }
        let data = fs::read(&index_path)?;
        let index_map: HashMap<String, String> = bincode::deserialize(&data)
            .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))?;
        Ok(index_map)
    }

    pub fn write_index(dir: &Path, index_map: &HashMap<String, String>) -> io::Result<()> {
        let index_path = dir.join("index");
        let lock_path = dir.join("index.lock");

        if lock_path.exists() {
            return Err(Error::new(
                ErrorKind::WouldBlock,
                "Another process is currently modifying the index (index.lock exists).",
            ));
        }

        let serialized_index = bincode::serialize(index_map)
            .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))?;

        fs::write(&lock_path, serialized_index)?;
        fs::rename(&lock_path, &index_path)?;

        Ok(())
    }

    pub fn remove_index(dir: &Path) -> io::Result<()> {
        let index_path = dir.join("index");
        if index_path.exists() {
            fs::remove_file(index_path)?;
        }
        let lock_path = dir.join("index.lock");
        if lock_path.exists() {
            let _ = fs::remove_file(lock_path);
        }
        Ok(())
    }
}