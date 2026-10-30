use std::collections::HashMap;
use std::fs;
use std::io::{Error, ErrorKind, Result};
use std::path::{Path, PathBuf};

pub struct User;

#[derive(Default, Debug, Clone)]
pub struct ClientConfig {
    pub user: Option<UserSection>,
    pub remotes: HashMap<String, RemoteSection>,
    pub checkout: Option<CheckoutSection>,
}

#[derive(Default, Debug, Clone)]
pub struct UserSection {
    pub name: Option<String>,
    pub email: Option<String>,
    pub token: Option<String>,
}

#[derive(Default, Debug, Clone)]
pub struct RemoteSection {
    pub url: String,
    pub repository: String,
    pub auth_token: String,
}

#[derive(Default, Debug, Clone)]
pub struct CheckoutSection {
    pub mode: Option<String>,
}

impl User {
    pub fn set(global: bool, key_path: &str, value: &str) -> std::io::Result<()> {
        let cfg = crate::config::get();
        let target_path = if global {
            let home = Self::get_home_dir().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "Home directory not found")
            })?;
            home.join(".astriaconfig")
        } else {
            Path::new(&cfg.config_folder).join("config")
        };

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut config = ClientConfig::load_from_path_or_default(&target_path)?;
        let parts: Vec<&str> = key_path.split('.').collect();

        if parts.len() == 2 && parts[0] == "user" {
            let key = parts[1];
            let user = config.user.get_or_insert_with(UserSection::default);
            match key {
                "name" => user.name = Some(value.to_string()),
                "email" => user.email = Some(value.to_string()),
                "token" | "auth_token" => user.token = Some(value.to_string()),
                _ => {
                    return Err(Error::new(
                        ErrorKind::InvalidInput,
                        format!("Chave desconhecida: {}", key),
                    ))
                }
            }
        } else if parts.len() == 2 && parts[0] == "checkout" {
            let key = parts[1];
            let checkout = config.checkout.get_or_insert_with(CheckoutSection::default);
            match key {
                "mode" => checkout.mode = Some(value.to_string()),
                _ => {
                    return Err(Error::new(
                        ErrorKind::InvalidInput,
                        format!("Chave desconhecida: {}", key),
                    ))
                }
            }
        } else if parts.len() == 3 && parts[0] == "remote" {
            let remote_name = parts[1];
            let key = parts[2];
            let remote = config
                .remotes
                .entry(remote_name.to_string())
                .or_insert_with(RemoteSection::default);

            match key {
                "url" => remote.url = value.to_string(),
                "repository" => remote.repository = value.to_string(),
                "auth_token" | "token" => remote.auth_token = value.to_string(),
                _ => {
                    return Err(Error::new(
                        ErrorKind::InvalidInput,
                        format!("Chave desconhecida: {}", key),
                    ))
                }
            }
        } else {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("Seção/Chave desconhecida: {}", key_path),
            ));
        }

        config.save_to_path(&target_path)?;
        println!("User config saved at: {:?}", target_path);
        Ok(())
    }

    pub fn get_user(dir: &Path) -> String {
        if let (Ok(name), Ok(email)) = (
            std::env::var("ASTRIA_AUTHOR_NAME"),
            std::env::var("ASTRIA_AUTHOR_EMAIL"),
        ) {
            if !name.trim().is_empty() && !email.trim().is_empty() {
                return format!("{} <{}>", name.trim(), email.trim());
            }
        }

        if let (Ok(name), Ok(email)) = (
            std::env::var("GIT_AUTHOR_NAME"),
            std::env::var("GIT_AUTHOR_EMAIL"),
        ) {
            if !name.trim().is_empty() && !email.trim().is_empty() {
                return format!("{} <{}>", name.trim(), email.trim());
            }
        }

        let local_config = dir.join("config");
        if let Some(author) = Self::read_user_from_config(&local_config) {
            return author;
        }

        if let Some(home) = Self::get_home_dir() {
            let global_config_1 = home.join(".astriaconfig");
            if let Some(author) = Self::read_user_from_config(&global_config_1) {
                return author;
            }
            let global_config_2 = home.join(".config").join("astria").join("config");
            if let Some(author) = Self::read_user_from_config(&global_config_2) {
                return author;
            }
        }

        "Unknown <unknown@astria.org>".to_string()
    }

    fn read_user_from_config(path: &Path) -> Option<String> {
        if !path.exists() {
            return None;
        }

        let content = std::fs::read_to_string(path).ok()?;
        let config = ClientConfig::parse(&content).ok()?;

        if let Some(user) = config.user {
            let name = user.name.filter(|n| !n.trim().is_empty());
            let email = user.email.filter(|e| !e.trim().is_empty());

            match (name, email) {
                (Some(n), Some(e)) => Some(format!("{} <{}>", n, e)),
                (Some(n), None) => Some(n),
                _ => None,
            }
        } else {
            None
        }
    }

    fn get_home_dir() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
    }
}

impl ClientConfig {
    pub fn default_contents() -> String {
        ["[user]", "    name = \"\"", "    email = \"\"", ""].join("\n")
    }

    pub fn with_origin(url: String, repository: String, auth_token: String) -> Self {
        let mut remotes = HashMap::new();
        remotes.insert(
            "origin".to_string(),
            RemoteSection {
                url,
                repository,
                auth_token,
            },
        );

        Self {
            user: Some(UserSection::default()),
            remotes,
            checkout: None,
        }
    }

