use crate::config;
use crate::utils::commit::CommitUtils;

pub struct BranchCommand;

impl BranchCommand {

    pub fn execute(name: Option<String>) -> std::io::Result<()> {

        let current_dir = std::env::current_dir()?;
        let cfg = config::CONFIG_GLOBAL.get().unwrap();
        let dir = current_dir.join(&cfg.config_folder);
            
        if !dir.exists() {
            println!("Error: Astria repository not initialized.");
            return Ok(());
        }
        let heads_dir = dir.join("refs").join("heads");
        
        match name {
            Some(branch_name) => {
                let head_commit = CommitUtils::get_head_commit_hash(&dir)?.unwrap_or_default();
                let branch_path = heads_dir.join(&branch_name);
                if let Some(parent) = branch_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                std::fs::write(&branch_path, &head_commit)?;
                if head_commit.is_empty() {
                    println!("Branch '{}' created (empty, no commits yet).", branch_name);
                } else {        
                    println!("Branch '{}' created at commit {}", branch_name, &head_commit[..7.min(head_commit.len())]);
                }
            }
            None => {
                let current_head_content = std::fs::read_to_string(dir.join("HEAD"))?.trim().to_string();
                let current_branch = if current_head_content.starts_with("ref: refs/heads/") {
                    current_head_content[16..].trim().to_string()
                } else {
                    String::new()
                };

                if heads_dir.exists() {
                    println!("Branches:");
                    let mut branches = Vec::new();
                    CommitUtils::collect_branch_names(&heads_dir, &heads_dir, &mut branches)?;

                    branches.sort();

                    for branch_name in branches {
                        if branch_name == current_branch {
                            println!("* {}", branch_name);
                        } else {
                            println!("  {}", branch_name);
                        }
                    }
                } else {
                    println!("No branches found.");
                }
            }
        }

        Ok(())
    }
}