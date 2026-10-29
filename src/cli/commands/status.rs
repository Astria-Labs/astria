use crate::config;
use std::collections::HashMap;
use crate::utils::index::IndexUtils;
use crate::utils::commit::CommitUtils;

pub struct StatusCommand;

impl StatusCommand {

    pub fn execute() -> std::io::Result<()> {

        let current_dir = std::env::current_dir()?;
        let cfg = config::CONFIG_GLOBAL.get().unwrap();
        let dir = current_dir.join(&cfg.config_folder);
        
        if !dir.exists() {
            println!("Error: Astria repository not initialized.");
            return Ok(());
        }

        let head_content = std::fs::read_to_string(dir.join("HEAD"))?.trim().to_string();
        if head_content.starts_with("ref: refs/heads/") {
            println!("On branch {}", &head_content[16..]);
        } else {
            println!("HEAD detached at {}", &head_content[7.min(head_content.len())..]);
        }

        let head_commit_opt = CommitUtils::get_head_commit_hash(&dir)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let committed_files = if let Some(ref head_commit) = head_commit_opt {
            CommitUtils::load_commit_rust_files(&dir, head_commit)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?

        } else {
            HashMap::new()
        };

        let index_map = IndexUtils::read_index(&dir)?;

        let workspace_files = CommitUtils::load_workspace_rust_files(&current_dir, &dir)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let mut staged = Vec::new();
        let mut modified = Vec::new();
        let mut untracked = Vec::new();

        for path in index_map.keys() {
            staged.push(path.clone());
        }

        for (path, content) in &workspace_files {
            if !index_map.contains_key(path) {
                if let Some(old_content) = committed_files.get(path) {
                    if old_content != content {
                        modified.push(path.clone());
                    }
                } else {
                    untracked.push(path.clone());
                }
            }
        }

        if !staged.is_empty() {
            println!("\nChanges to be committed:");
            for f in &staged {
                println!("  \x1b[32mstaged:     {}\x1b[0m", f); // Verde
            }
        }

        if !modified.is_empty() {
            println!("\nChanges not staged for commit:");
            for f in &modified {
                println!("  \x1b[31mmodified:   {}\x1b[0m", f); // Vermelho
            }
        }

        if !untracked.is_empty() {
            println!("\nUntracked files:");
            for f in &untracked {
                println!("  \x1b[31mnew file:   {}\x1b[0m", f); // Vermelho
            }
        }

        if staged.is_empty() && modified.is_empty() && untracked.is_empty() {
            println!("\nNothing to commit, working tree clean.");
        }

        Ok(())
    }
}