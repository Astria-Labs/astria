use std::path::Path;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum ObjectType {
    Blob,
    Tree,
    Commit,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AstriaObject {
    pub obj_type: ObjectType,
    pub data: Vec<u8>,
}

impl AstriaObject {
    pub fn save(&self, dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
        let serialized = bincode::serialize(self).unwrap();
        let compressed = zstd::encode_all(&serialized[..], 3)?;
        
        let hash = blake3::hash(&compressed).to_hex().to_string();

        let objects_dir = dir.join("objects");
        std::fs::create_dir_all(&objects_dir)?;

        let target_path = objects_dir.join(&hash);
        std::fs::write(&target_path, compressed)?;

        Ok(hash)
    }
}

pub struct Blob {
    pub content: Vec<u8>,
}

impl Blob {
    pub fn new(content: Vec<u8>) -> Self {
        Self { content }
    }

    pub fn to_astria_object(&self) -> AstriaObject {
        let data = bincode::serialize(&self.content).unwrap();
        AstriaObject {
            obj_type: ObjectType::Blob,
            data,
        }
    }

    pub fn save(&self, dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
        self.to_astria_object().save(dir)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TreeEntry {
    pub mode: u32,
    pub name: String,
    pub object_hash: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TreeObject {
    pub entries: Vec<TreeEntry>,
}

impl TreeObject {
    pub fn to_astria_object(&self) -> AstriaObject {
        let data = bincode::serialize(self).unwrap();
        AstriaObject {
            obj_type: ObjectType::Tree,
            data,
        }
    }

    pub fn save(&self, dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
        self.to_astria_object().save(dir)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CommitObject {
    pub tree_hash: String,
    pub parent_hash: Option<String>,
    pub message: String,
    pub author: String,
    pub timestamp: u64,
}

impl CommitObject {
    pub fn new(tree_hash: String, parent_hash: Option<String>, message: String, author: String) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Self {
            tree_hash,
            parent_hash,
            message,
            author,
            timestamp,
        }
    }

    pub fn to_astria_object(&self) -> AstriaObject {
        let data = bincode::serialize(self).unwrap();
        AstriaObject {
            obj_type: ObjectType::Commit,
            data,
        }
    }
}