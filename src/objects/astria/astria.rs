use std::path::Path;
use serde::{Serialize, Deserialize};
use crate::objects::object_type::ObjectType;

#[derive(Serialize, Deserialize, Debug)]
pub struct Astria {
    pub obj_type: ObjectType,
    pub data: Vec<u8>,
}

impl Astria {

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