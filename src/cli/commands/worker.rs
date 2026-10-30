use crate::config;
use crate::worker::BackgroundWorker;

pub struct WorkerCommand;

impl WorkerCommand {
    pub fn execute() -> std::io::Result<()> {
        let current_dir = std::env::current_dir()?;
        let cfg = config::CONFIG_GLOBAL.get().unwrap();
        let dir = current_dir.join(&cfg.config_folder);

        if !dir.exists() {
            println!("Error. Astria repository not initialized.");
            return Ok(());
        }

        let _ = BackgroundWorker::run(&dir);

        Ok(())
    }
}
