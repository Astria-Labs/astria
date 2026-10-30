use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum ObjectType {
    Blob,
    Tree,
    Commit,
}
