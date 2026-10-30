use crate::config;
use crate::objects::{astria::astria::Astria, commit::commit::Commit};
use crate::utils::commit::CommitUtils;

pub struct LogCommand;

impl LogCommand {
    pub fn execute() -> std::io::Result<()> {
        let current_dir = std::env::current_dir()?;
        let cfg = config::CONFIG_GLOBAL.get().unwrap();
        let dir = current_dir.join(&cfg.config_folder);

        if !dir.exists() {
            println!("Error: Astria repository not initialized.");
            return Ok(());
        }

        let current_commit_opt = CommitUtils::get_head_commit_hash(&dir)?;
        let mut current_commit_hash = match current_commit_opt {
            Some(h) => h,
            None => {
                println!("No commits found in this repository yet.");
                return Ok(());
            }
        };

        println!("--------------------------------------------------");
        let mut commit_count = 0;

        while !current_commit_hash.is_empty() {
            commit_count += 1;

            let obj_path = dir.join("objects").join(&current_commit_hash);
            if !obj_path.exists() {
                println!("Error: Commit object {} not found.", current_commit_hash);
                break;
            }

            let compressed_data = std::fs::read(&obj_path)?;
            let serialized_data = zstd::decode_all(&compressed_data[..])?;
            let main_object: Astria = bincode::deserialize(&serialized_data)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

            let commit_obj: Commit = bincode::deserialize(&main_object.data)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

            let datetime = chrono::DateTime::from_timestamp(commit_obj.timestamp as i64, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "Unknown time".to_string());

            println!("commit {}", current_commit_hash);
            println!("Tree:   {}", commit_obj.tree_hash);
            println!("Author: {}", commit_obj.author);
            println!("Date:   {}", datetime);
            println!("\n    {}\n", commit_obj.message);

            if let Some(parent) = commit_obj.parents.first() {
                current_commit_hash = parent.to_string();
                println!("--------------------------------------------------");
            } else {
                break;
            }
        }

        if commit_count == 0 {
            println!("No commit history available.");
        }

        Ok(())
    }
}
