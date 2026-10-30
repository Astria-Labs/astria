use crate::network::protocol::{SemanticMatch, SyncMode, WireMessage};
use crate::objects::astria::astria::Astria;
use crate::objects::commit::commit::Commit;
use crate::objects::object_type::ObjectType;
use crate::objects::tree::tree::Tree;
use crate::server::db::ServerAuthDb;
use crate::vector::VectorStore;
use crate::virtual_index::is_critical_path;
use quinn::{Endpoint, ServerConfig};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::Path;

pub struct AstriaServer {
    bind_addr: SocketAddr,
}

impl AstriaServer {
    pub fn new(bind_addr: SocketAddr) -> Self {
        Self { bind_addr }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let (server_config, _cert_der) = Self::configure_server()?;
        let endpoint = Endpoint::server(server_config, self.bind_addr)?;

        println!(
            "🚀 Astria Network Daemon running on QUIC at {}",
            self.bind_addr
        );

        while let Some(conn) = endpoint.accept().await {
            tokio::spawn(async move {
                match conn.await {
                    Ok(connection) => {
                        println!("🔗 New peer connected: {}", connection.remote_address());
                        if let Err(e) = Self::handle_connection(connection).await {
                            eprintln!("❌ Connection error: {}", e);
                        }
                    }
                    Err(e) => eprintln!("⚠️ Failed to establish QUIC connection: {}", e),
                }
            });
        }

        Ok(())
    }

