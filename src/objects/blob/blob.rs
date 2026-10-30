use crate::objects::astria::astria::Astria;
use crate::objects::object_type::ObjectType;
use std::path::Path;

pub struct Blob {
    pub content: Vec<u8>,
}

impl Blob {
    pub fn new(content: Vec<u8>) -> Self {
        Self { content }
    }

    pub fn to_astria_object(&self) -> Astria {
        let data = bincode::serialize(&self.content).unwrap();
        Astria {
            obj_type: ObjectType::Blob,
            data,
        }
    }

    pub fn save(&self, dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
        self.to_astria_object().save(dir)
    }
}
