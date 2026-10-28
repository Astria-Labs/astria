use crate::config;
use std::collections::BTreeSet;
use crate::parser::SemanticParser;
use crate::utils::commit::CommitUtils;

pub struct DiffCommand;

impl DiffCommand {

    pub fn execute(targets: Vec<String>) -> std::io::Result<()> {

        let current_dir = std::env::current_dir()?;
        let cfg = config::CONFIG_GLOBAL.get().unwrap();
        let dir = current_dir.join(&cfg.config_folder);

        if !dir.exists() {
            println!("Error: Astria repository not initialized.");
            return Ok(());
        }

        let (files_old, files_new, label_old, label_new, file_filter) = match targets.len() {
            0 => {
                let head_commit = match CommitUtils::get_head_commit_hash(&dir) {
                    Ok(Some(commit)) => commit,
                    Ok(None) => {
                        println!("No HEAD commits found yet. Run 'ast commit' to create your first commit.");
                        std::process::exit(0);
                    },
                    Err(e) => return Err(e.into()),
                };
                let f_head = CommitUtils::load_commit_rust_files(&dir, &head_commit)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

                let f_ws = CommitUtils::load_workspace_rust_files(&current_dir, &dir)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

                (f_head, f_ws, format!("HEAD ({})", &head_commit[..7]), "workspace".to_string(), None)
            }
            1 => {
                let target = &targets[0];
                let target_path = current_dir.join(target);
                
                if target_path.exists() && target_path.is_file() {
                    // Ex: "ast diff src/main.rs" -> Compara o arquivo atual com o HEAD
                    let head_commit = match CommitUtils::get_head_commit_hash(&dir) {
                        Ok(Some(commit)) => commit,
                        Ok(None) => {
                            println!("No HEAD commits found yet. Run 'ast commit' to create your first commit.");
                            std::process::exit(0);
                        },
                        Err(e) => return Err(e.into()),
                    };
                    let f_head = CommitUtils::load_commit_rust_files(&dir, &head_commit)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
                    
                    let f_ws = CommitUtils::load_workspace_rust_files(&current_dir, &dir)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

                    (f_head, f_ws, format!("HEAD ({})", &head_commit[..7]), "workspace".to_string(), Some(target.clone()))
                } else {
                    // Ex: "ast diff <COMMIT_HASH>" -> Compara o commit informado com o Workspace atual
                    let f_a = CommitUtils::load_commit_rust_files(&dir, target)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

                    let f_ws = CommitUtils::load_workspace_rust_files(&current_dir, &dir)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

                    (f_a, f_ws, format!("commit {}", &target[..7.min(target.len())]), "workspace".to_string(), None)
                }
            }
            2 => {
                let a = &targets[0];
                let b = &targets[1];
                let f_a = CommitUtils::load_commit_rust_files(&dir, a)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

                let f_b = CommitUtils::load_commit_rust_files(&dir, b)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

                (f_a, f_b, format!("commit {}", &a[..7.min(a.len())]), format!("commit {}", &b[..7.min(b.len())]), None)
            }
            _ => return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Too many arguments for diff."
            )),
        };

        println!("🔍 AST Semantic Diff: {} ➔ {}\n", label_old, label_new);

        let mut parser = SemanticParser::new();
        let mut all_files: BTreeSet<_> = files_old.keys().cloned().collect();
        all_files.extend(files_new.keys().cloned());

        let mut total_changes = 0;

        for file in all_files {
            if let Some(ref target) = file_filter {
                if !file.contains(target) {
                    continue;
                }
            }

            let empty = String::new();
            let code_old = files_old.get(&file).unwrap_or(&empty);
            let code_new = files_new.get(&file).unwrap_or(&empty);

            let symbols_old = parser.extract_symbols(code_old);
            let symbols_new = parser.extract_symbols(code_new);

            let mut file_printed = false;
            let mut matched_old = BTreeSet::new();
            let mut matched_new = BTreeSet::new();

            for (key, sym_new) in &symbols_new {
                if let Some(sym_old) = symbols_old.get(key) {
                    matched_new.insert(key.clone());
                    matched_old.insert(key.clone());
                    if sym_old.content_hash != sym_new.content_hash {
                        if !file_printed {
                            println!("  File: {}", file);
                            file_printed = true;
                        }
                        println!("    [MODIFIED] {} '{}'", sym_new.symbol_type, sym_new.name);
                        total_changes += 1;
                    }
                }
            }

            let unmatched_new: Vec<_> = symbols_new.iter().filter(|(k, _)| !matched_new.contains(*k)).collect();
            let mut consumed_old = BTreeSet::new();

            for (key_new, sym_new) in unmatched_new {
                let found_old = symbols_old.iter().find(|(k_old, sym_old)| {
                    !matched_old.contains(*k_old) 
                    && !consumed_old.contains(*k_old)
                    && sym_old.symbol_type == sym_new.symbol_type
                    && sym_old.content_hash == sym_new.content_hash
                });

                if let Some((key_old, sym_old)) = found_old {
                    consumed_old.insert(key_old.clone());
                    matched_new.insert(key_new.clone());
                    matched_old.insert(key_old.clone());

                    if !file_printed {
                        println!("  File: {}", file);
                        file_printed = true;
                    }
                    println!("    [RENAMED]  {} '{}' ➔ '{}'", sym_new.symbol_type, sym_old.name, sym_new.name);
                    total_changes += 1;
                }
            }

            for (key, sym_new) in &symbols_new {
                if !matched_new.contains(key) {
                    if !file_printed {
                        println!("  File: {}", file);
                        file_printed = true;
                    }
                    println!("    [ADDED]    {} '{}'", sym_new.symbol_type, sym_new.name);
                    total_changes += 1;
                }
            }

            for (key, sym_old) in &symbols_old {
                if !matched_old.contains(key) {
                    if !file_printed {
                        println!("  File: {}", file);
                        file_printed = true;
                    }
                    println!("    [REMOVED]  {} '{}'", sym_old.symbol_type, sym_old.name);
                    total_changes += 1;
                }
            }
        }

        if total_changes == 0 {
            println!("No AST structural changes detected.");
        } else {
            println!("\nTotal structural changes: {}", total_changes);
        }

        Ok(())
    }
}
