pub mod remotes;
pub mod user;

pub use remotes::{RemoteConfig, RemoteSection};
use std::sync::OnceLock;
pub use user::{ClientConfig, User, UserSection};

pub struct Config {
    pub ignore_file: String,
    pub config_folder: String,
}

pub static CONFIG_GLOBAL: OnceLock<Config> = OnceLock::new();

pub fn get() -> &'static Config {
    CONFIG_GLOBAL.get().expect("Global config not initialized.")
}
