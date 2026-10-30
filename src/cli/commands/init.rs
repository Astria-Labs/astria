use crate::storage;

pub struct InitCommand;

impl InitCommand {
    pub fn execute() -> std::io::Result<()> {
        let current_dir = std::env::current_dir()?;
        let _ = storage::init_repository(&current_dir);

        Ok(())
    }
}
