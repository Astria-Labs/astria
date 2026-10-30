use crate::config;

use crate::utils::commit::CommitUtils;

pub struct MergeCommand;

impl MergeCommand {
    pub fn execute(target: String) -> std::io::Result<()> {
        let current_dir = std::env::current_dir()?;
        let cfg = config::CONFIG_GLOBAL.get().unwrap();
        let dir = current_dir.join(&cfg.config_folder);

        if !dir.exists() {
            println!("Error: Astria repository not initialized.");
            return Ok(());
        }

        // 1. Resolves destination commit (can be a branch name or a hash number)
        let heads_dir = dir.join("refs").join("heads");
        let target_branch_file = heads_dir.join(&target);

        let target_commit_hash = if target_branch_file.exists() {
            std::fs::read_to_string(&target_branch_file)?
                .trim()
                .to_string()
        } else {
            let commit_path = dir.join("objects").join(&target);
            if commit_path.exists() {
                target.clone()
            } else {
                println!("Error: Branch or commit '{}' not found.", target);
                return Ok(());
            }
        };

        let current_head_content = std::fs::read_to_string(dir.join("HEAD"))?
            .trim()
            .to_string();

        // 2. Check if we're on a branch or HEAD
        if current_head_content.starts_with("ref: refs/heads/") {
            let current_branch = current_head_content[16..].trim().to_string();
            let branch_file = heads_dir.join(&current_branch);

            // Updates current branch's pointer for destination commmit (Fast-Forward)
            std::fs::write(&branch_file, &target_commit_hash)?;
            println!(
                "Updating {}..{}",
                &CommitUtils::get_head_commit_hash(&dir)?.unwrap_or_default()[..7],
                &target_commit_hash[..7]
            );
            println!(
                "Fast-forward merge successful. Branch '{}' now points to {}",
                current_branch,
                &target_commit_hash[..7]
            );
        } else {
            // HEAD: only updates the HEAD
            CommitUtils::update_head(&dir, &target_commit_hash)?;
            println!(
                "Fast-forward merge successful. HEAD is now at {}",
                &target_commit_hash[..7]
            );
        }

        // 3. Optional. Recovers the merged files in the commit for the current workspace
        let merged_files = CommitUtils::load_commit_rust_files(&dir, &target_commit_hash)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

        for (file_path, content) in merged_files {
            let dest_path = current_dir.join(&file_path);
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&dest_path, content)?;
        }

        println!("Workspace successfully synchronized with merged commit.");

        // std::process::exit(0);

        Ok(())
    }
}
