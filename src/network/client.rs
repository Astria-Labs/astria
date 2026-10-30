use crate::network::protocol::{SemanticMatch, SyncMode, WireMessage};
use crate::objects::astria::astria::Astria;
use crate::objects::commit::commit::Commit;
use crate::objects::object_type::ObjectType;
use crate::objects::tree::tree::Tree;
use crate::virtual_index::{is_critical_path, VirtualEntry, VirtualIndex};
use quinn::{ClientConfig as QuinnClientConfig, Endpoint};
use std::fs;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

pub struct AstriaClient {
    server_addr: SocketAddr,
}

impl AstriaClient {
    pub fn new(server_addr: SocketAddr) -> Self {
        Self { server_addr }
    }

    pub async fn pull(
        &self,
        repo_path: &Path,
        client_id: &str,
        auth_token: &str,
        target_repository: &str,
        sync_mode: SyncMode,
        known_commits: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 1. Bind local endpoint and configure client to trust self-signed certs (for development/local daemon)
        let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
        let crypto = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
            .with_no_client_auth();

        endpoint.set_default_client_config(QuinnClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(crypto)?,
        )));

        println!("Connecting to Astria server at {}...", self.server_addr);
        let connection = endpoint.connect(self.server_addr, "astria.local")?.await?;

        // 2. Open a bidirectional stream with the server
        let (mut send, mut recv) = connection.open_bi().await?;

        // 3. Send ClientHello containing our local DAG state
        let hello_msg = WireMessage::ClientHello {
            client_id: client_id.to_string(),
            auth_token: auth_token.to_string(),
            target_repository: target_repository.to_string(),
            sync_mode,
            known_commits,
        };
        let payload = bincode::serialize(&hello_msg)?;

        // Write message length prefix followed by payload bytes
        send.write_all(&(payload.len() as u64).to_le_bytes())
            .await?;
        send.write_all(&payload).await?;
        send.finish()?;

        // 4. Read Server Response (Missing objects needed to sync DAG)
        let mut len_buf = [0u8; 8];
        recv.read_exact(&mut len_buf).await?;
        let resp_len = u64::from_le_bytes(len_buf) as usize;

        let mut resp_buf = vec![0u8; resp_len];
        recv.read_exact(&mut resp_buf).await?;

        let response: WireMessage = bincode::deserialize(&resp_buf)?;

        match response {
            WireMessage::ServerResponse {
                success,
                message,
                missing_objects,
            } => {
                if !success {
                    return Err(format!("Server rejected pull: {}", message).into());
                }

                for _ in 0..missing_objects.len() {
                    let mut obj_len_buf = [0u8; 8];
                    recv.read_exact(&mut obj_len_buf).await?;
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
                        let computed_hash = blake3::hash(&decompressed_data).to_hex().to_string();

                        if computed_hash != expected_hash {
                            return Err(
                                format!("Integrity check failed for {}", expected_hash).into()
                            );
                        }

                        let objects_dir = repo_path.join(".astria").join("objects");
                        fs::create_dir_all(&objects_dir)?;
                        fs::write(objects_dir.join(&expected_hash), &decompressed_data)?;
                    }
                }
            }
            _ => eprintln!("Unexpected server response."),
        }

        connection.close(0u32.into(), b"Sync complete");
        if sync_mode != SyncMode::Full {
            Self::refresh_virtual_index(repo_path, sync_mode == SyncMode::Lazy)?;
        }
        Ok(())
    }

    pub async fn push(
        &self,
        repo_path: &Path,
        client_id: &str,
        auth_token: &str,
        target_repository: &str,
        new_commits: Vec<String>,
        objects_to_send: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
        let crypto = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
            .with_no_client_auth();

        endpoint.set_default_client_config(QuinnClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(crypto)?,
        )));

        let connection = endpoint.connect(self.server_addr, "astria.local")?.await?;
        let (mut send, mut recv) = connection.open_bi().await?;

        // 1. Envia a intenção de push
        let push_msg = WireMessage::ClientPush {
            client_id: client_id.to_string(),
            auth_token: auth_token.to_string(),
            target_repository: target_repository.to_string(),
            new_commits,
            objects_to_send: objects_to_send.clone(),
        };
        let payload = bincode::serialize(&push_msg)?;

        send.write_all(&(payload.len() as u64).to_le_bytes())
            .await?;
        send.write_all(&payload).await?;

        // 2. Envia cada objeto do CAS compactado via Zstd
        let objects_dir = repo_path.join(".astria").join("objects");
        for obj_hash in objects_to_send {
            let obj_path = objects_dir.join(&obj_hash);
            if obj_path.exists() {
                let raw_data = fs::read(&obj_path)?;
                let compressed_data = zstd::encode_all(&raw_data[..], 3)?;

                let obj_msg = WireMessage::SendObject {
                    hash: obj_hash,
                    compressed_data,
                };
                let obj_bytes = bincode::serialize(&obj_msg)?;

                send.write_all(&(obj_bytes.len() as u64).to_le_bytes())
                    .await?;
                send.write_all(&obj_bytes).await?;
            }
        }
        send.finish()?;

        // 3. Lê a resposta do servidor
        let mut len_buf = [0u8; 8];
        recv.read_exact(&mut len_buf).await?;
        let resp_len = u64::from_le_bytes(len_buf) as usize;

        let mut resp_buf = vec![0u8; resp_len];
        recv.read_exact(&mut resp_buf).await?;

        let response: WireMessage = bincode::deserialize(&resp_buf)?;

        match response {
            WireMessage::PushResponse { success, message } => {
                if !success {
                    return Err(format!("Server rejected push: {}", message).into());
                }
                println!("🚀 Push completed successfully!");
            }
            _ => return Err("Unexpected server response".into()),
        }

        connection.close(0u32.into(), b"Push complete");
        Ok(())
    }

    pub async fn fetch(
        &self,
        repo_path: &Path,
        client_id: &str,
        auth_token: &str,
        target_repository: &str,
        sync_mode: SyncMode,
        known_commits: Vec<String>,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        self.pull_internal(
            repo_path,
            client_id,
            auth_token,
            target_repository,
            sync_mode,
            known_commits,
            false,
        )
        .await
    }

    pub async fn clone_repo(
        &self,
        target_path: &Path,
        client_id: &str,
        auth_token: &str,
        target_repository: &str,
        sync_mode: SyncMode,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let objects_dir = target_path.join(".astria").join("objects");
        fs::create_dir_all(&objects_dir)?;
        self.pull_internal(
            target_path,
            client_id,
            auth_token,
            target_repository,
            sync_mode,
            vec![],
            true,
        )
        .await?;
        Ok(())
    }

    async fn pull_internal(
        &self,
        repo_path: &Path,
        client_id: &str,
        auth_token: &str,
        target_repository: &str,
        sync_mode: SyncMode,
        known_commits: Vec<String>,
        _is_clone: bool,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
        let crypto = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
            .with_no_client_auth();

        endpoint.set_default_client_config(QuinnClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(crypto)?,
        )));

        let connection = endpoint.connect(self.server_addr, "astria.local")?.await?;
        let (mut send, mut recv) = connection.open_bi().await?;

        let hello_msg = WireMessage::ClientHello {
            client_id: client_id.to_string(),
            auth_token: auth_token.to_string(),
            target_repository: target_repository.to_string(),
            sync_mode,
            known_commits,
        };
        let payload = bincode::serialize(&hello_msg)?;

        send.write_all(&(payload.len() as u64).to_le_bytes())
            .await?;
        send.write_all(&payload).await?;
        send.finish()?;

        let mut len_buf = [0u8; 8];
        recv.read_exact(&mut len_buf).await?;
        let resp_len = u64::from_le_bytes(len_buf) as usize;

        let mut resp_buf = vec![0u8; resp_len];
        recv.read_exact(&mut resp_buf).await?;

        let response: WireMessage = bincode::deserialize(&resp_buf)?;
        let mut downloaded_hashes = vec![];

        match response {
            WireMessage::ServerResponse {
                success,
                message,
                missing_objects,
            } => {
                if !success {
                    return Err(format!("Server rejected sync: {}", message).into());
                }

                for _ in 0..missing_objects.len() {
                    let mut obj_len_buf = [0u8; 8];
                    recv.read_exact(&mut obj_len_buf).await?;
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
                        let computed_hash = blake3::hash(&decompressed_data).to_hex().to_string();

                        if computed_hash != expected_hash {
                            return Err(
                                format!("Integrity check failed for {}", expected_hash).into()
                            );
                        }

                        let objects_dir = repo_path.join(".astria").join("objects");
                        fs::create_dir_all(&objects_dir)?;
                        fs::write(objects_dir.join(&expected_hash), &decompressed_data)?;
                        downloaded_hashes.push(expected_hash);
                    }
                }
            }
            _ => return Err("Unexpected server response".into()),
        }

        connection.close(0u32.into(), b"Sync complete");
        if sync_mode != SyncMode::Full {
            Self::refresh_virtual_index(repo_path, sync_mode == SyncMode::Lazy)?;
        }
        Ok(downloaded_hashes)
    }

    pub async fn semantic_query(
        &self,
        client_id: &str,
        auth_token: &str,
        target_repository: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SemanticMatch>, Box<dyn std::error::Error>> {
        let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
        let crypto = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
            .with_no_client_auth();

        endpoint.set_default_client_config(QuinnClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(crypto)?,
        )));

        let connection = endpoint.connect(self.server_addr, "astria.local")?.await?;
        let (mut send, mut recv) = connection.open_bi().await?;

        let request = WireMessage::SemanticQuery {
            client_id: client_id.to_string(),
            auth_token: auth_token.to_string(),
            target_repository: target_repository.to_string(),
            query: query.to_string(),
            limit,
        };
        let payload = bincode::serialize(&request)?;
        send.write_all(&(payload.len() as u64).to_le_bytes())
            .await?;
        send.write_all(&payload).await?;
        send.finish()?;

        let mut len_buf = [0u8; 8];
        recv.read_exact(&mut len_buf).await?;
        let resp_len = u64::from_le_bytes(len_buf) as usize;

        let mut resp_buf = vec![0u8; resp_len];
        recv.read_exact(&mut resp_buf).await?;

        let response: WireMessage = bincode::deserialize(&resp_buf)?;
        connection.close(0u32.into(), b"Semantic query complete");

        match response {
            WireMessage::SemanticQueryResponse {
                success,
                message,
                matches,
            } => {
                if !success {
                    return Err(format!("Server rejected semantic query: {}", message).into());
                }
                Ok(matches)
            }
            _ => Err("Unexpected server response".into()),
        }
    }

    pub async fn fetch_objects(
        &self,
        repo_path: &Path,
        client_id: &str,
        auth_token: &str,
        target_repository: &str,
        hashes: Vec<String>,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
        let crypto = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
            .with_no_client_auth();

        endpoint.set_default_client_config(QuinnClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(crypto)?,
        )));

        let connection = endpoint.connect(self.server_addr, "astria.local")?.await?;
        let (mut send, mut recv) = connection.open_bi().await?;

        let request = WireMessage::RequestObjects {
            client_id: client_id.to_string(),
            auth_token: auth_token.to_string(),
            target_repository: target_repository.to_string(),
            hashes,
        };
        let payload = bincode::serialize(&request)?;
        send.write_all(&(payload.len() as u64).to_le_bytes())
            .await?;
        send.write_all(&payload).await?;
        send.finish()?;

        Self::receive_objects(repo_path, &mut recv).await
    }

    async fn receive_objects(
        repo_path: &Path,
        recv: &mut quinn::RecvStream,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut len_buf = [0u8; 8];
        recv.read_exact(&mut len_buf).await?;
        let resp_len = u64::from_le_bytes(len_buf) as usize;

        let mut resp_buf = vec![0u8; resp_len];
        recv.read_exact(&mut resp_buf).await?;

        let response: WireMessage = bincode::deserialize(&resp_buf)?;
        let mut downloaded_hashes = vec![];

        let missing_objects = match response {
            WireMessage::ServerResponse {
                success,
                message,
                missing_objects,
            } => {
                if !success {
                    return Err(format!("Server rejected object request: {}", message).into());
                }
                missing_objects
            }
            _ => return Err("Unexpected server response".into()),
        };

        for _ in 0..missing_objects.len() {
            let mut obj_len_buf = [0u8; 8];
            recv.read_exact(&mut obj_len_buf).await?;
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
                let computed_hash = blake3::hash(&decompressed_data).to_hex().to_string();

                if computed_hash != expected_hash {
                    return Err(format!("Integrity check failed for {}", expected_hash).into());
                }

                let objects_dir = repo_path.join(".astria").join("objects");
                fs::create_dir_all(&objects_dir)?;
                fs::write(objects_dir.join(&expected_hash), &decompressed_data)?;
                downloaded_hashes.push(expected_hash);
            }
        }

        Ok(downloaded_hashes)
    }

    fn refresh_virtual_index(
        repo_path: &Path,
        materialize_critical: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let repo_dir = repo_path.join(".astria");
        let objects_dir = repo_dir.join("objects");
        if !objects_dir.exists() {
            return Ok(());
        }

        let mut index = VirtualIndex::default();

        for entry in fs::read_dir(&objects_dir)? {
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
                    let blob_exists = objects_dir.join(&tree_entry.object_hash).exists();
                    index.upsert(VirtualEntry {
                        path: tree_entry.name,
                        hash: tree_entry.object_hash,
                        is_virtual: !blob_exists,
                        mode: tree_entry.mode,
                        size: 0,
                    });
                }
            }
        }

        index.save(&repo_dir)?;
        if materialize_critical {
            for entry in index
                .entries
                .iter()
                .filter(|entry| !entry.is_virtual && is_critical_path(&entry.path))
            {
                Self::materialize_blob(repo_path, &entry.path, &entry.hash)?;
            }
        }
        Ok(())
    }

    fn read_astria_object(path: &Path) -> Result<Option<Astria>, Box<dyn std::error::Error>> {
        if !path.is_file() {
            return Ok(None);
        }
        let compressed = fs::read(path)?;
        let serialized = zstd::decode_all(&compressed[..])?;
        let object = bincode::deserialize::<Astria>(&serialized)?;
        Ok(Some(object))
    }

    pub fn materialize_blob(
        repo_path: &Path,
        path: &str,
        blob_hash: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let blob_path = repo_path.join(".astria").join("objects").join(blob_hash);
        let Some(object) = Self::read_astria_object(&blob_path)? else {
            return Err(format!("Blob object {} not found", blob_hash).into());
        };
        let blob_data = match object.obj_type {
            ObjectType::Blob => object.data,
            _ => return Err(format!("Object {} is not a blob", blob_hash).into()),
        };
        let content: Vec<u8> = bincode::deserialize(&blob_data)?;
        let target = repo_path.join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(target, content)?;

        let repo_dir = repo_path.join(".astria");
        let mut index = VirtualIndex::load(&repo_dir)?;
        if let Some(entry) = index.entries.iter_mut().find(|entry| entry.path == path) {
            entry.is_virtual = false;
        }
        index.save(&repo_dir)?;
        Ok(())
    }
}

#[derive(Debug)]
struct SkipServerVerification;

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ostulated_alt_scts: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
        ]
    }
}
