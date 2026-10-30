use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    Full,
    Lazy,
    OnDemand,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SemanticMatch {
    pub file: String,
    pub name: String,
    pub kind: String,
    pub start_line: i32,
    pub end_line: i32,
    pub distance: f32,
    pub blob_hash: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum WireMessage {
    ClientHello {
        client_id: String,
        auth_token: String,
        target_repository: String,
        sync_mode: SyncMode,
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

    RequestObjects {
        client_id: String,
        auth_token: String,
        target_repository: String,
        hashes: Vec<String>,
    },

    SemanticQuery {
        client_id: String,
        auth_token: String,
        target_repository: String,
        query: String,
        limit: usize,
    },

    SemanticQueryResponse {
        success: bool,
        message: String,
        matches: Vec<SemanticMatch>,
    },

    Ack {
        message: String,
    },
}
