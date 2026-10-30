use crate::cli::args::{ServerSubCommand, TokenSubCommand};
use crate::network::server::AstriaServer;
use crate::server::db::ServerAuthDb;
use std::io::{Error, ErrorKind, Result};

pub struct ServerCommand;

impl ServerCommand {
    pub async fn execute(addr: String, subcommand: Option<ServerSubCommand>) -> Result<()> {
        match subcommand.unwrap_or(ServerSubCommand::Start) {
            ServerSubCommand::Start => {
                let bind_addr = addr.parse().map_err(|e| {
                    Error::new(
                        ErrorKind::InvalidInput,
                        format!("Invalid server socket address: {}", e),
                    )
                })?;
                let server = AstriaServer::new(bind_addr);
                server
                    .start()
                    .await
                    .map_err(|e| Error::new(ErrorKind::Other, e.to_string()))?;
            }
            ServerSubCommand::Token {
                command:
                    TokenSubCommand::Create {
                        user,
                        repo,
                        perms,
                        expires,
                    },
            } => {
                Self::handle_token_create(user, repo, perms, expires)?;
            }
        }
        Ok(())
    }

    fn handle_token_create(
        user: String,
        repo: String,
        perms: String,
        expires: String,
    ) -> Result<()> {
        let expires_days = Self::parse_duration(&expires)?;
        let db_path = "astria_server.db";

        let mut db = ServerAuthDb::new(db_path).map_err(|e| {
            Error::new(ErrorKind::Other, format!("Failed to open server DB: {}", e))
        })?;

        let raw_token = db
            .create_token(&user, &repo, &perms, expires_days)
            .map_err(|e| Error::new(ErrorKind::Other, format!("Failed to create token: {}", e)))?;

        println!("========================================================");
        println!("🔒 Astria Server - Scoped Access Token Generated");
        println!("========================================================");
        println!(" User       : {}", user);
        println!(" Repository : {}", repo);
        println!(" Permission : {}", perms);
        println!(" Expires    : {}", expires);
        println!("--------------------------------------------------------");
        println!("\n {}\n", raw_token);
        println!("--------------------------------------------------------");
        println!("⚠️  WARNING: Save this token now! It will not be shown again.");
        println!("========================================================");

        Ok(())
    }

    fn parse_duration(s: &str) -> Result<Option<i64>> {
        let s_lower = s.to_lowercase();
        if s_lower == "never" || s_lower == "none" {
            return Ok(None);
        }

        if let Some(num_str) = s_lower.strip_suffix('d') {
            let days: i64 = num_str.parse().map_err(|_| {
                Error::new(
                    ErrorKind::InvalidInput,
                    format!("Invalid duration number: '{}'.", num_str),
                )
            })?;
            return Ok(Some(days));
        }

        Err(Error::new(
            ErrorKind::InvalidInput,
            format!(
                "Invalid expiration format: '{}'. Use e.g. '30d', '90d', or 'never'.",
                s
            ),
        ))
    }
}
