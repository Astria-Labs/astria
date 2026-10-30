use crate::config::remotes::{RemoteConfig, RemoteSection};
use crate::config::user::User;
use std::io::{Error, ErrorKind};

pub struct ConfigCommand;

impl ConfigCommand {
    pub fn execute(global: bool, args: Vec<String>) -> std::io::Result<()> {
        if args.len() >= 2 {
            let key = &args[0];
            let raw = args[1..].join(" ");
            let value = raw.trim_matches('"').trim_matches('\'');

            let result = if key.starts_with("remote.") {
                Self::set_remote(key, value)
            } else {
                User::set(global, key, value)
            };

            match result {
                Ok(_) if global => println!("Your global configurations have been set."),
                Ok(_) => println!("Your repository configurations have been set."),
                Err(e) => eprintln!("Failed to set: {}", e),
            }
        } else {
            println!("Usage: ast config [--global] user.name \"Your name\"");
            println!("       ast config [--global] user.email \"your@email.com\"");
            println!("       ast config remote.origin.url \"quic://127.0.0.1:4433\"");
            println!("       ast config remote.origin.repository \"meu-projeto\"");
            println!("       ast config remote.origin.auth_token \"astria_token_secret_xyz123\"");
        }

        Ok(())
    }

    fn set_remote(key_path: &str, value: &str) -> std::io::Result<()> {
        let parts: Vec<&str> = key_path.split('.').collect();
        if parts.len() != 3 || parts[0] != "remote" {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "Invalid remote format. Use remote.<name>.url, remote.<name>.repository or remote.<name>.auth_token",
            ));
        }

        let current_dir = std::env::current_dir()?;
        let mut remotes = RemoteConfig::load_from_repository(&current_dir)?;
        let remote = remotes
            .remotes
            .entry(parts[1].to_string())
            .or_insert_with(RemoteSection::default);

        match parts[2] {
            "url" => remote.url = value.to_string(),
            "repository" => remote.repository = value.to_string(),
            "auth_token" | "token" => remote.auth_token = value.to_string(),
            other => {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!("Unknown remote key: {}", other),
                ))
            }
        }

        remotes.save_to_repository(&current_dir)
    }
}
