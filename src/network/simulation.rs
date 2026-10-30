use crate::index::{Index, IndexEntry};
use std::io::{Error, ErrorKind, Result};

pub struct NetworkSimulation;

impl NetworkSimulation {
    pub fn generate_mock_metadata() -> Result<Vec<u8>> {
        let mock_entries = vec![IndexEntry {
            path: "src/phantom_file.rs".to_string(),
            hash: "blake3_mock_hash_main_001".to_string(),
            is_virtual: true,
            mode: 0o644,
            size: 2048,
        }];

        bincode::serialize(&mock_entries).map_err(|e| Error::new(ErrorKind::Other, e.to_string()))
    }

    pub fn update_local_index_with_virtuals(_metadata_bytes: &[u8]) -> Result<()> {
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
}