    pub fn load_from_repository(repo_path: &Path) -> Result<Self> {
        let cfg = crate::config::get();
        let config_path = repo_path.join(&cfg.config_folder).join("config");

        if !config_path.exists() {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!(
                    "Config file not found at {:?}. Are you inside an Astria repository?",
                    config_path
                ),
            ));
        }

        let content = fs::read_to_string(config_path)?;
        Self::parse(&content)
    }

    pub fn load_from_path_or_default(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            Self::parse(&content)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save_to_repository(&self, repo_path: &Path) -> Result<()> {
        let cfg = crate::config::get();
        self.save_to_path(&repo_path.join(&cfg.config_folder).join("config"))
    }

    pub fn save_to_path(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.to_config_string())
    }

    pub fn parse(content: &str) -> Result<Self> {
        enum Section {
            None,
            User,
            Remote(String),
            Checkout,
        }

        let mut config = Self::default();
        let mut section = Section::None;

        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                let header = &line[1..line.len() - 1];
                if header == "user" {
                    section = Section::User;
                } else if header == "checkout" {
                    section = Section::Checkout;
                } else if let Some(remote_name) = Self::parse_remote_header(header) {
                    config
                        .remotes
                        .entry(remote_name.clone())
                        .or_insert_with(RemoteSection::default);
                    section = Section::Remote(remote_name);
                } else {
                    section = Section::None;
                }
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                continue;
            };

            let key = key.trim();
            let value = Self::unquote(value.trim());

            match &section {
                Section::User => {
                    let user = config.user.get_or_insert_with(UserSection::default);
                    match key {
                        "name" => user.name = Some(value),
                        "email" => user.email = Some(value),
                        "token" | "auth_token" => user.token = Some(value),
                        _ => {}
                    }
                }
                Section::Remote(name) => {
                    let remote = config
                        .remotes
                        .entry(name.clone())
                        .or_insert_with(RemoteSection::default);
                    match key {
                        "url" => remote.url = value,
                        "repository" => remote.repository = value,
                        "auth_token" | "token" => remote.auth_token = value,
                        _ => {}
                    }
                }
                Section::Checkout => {
                    let checkout = config.checkout.get_or_insert_with(CheckoutSection::default);
                    if key == "mode" {
                        checkout.mode = Some(value);
                    }
                }
                Section::None => {}
            }
        }

        Ok(config)
    }

    fn parse_remote_header(header: &str) -> Option<String> {
        let rest = header.strip_prefix("remote ")?;
        let name = rest.trim().strip_prefix('"')?.strip_suffix('"')?;
        if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        }
    }

    fn unquote(value: &str) -> String {
        value.trim_matches('"').trim_matches('\'').to_string()
    }

    fn quote(value: &str) -> String {
        value.replace('\\', "\\\\").replace('"', "\\\"")
    }

    pub fn to_config_string(&self) -> String {
        let user = self.user.clone().unwrap_or_default();
        let mut out = String::new();

        out.push_str("[user]\n");
        out.push_str(&format!(
            "    name = \"{}\"\n",
            Self::quote(user.name.as_deref().unwrap_or(""))
        ));
        out.push_str(&format!(
            "    email = \"{}\"\n",
            Self::quote(user.email.as_deref().unwrap_or(""))
        ));
        if let Some(token) = user.token.filter(|t| !t.trim().is_empty()) {
            out.push_str(&format!("    token = \"{}\"\n", Self::quote(&token)));
        }

        if let Some(checkout) = self.checkout.clone() {
            if let Some(mode) = checkout.mode.filter(|m| !m.trim().is_empty()) {
                out.push('\n');
                out.push_str("[checkout]\n");
                out.push_str(&format!("    mode = \"{}\"\n", Self::quote(&mode)));
            }
        }

        out.push('\n');
        out
    }

    pub fn get_remote(&self, remote_name: &str) -> Result<&RemoteSection> {
        self.remotes.get(remote_name).ok_or_else(|| {
            Error::new(
                ErrorKind::NotFound,
                format!("Remote '{}' not found.", remote_name),
            )
        })
    }

    pub fn client_id(&self) -> String {
        self.user
            .as_ref()
            .and_then(|u| u.name.clone())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| "cli_user".to_string())
    }

    pub fn checkout_mode(&self) -> String {
        self.checkout
            .as_ref()
            .and_then(|checkout| checkout.mode.clone())
            .filter(|mode| !mode.trim().is_empty())
            .unwrap_or_else(|| "full".to_string())
    }

    pub fn set_checkout_mode(repo_path: &Path, mode: &str) -> Result<()> {
        let mut config = Self::load_from_repository(repo_path)?;
        let checkout = config.checkout.get_or_insert_with(CheckoutSection::default);
        checkout.mode = Some(mode.to_string());
        config.save_to_repository(repo_path)
    }

    /// Valida e extrai o repositório e o token para conexões QUIC.
    pub fn get_auth_data(&self) -> Result<(String, String)> {
        self.get_auth_data_for_remote("origin")
    }

    pub fn get_auth_data_for_remote(&self, remote_name: &str) -> Result<(String, String)> {
        let remote = self.get_remote(remote_name)?;
        let repository = remote.repository.clone();
        if repository.trim().is_empty() {
            return Err(Error::new(ErrorKind::NotFound, format!("Field 'repository' is missing or empty in .astria/config under [remote \"{}\"]", remote_name)));
        }

        let token = if !remote.auth_token.trim().is_empty() {
            remote.auth_token.clone()
        } else {
            self.user
                .as_ref()
                .and_then(|u| u.token.clone())
                .filter(|t| !t.trim().is_empty())
                .ok_or_else(|| {
                    Error::new(ErrorKind::NotFound, format!("Field 'auth_token' is missing or empty in .astria/config under [remote \"{}\"]", remote_name))
                })?
        };

        Ok((repository, token))
    }
}
