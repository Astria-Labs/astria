use serde::{Serialize, Deserialize};
use crate::objects::astria::astria::Astria;
use crate::objects::object_type::ObjectType;

#[derive(Serialize, Deserialize, Debug)]
pub struct Commit {
    pub tree_hash: String,
    pub parent_hash: Option<String>,
    pub message: String,
    pub author: String,
    pub timestamp: u64,
}

impl Commit {
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

    pub fn to_astria_object(&self) -> Astria {
        let data = bincode::serialize(self).unwrap();
        Astria {
            obj_type: ObjectType::Commit,
            data,
        }
    }
}