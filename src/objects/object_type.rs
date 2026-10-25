use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum ObjectType {
    Blob,
    Tree,
    Commit,
}
