use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TreeEntry {
    pub mode: u32,
    pub name: String,
    pub object_hash: String,
}