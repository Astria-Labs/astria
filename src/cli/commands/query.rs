use crate::config;
use crate::config::remotes::RemoteConfig;
use crate::config::user::ClientConfig;
use crate::network::client::AstriaClient;
use crate::parser::SemanticParser;
use crate::utils::commit::CommitUtils;
use crate::vector;
use crate::vector::VectorStore;
use std::collections::{HashMap, HashSet};
use std::io::{Error, ErrorKind};
use std::net::SocketAddr;

pub struct QueryCommand;

impl QueryCommand {
    pub async fn execute(
        search: Option<String>,
        func: bool,
        r#struct: bool,
        interface: bool,
        pub_only: bool,
        file: Option<String>,
        commit: Option<String>,
        ai: Option<String>,
    ) -> std::io::Result<()> {
        let current_dir = std::env::current_dir()?;
        let cfg = config::CONFIG_GLOBAL.get().unwrap();
        let dir = current_dir.join(&cfg.config_folder);

        if !dir.exists() {
            println!("Error: Astria repository not initialized.");
            return Ok(());
        }

        if let Some(ref ai_query) = ai {
            let mode = ClientConfig::load_from_repository(&current_dir)
                .map(|config| config.checkout_mode())
                .unwrap_or_else(|_| "full".to_string());
            if mode == "ondemand" {
                Self::execute_ondemand_ai_query(&current_dir, &dir, ai_query).await?;
                return Ok(());
            }

            let vectors_dir = dir.join("vectors");
            let db_path_str = vectors_dir.to_str().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::Other, "Invalid path string")
            })?;

            let indexer: Box<dyn vector::SemanticIndexer> = Box::new(
                VectorStore::new(db_path_str)
                    .await
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?,
            );
            indexer
                .search(ai_query)
                .await
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

            return Ok(());
        }

        let files: HashMap<String, String> = match commit {
            Some(ref commit_hash) => {
                println!(
                    "🔎 Querying AST on commit {}...",
                    &commit_hash[..7.min(commit_hash.len())]
                );
                CommitUtils::load_commit_rust_files(&dir, commit_hash)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?
            }
            None => {
                println!("🔎 Querying AST on workspace...");
                CommitUtils::load_workspace_rust_files(&current_dir, &dir)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?
            }
        };

        let mut parser = SemanticParser::new();
        let mut matches_count = 0;
        let filter_types = func || r#struct || interface;

        for (file_path, content) in &files {
            if let Some(ref target_file) = file {
                if !file_path.contains(target_file) {
                    continue;
                }
            }

            let symbols = parser.extract_symbols(content);
            let mut file_header_printed = false;

            for (_key, sym) in symbols {
                if pub_only && !sym.is_public {
                    continue;
                }

                if filter_types {
                    let matches_category = match sym.symbol_type.as_str() {
                        "function" | "method" if func => true,
                        "struct" | "class" | "enum" if r#struct => true,
                        "interface" | "trait" if interface => true,
                        _ => false,
                    };
                    if !matches_category {
                        continue;
                    }
                }

                if let Some(ref pat) = search {
                    let pat_lower = pat.to_lowercase();
                    let name_lower = sym.name.to_lowercase();
                    if !name_lower.contains(&pat_lower) {
                        continue;
                    }
                }

                if !file_header_printed {
                    println!("\n  📄 File: {}", file_path);
                    file_header_printed = true;
                }

                let vis_label = if sym.is_public { "[PUB]" } else { "[PRIV]" };
                println!(
                    "    {} {:<10} {} (line {})",
                    vis_label, sym.symbol_type, sym.name, sym.line_number
                );

                matches_count += 1;
            }
        }

        if matches_count == 0 {
            println!("No matching symbols found.");
        } else {
            println!("\nTotal matching symbols found: {}", matches_count);
        }

        Ok(())
    }

    async fn execute_ondemand_ai_query(
        repo_path: &std::path::Path,
        repo_dir: &std::path::Path,
        ai_query: &str,
    ) -> std::io::Result<()> {
        let remote_config = RemoteConfig::load_from_repository(repo_path).map_err(|e| {
            Error::new(
                ErrorKind::NotFound,
                format!("Failed to load .astria/remotes: {}", e),
            )
        })?;
        let remote = remote_config.get_remote("origin")?;
        if remote.url.trim().is_empty() {
            return Err(Error::new(
                ErrorKind::NotFound,
                "Remote 'origin' is missing an URL in .astria/remotes.",
            ));
        }
        if !remote.url.starts_with("quic://") {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Ondemand semantic query currently requires a quic:// origin remote.",
            ));
        }

        let (repository, auth_token) = remote_config.get_auth_data("origin")?;
        let addr: SocketAddr = remote
            .url
            .trim_start_matches("quic://")
            .parse()
            .map_err(|e| {
                Error::new(ErrorKind::InvalidData, format!("Invalid remote URL: {}", e))
            })?;

        let client = AstriaClient::new(addr);
        let matches = client
            .semantic_query("cli_user", &auth_token, &repository, ai_query, 5)
            .await
            .map_err(|e| Error::new(ErrorKind::Other, format!("Semantic query failed: {}", e)))?;

        if matches.is_empty() {
            println!(
                "No semantically matching code blocks found for '{}'.",
                ai_query
            );
            return Ok(());
        }

        let mut requested_hashes = HashSet::new();
        let mut paths_to_materialize = Vec::new();

        for result in &matches {
            println!(
                "  🎯 [Score/Dist: {:.4}] File: {} | [{}] '{}' (Lines {}-{})",
                result.distance,
                result.file,
                result.kind,
                result.name,
                result.start_line,
                result.end_line
            );

            if let Some(hash) = &result.blob_hash {
                if requested_hashes.insert(hash.clone()) {
                    paths_to_materialize.push((result.file.clone(), hash.clone()));
                }
            }
        }

        println!("\nFound {} semantic match(es).", matches.len());

        if requested_hashes.is_empty() {
            println!("No blob references were returned for these matches.");
            return Ok(());
        }

        let hashes: Vec<String> = requested_hashes.into_iter().collect();
        client
            .fetch_objects(repo_path, "cli_user", &auth_token, &repository, hashes)
            .await
            .map_err(|e| Error::new(ErrorKind::Other, format!("Hydration failed: {}", e)))?;

        let mut materialized = 0;
        for (path, hash) in paths_to_materialize {
            AstriaClient::materialize_blob(repo_path, &path, &hash)
                .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;
            materialized += 1;
        }

        let _ = crate::virtual_index::VirtualIndex::load(repo_dir)?;
        println!("Materialized {} file(s) for ondemand query.", materialized);
        Ok(())
    }
}