    async fn handle_connection(
        connection: quinn::Connection,
    ) -> Result<(), Box<dyn std::error::Error>> {
        while let Ok((mut send, mut recv)) = connection.accept_bi().await {
            let mut len_buf = [0u8; 8];
            if recv.read_exact(&mut len_buf).await.is_err() {
                break;
            }
            let payload_len = u64::from_le_bytes(len_buf) as usize;

            let mut payload_buf = vec![0u8; payload_len];
            recv.read_exact(&mut payload_buf).await?;

            let message: WireMessage = bincode::deserialize(&payload_buf)?;

            let db = ServerAuthDb::new("astria_server.db")?;

            match message {
                WireMessage::ClientHello {
                    client_id: _,
                    auth_token,
                    target_repository,
                    sync_mode,
                    known_commits,
                } => {
                    let is_authorized =
                        match db.validate_access(&auth_token, &target_repository, "read") {
                            Ok(valid) => valid,
                            Err(e) => {
                                let resp = WireMessage::ServerResponse {
                                    success: false,
                                    message: format!(
                                        "Unauthorized for repository '{}': {}",
                                        target_repository, e
                                    ),
                                    missing_objects: vec![],
                                };
                                let resp_bytes = bincode::serialize(&resp)?;
                                send.write_all(&(resp_bytes.len() as u64).to_le_bytes())
                                    .await?;
                                send.write_all(&resp_bytes).await?;
                                send.finish()?;
                                continue;
                            }
                        };
                    if !is_authorized {
                        let resp = WireMessage::ServerResponse {
                            success: false,
                            message: format!("Unauthorized for repository '{}'", target_repository),
                            missing_objects: vec![],
                        };
                        let resp_bytes = bincode::serialize(&resp)?;
                        send.write_all(&(resp_bytes.len() as u64).to_le_bytes())
                            .await?;
                        send.write_all(&resp_bytes).await?;
                        send.finish()?;
                        continue;
                    }

                    let objects_dir = std::path::Path::new(".astria/objects");
                    let missing_objects =
                        Self::select_missing_objects(objects_dir, &known_commits, sync_mode)?;

                    let resp = WireMessage::ServerResponse {
                        success: true,
                        message: "OK".to_string(),
                        missing_objects: missing_objects.clone(),
                    };
                    let resp_bytes = bincode::serialize(&resp)?;
                    send.write_all(&(resp_bytes.len() as u64).to_le_bytes())
                        .await?;
                    send.write_all(&resp_bytes).await?;

                    Self::send_objects(&mut send, &missing_objects).await?;
                    send.finish()?;
                }
                WireMessage::RequestObjects {
                    client_id: _,
                    auth_token,
                    target_repository,
                    hashes,
                } => {
                    let is_authorized =
                        match db.validate_access(&auth_token, &target_repository, "read") {
                            Ok(valid) => valid,
                            Err(e) => {
                                let resp = WireMessage::ServerResponse {
                                    success: false,
                                    message: format!(
                                        "Unauthorized for repository '{}': {}",
                                        target_repository, e
                                    ),
                                    missing_objects: vec![],
                                };
                                let resp_bytes = bincode::serialize(&resp)?;
                                send.write_all(&(resp_bytes.len() as u64).to_le_bytes())
                                    .await?;
                                send.write_all(&resp_bytes).await?;
                                send.finish()?;
                                continue;
                            }
                        };

                    if !is_authorized {
                        let resp = WireMessage::ServerResponse {
                            success: false,
                            message: format!("Unauthorized for repository '{}'", target_repository),
                            missing_objects: vec![],
                        };
                        let resp_bytes = bincode::serialize(&resp)?;
                        send.write_all(&(resp_bytes.len() as u64).to_le_bytes())
                            .await?;
                        send.write_all(&resp_bytes).await?;
                        send.finish()?;
                        continue;
                    }

                    let resp = WireMessage::ServerResponse {
                        success: true,
                        message: "OK".to_string(),
                        missing_objects: hashes.clone(),
                    };
                    let resp_bytes = bincode::serialize(&resp)?;
                    send.write_all(&(resp_bytes.len() as u64).to_le_bytes())
                        .await?;
                    send.write_all(&resp_bytes).await?;
                    Self::send_objects(&mut send, &hashes).await?;
                    send.finish()?;
                }
                WireMessage::SemanticQuery {
                    client_id: _,
                    auth_token,
                    target_repository,
                    query,
                    limit,
                } => {
                    let is_authorized =
                        match db.validate_access(&auth_token, &target_repository, "read") {
                            Ok(valid) => valid,
                            Err(e) => {
                                let resp = WireMessage::SemanticQueryResponse {
                                    success: false,
                                    message: format!(
                                        "Unauthorized for repository '{}': {}",
                                        target_repository, e
                                    ),
                                    matches: vec![],
                                };
                                let resp_bytes = bincode::serialize(&resp)?;
                                send.write_all(&(resp_bytes.len() as u64).to_le_bytes())
                                    .await?;
                                send.write_all(&resp_bytes).await?;
                                send.finish()?;
                                continue;
                            }
                        };

                    if !is_authorized {
                        let resp = WireMessage::SemanticQueryResponse {
                            success: false,
                            message: format!("Unauthorized for repository '{}'", target_repository),
                            matches: vec![],
                        };
                        let resp_bytes = bincode::serialize(&resp)?;
                        send.write_all(&(resp_bytes.len() as u64).to_le_bytes())
                            .await?;
                        send.write_all(&resp_bytes).await?;
                        send.finish()?;
                        continue;
                    }

                    let matches = Self::semantic_query(&query, limit).await?;
                    let resp = WireMessage::SemanticQueryResponse {
                        success: true,
                        message: "OK".to_string(),
                        matches,
                    };
                    let resp_bytes = bincode::serialize(&resp)?;
                    send.write_all(&(resp_bytes.len() as u64).to_le_bytes())
                        .await?;
                    send.write_all(&resp_bytes).await?;
                    send.finish()?;
                }
                WireMessage::ClientPush {
                    client_id: _,
                    auth_token,
                    target_repository,
                    new_commits: _,
                    objects_to_send,
                } => {
                    let is_authorized =
                        match db.validate_access(&auth_token, &target_repository, "write") {
                            Ok(valid) => valid,
                            Err(_e) => {
                                let resp = WireMessage::PushResponse {
                                    success: false,
                                    message: "Unauthorized".to_string(),
                                };
                                let resp_bytes = bincode::serialize(&resp)?;
                                send.write_all(&(resp_bytes.len() as u64).to_le_bytes())
                                    .await?;
                                send.write_all(&resp_bytes).await?;
                                send.finish()?;
                                continue;
                            }
                        };
                    if !is_authorized {
                        let resp = WireMessage::PushResponse {
                            success: false,
                            message: format!("Unauthorized for repository '{}'", target_repository),
                        };
                        let resp_bytes = bincode::serialize(&resp)?;
                        send.write_all(&(resp_bytes.len() as u64).to_le_bytes())
                            .await?;
                        send.write_all(&resp_bytes).await?;
                        send.finish()?;
                        continue;
                    }

                    // Recebe os objetos enviados pelo cliente e salva no CAS do servidor
                    let objects_dir = std::path::Path::new(".astria/objects");
                    std::fs::create_dir_all(&objects_dir)?;

                    for _ in 0..objects_to_send.len() {
                        let mut obj_len_buf = [0u8; 8];
                        if recv.read_exact(&mut obj_len_buf).await.is_err() {
                            break;
                        }
                        let obj_len = u64::from_le_bytes(obj_len_buf) as usize;

                        let mut obj_buf = vec![0u8; obj_len];
                        recv.read_exact(&mut obj_buf).await?;

                        let obj_msg: WireMessage = bincode::deserialize(&obj_buf)?;

                        if let WireMessage::SendObject {
                            hash: expected_hash,
                            compressed_data,
                        } = obj_msg
                        {
                            let decompressed_data = zstd::decode_all(&compressed_data[..])?;
                            let computed_hash =
                                blake3::hash(&decompressed_data).to_hex().to_string();

                            if computed_hash != expected_hash {
                                eprintln!(
                                    "Integrity check failed during push for {}",
                                    expected_hash
                                );
                                continue;
                            }

                            std::fs::write(objects_dir.join(&expected_hash), &decompressed_data)?;
                        }
                    }

                    let resp = WireMessage::PushResponse {
                        success: true,
                        message: "Push accepted".to_string(),
                    };
                    let resp_bytes = bincode::serialize(&resp)?;
                    send.write_all(&(resp_bytes.len() as u64).to_le_bytes())
                        .await?;
                    send.write_all(&resp_bytes).await?;
                    send.finish()?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn select_missing_objects(
        objects_dir: &Path,
        known_objects: &[String],
        sync_mode: SyncMode,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        if !objects_dir.exists() {
            return Ok(vec![]);
        }

        let known: HashSet<&str> = known_objects.iter().map(|s| s.as_str()).collect();
        let mut selected = HashSet::new();
        let mut critical_blobs = HashSet::new();

        for entry in std::fs::read_dir(objects_dir)? {
            let entry = entry?;
            let hash = entry.file_name().to_string_lossy().to_string();
            if known.contains(hash.as_str()) {
                continue;
            }

            match sync_mode {
                SyncMode::Full => {
                    selected.insert(hash);
                }
                SyncMode::Lazy => {
                    let Some(object) = Self::read_astria_object(&entry.path())? else {
                        continue;
                    };
                    match object.obj_type {
                        ObjectType::Commit | ObjectType::Tree => {
                            if let ObjectType::Tree = object.obj_type {
                                if let Ok(tree) = bincode::deserialize::<Tree>(&object.data) {
                                    for tree_entry in tree.entries {
                                        if is_critical_path(&tree_entry.name) {
                                            critical_blobs.insert(tree_entry.object_hash);
                                        }
                                    }
                                }
                            }
                            selected.insert(hash);
                        }
                        ObjectType::Blob => {}
                    }
                }
                SyncMode::OnDemand => {
                    let Some(object) = Self::read_astria_object(&entry.path())? else {
                        continue;
                    };
                    match object.obj_type {
                        ObjectType::Commit | ObjectType::Tree => {
                            selected.insert(hash);
                        }
                        ObjectType::Blob => {}
                    }
                }
            }
        }

        for blob_hash in critical_blobs {
            if !known.contains(blob_hash.as_str()) {
                selected.insert(blob_hash);
            }
        }

        let mut selected: Vec<String> = selected.into_iter().collect();
        selected.sort();
        Ok(selected)
    }

    async fn semantic_query(
        query: &str,
        limit: usize,
    ) -> Result<Vec<SemanticMatch>, Box<dyn std::error::Error>> {
        let vector_store = VectorStore::new(".astria/vectors").await?;
        let path_to_blob = Self::build_path_blob_index(Path::new(".astria/objects"))?;
        let results = vector_store.search_matches(query, limit).await?;

        Ok(results
            .into_iter()
            .map(|result| {
                let blob_hash = path_to_blob.get(&result.file).cloned();
                SemanticMatch {
                    file: result.file,
                    name: result.name,
                    kind: result.kind,
                    start_line: result.start_line,
                    end_line: result.end_line,
                    distance: result.distance,
                    blob_hash,
                }
            })
            .collect())
    }

    fn build_path_blob_index(
        objects_dir: &Path,
    ) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
        let mut path_to_blob = HashMap::new();
        if !objects_dir.exists() {
            return Ok(path_to_blob);
        }

        for entry in std::fs::read_dir(objects_dir)? {
            let entry = entry?;
            let Some(object) = Self::read_astria_object(&entry.path())? else {
                continue;
            };

            if let ObjectType::Commit = object.obj_type {
                let commit: Commit = bincode::deserialize(&object.data)?;
                let tree_path = objects_dir.join(commit.tree_hash);
                let Some(tree_object) = Self::read_astria_object(&tree_path)? else {
                    continue;
                };
                let tree: Tree = bincode::deserialize(&tree_object.data)?;

                for tree_entry in tree.entries {
                    path_to_blob.insert(tree_entry.name, tree_entry.object_hash);
                }
            }
        }

        Ok(path_to_blob)
    }

    fn read_astria_object(path: &Path) -> Result<Option<Astria>, Box<dyn std::error::Error>> {
        if !path.is_file() {
            return Ok(None);
        }
        let compressed = std::fs::read(path)?;
        let serialized = zstd::decode_all(&compressed[..])?;
        let object = bincode::deserialize::<Astria>(&serialized)?;
        Ok(Some(object))
    }

    async fn send_objects(
        send: &mut quinn::SendStream,
        hashes: &[String],
    ) -> Result<(), Box<dyn std::error::Error>> {
        for obj_hash in hashes {
            let obj_path = std::path::Path::new(".astria/objects").join(obj_hash);
            if obj_path.exists() {
                let raw_data = std::fs::read(&obj_path)?;
                let compressed_data = zstd::encode_all(&raw_data[..], 3)?;

                let obj_msg = WireMessage::SendObject {
                    hash: obj_hash.clone(),
                    compressed_data,
                };

                let obj_bytes = bincode::serialize(&obj_msg)?;
                send.write_all(&(obj_bytes.len() as u64).to_le_bytes())
                    .await?;
                send.write_all(&obj_bytes).await?;
            }
        }

        Ok(())
    }

    fn configure_server() -> Result<(ServerConfig, Vec<u8>), Box<dyn std::error::Error>> {
        let cert = rcgen::generate_simple_self_signed(vec!["astria.local".into()])?;
        let cert_der = cert.cert.der().to_vec();
        let priv_key = cert.key_pair.serialize_der();

        let cert_chain = vec![rustls::pki_types::CertificateDer::from(cert_der.clone())];
        let private_key = rustls::pki_types::PrivateKeyDer::Pkcs8(priv_key.into());

        let server_config = ServerConfig::with_single_cert(cert_chain, private_key)?;
        Ok((server_config, cert_der))
    }
}
