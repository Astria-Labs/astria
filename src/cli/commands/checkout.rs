use crate::config;

pub struct CheckoutCommand;

impl CheckoutCommand {

    pub fn execute(name: String) -> std::io::Result<()> {

        let current_dir = std::env::current_dir()?;
        let cfg = config::CONFIG_GLOBAL.get().unwrap();
        let dir = current_dir.join(&cfg.config_folder);
        
        if !dir.exists() {
            println!("Error: Astria repository not initialized.");
            return Ok(());
        }

        let heads_dir = dir.join("refs").join("heads");
        let target_branch_file = heads_dir.join(&name);

        if target_branch_file.is_file() {
            let head_path = dir.join("HEAD");
            std::fs::write(&head_path, format!("ref: refs/heads/{}", name))?;
            println!("Switched to branch '{}'", name);
        } else {
            let commit_path = dir.join("objects").join(&name);
            if commit_path.exists() {
                let head_path = dir.join("HEAD");
                std::fs::write(&head_path, &name)?;
                println!("Switched to detached HEAD at commit {}", &name[..7.min(name.len())]);
            } else {
                println!("Error: Branch or commit '{}' not found.", name);
            }
        }
        
        Ok(())
    }
}