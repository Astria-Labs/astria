use crate::config;
use crate::config::user::User;
use crate::queue::TaskQueue;
use crate::utils::commit::CommitUtils;
use crate::utils::index::IndexUtils;
use crate::worker::BackgroundWorker;
use std::collections::HashMap;

use crate::objects::{
    blob::blob::Blob, commit::commit::Commit, tree::tree::Tree, tree::tree_entry::TreeEntry,
};

pub struct CommitCommand;

impl CommitCommand {
    pub fn execute(message: String) -> std::io::Result<()> {
        let current_dir = std::env::current_dir()?;
        let cfg = config::CONFIG_GLOBAL.get().unwrap();
        let dir = current_dir.join(&cfg.config_folder);

        if !dir.exists() {
            println!("Error: Astria repository not initialized. Run 'cargo run -- init' first.");
            return Ok(());
        }

        let abs_dir = std::fs::canonicalize(&dir)?;
        let project_root = abs_dir.parent().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "Invalid repository root")
        })?;

        let index_path = dir.join("index");
        if !index_path.exists() {
            println!("No changes added to commit (use 'Astria add <file>').");
            return Ok(());
        }

        let index_data = std::fs::read(&index_path)?;
        let index_map: HashMap<String, String> = bincode::deserialize(&index_data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

        if index_map.is_empty() {
            println!("Nothing to commit, index is empty.");
            return Ok(());
        }

        let parent_hash_opt = CommitUtils::get_head_commit_hash(&dir)?;
        let mut parents = Vec::new();

        if let Some(parent_hash) = parent_hash_opt {
            CommitUtils::validate_object_exists(&dir, &parent_hash)?;
            parents.push(parent_hash);
        }

        let mut tree_entries = Vec::new();

        for (path, status) in &index_map {
            if status != "staged" {
                continue;
            }

            let file_abs_path = project_root.join(&path);
            if !file_abs_path.is_file() {
                continue;
            }

            let content = match std::fs::read(&file_abs_path) {
                Ok(c) => c,
                Err(e) => {
                    println!("Error reading file from disk: {:?}", e);
                    continue;
                }
            };

            let blob = Blob::new(content);
            let blob_hash = match blob.save(&dir) {
                Ok(h) => h,
                Err(e) => {
                    println!("Erro to save blob: {:?}", e);
                    continue;
                }
            };

            println!("Blob created! {}", &blob_hash[..7]);

            tree_entries.push(TreeEntry {
                mode: 0o100644,
                name: path.to_string(),
                object_hash: blob_hash,
            });
        }

        if tree_entries.is_empty() {
            println!("Nothing to commit.");
            return Ok(());
        }

        // Garantee deterministic ordering
        tree_entries.sort_by(|a, b| a.name.cmp(&b.name));

        let tree = Tree {
            entries: tree_entries.clone(),
        };
        let tree_hash = tree
            .save(&dir)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        CommitUtils::validate_object_exists(&dir, &tree_hash)?;

        let author = User::get_user(&dir);
        let commit = Commit::new(tree_hash.clone(), parents, message, author);
        let commit_main_object = commit.to_astria_object();
        let commit_hash = commit_main_object
            .save(&dir)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        CommitUtils::update_head(&dir, &commit_hash)?;

        if let Err(_e) = TaskQueue::push(&dir, &commit_hash, &tree_hash) {
            eprintln!("⚠️ Warning: Commit has been created, but failed to be queued.")
        } else {
            BackgroundWorker::spawn(&dir);
        }

        IndexUtils::remove_index(&dir)?;

        println!("\n⚡️ Commit created successfully: {}", commit_hash);
        Ok(())
    }
}
