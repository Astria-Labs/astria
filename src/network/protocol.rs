use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum WireMessage {
    ClientHello {
        client_id: String,
        auth_token: String,
        target_repository: String,
        known_commits: Vec<String>,
    },

    ClientPush {
        client_id: String,
        auth_token: String,
        target_repository: String,
        new_commits: Vec<String>,
        objects_to_send: Vec<String>,
    },

    PushResponse {
        success: bool,
        message: String,
    },

    ServerResponse {
        success: bool,
        message: String,
        missing_objects: Vec<String>,
    },

    SendObject {
        hash: String,
        compressed_data: Vec<u8>,
    },

    Ack {
        message: String,
    },
}
