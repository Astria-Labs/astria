use crate::network::protocol::WireMessage;
use crate::server::db::ServerAuthDb;
use quinn::{Endpoint, ServerConfig};
use std::net::SocketAddr;

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
                    let mut missing_objects: Vec<String> = vec![];

                    let server_objects = if objects_dir.exists() {
                        std::fs::read_dir(objects_dir)?
                            .filter_map(|entry| {
                                entry.ok().and_then(|e| e.file_name().into_string().ok())
                            })
                            .collect::<Vec<String>>()
                    } else {
                        vec![]
                    };

                    for obj_hash in server_objects {
                        if !known_commits.contains(&obj_hash) {
                            missing_objects.push(obj_hash);
                        }
                    }

                    let resp = WireMessage::ServerResponse {
                        success: true,
                        message: "OK".to_string(),
                        missing_objects: missing_objects.clone(),
                    };
                    let resp_bytes = bincode::serialize(&resp)?;
                    send.write_all(&(resp_bytes.len() as u64).to_le_bytes())
                        .await?;
                    send.write_all(&resp_bytes).await?;

                    for obj_hash in missing_objects {
                        let obj_path = std::path::Path::new(".astria/objects").join(&obj_hash);
                        if obj_path.exists() {
                            let raw_data = std::fs::read(&obj_path)?;
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
