use std::sync::OnceLock;

pub struct Config {
    pub ignore_file: String,
    pub config_folder: String
}

pub static CONFIG_GLOBAL: OnceLock<Config> = OnceLock::new();

pub fn get() -> &'static Config {
    CONFIG_GLOBAL.get().expect("Global config not initialized.")
}