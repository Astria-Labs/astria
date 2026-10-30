use crate::objects::astria::astria::Astria;
use crate::objects::object_type::ObjectType;
use crate::objects::tree::tree_entry::TreeEntry;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Debug)]
pub struct Tree {
    pub entries: Vec<TreeEntry>,
}

impl Tree {
    pub fn to_astria_object(&self) -> Astria {
        let data = bincode::serialize(self).unwrap();
        Astria {
            obj_type: ObjectType::Tree,
            data,
        }
    }

    pub fn save(&self, dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
        self.to_astria_object().save(dir)
    }
}
