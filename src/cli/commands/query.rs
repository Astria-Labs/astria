use crate::config;
use crate::parser::SemanticParser;
use crate::utils::commit::CommitUtils;
use crate::vector;
use crate::vector::VectorStore;
use std::collections::HashMap;

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
            let db_path_str = dir.to_str().ok_or_else(|| {
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
}
