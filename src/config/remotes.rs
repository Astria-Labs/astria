use std::collections::HashMap;
use std::fs;
use std::io::{Error, ErrorKind, Result};
use std::path::Path;

#[derive(Default, Debug, Clone)]
pub struct RemoteConfig {
    pub remotes: HashMap<String, RemoteSection>,
}

#[derive(Default, Debug, Clone)]
pub struct RemoteSection {
    pub url: String,
    pub repository: String,
    pub auth_token: String,
}

impl RemoteConfig {
    pub fn default_contents() -> String {
        [
            "[remote \"origin\"]",
            "    url = \"\"",
            "    repository = \"\"",
            "    auth_token = \"\"",
            "",
        ]
        .join("\n")
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
        Self { remotes }
    }

    pub fn load_from_repository(repo_path: &Path) -> Result<Self> {
        let cfg = crate::config::get();
        let remotes_path = repo_path.join(&cfg.config_folder).join("remotes");

        if !remotes_path.exists() {
            return Ok(Self::default());
        }

        let bytes = fs::read(&remotes_path)?;
        if let Ok(content) = String::from_utf8(bytes.clone()) {
            return Ok(Self::parse(&content));
        }

        let legacy: HashMap<String, String> = bincode::deserialize(&bytes)
            .map_err(|e| Error::new(ErrorKind::InvalidData, e.to_string()))?;

        let mut config = Self::default();
        for (name, url) in legacy {
            config.remotes.insert(
                name,
                RemoteSection {
                    url,
                    repository: String::new(),
                    auth_token: String::new(),
                },
            );
        }
        Ok(config)
    }

    pub fn save_to_repository(&self, repo_path: &Path) -> Result<()> {
        let cfg = crate::config::get();
        let remotes_path = repo_path.join(&cfg.config_folder).join("remotes");
        fs::write(remotes_path, self.to_config_string())
    }

    pub fn parse(content: &str) -> Self {
        let mut config = Self::default();
        let mut current_remote: Option<String> = None;

        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                current_remote = Self::parse_remote_header(&line[1..line.len() - 1]);
                if let Some(name) = &current_remote {
                    config
                        .remotes
                        .entry(name.clone())
                        .or_insert_with(RemoteSection::default);
                }
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                continue;
            };

            let Some(name) = &current_remote else {
                continue;
            };

            let remote = config
                .remotes
                .entry(name.clone())
                .or_insert_with(RemoteSection::default);
            let value = Self::unquote(value.trim());

            match key.trim() {
                "url" => remote.url = value,
                "repository" => remote.repository = value,
                "auth_token" | "token" => remote.auth_token = value,
                _ => {}
            }
        }

        config
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
        let mut out = String::new();
        let mut remote_names: Vec<_> = self.remotes.keys().cloned().collect();
        remote_names.sort();

        if remote_names.is_empty() {
            remote_names.push("origin".to_string());
        }

        for name in remote_names {
            let remote = self.remotes.get(&name).cloned().unwrap_or_default();
            out.push_str(&format!("[remote \"{}\"]\n", Self::quote(&name)));
            out.push_str(&format!("    url = \"{}\"\n", Self::quote(&remote.url)));
            out.push_str(&format!(
                "    repository = \"{}\"\n",
                Self::quote(&remote.repository)
            ));
            out.push_str(&format!(
                "    auth_token = \"{}\"\n\n",
                Self::quote(&remote.auth_token)
            ));
        }

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

    pub fn get_auth_data(&self, remote_name: &str) -> Result<(String, String)> {
        let remote = self.get_remote(remote_name)?;
        if remote.repository.trim().is_empty() {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!(
                    "Field 'repository' is missing in .astria/remotes under [remote \"{}\"]",
                    remote_name
                ),
            ));
        }
        if remote.auth_token.trim().is_empty() {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!(
                    "Field 'auth_token' is missing in .astria/remotes under [remote \"{}\"]",
                    remote_name
                ),
            ));
        }
        Ok((remote.repository.clone(), remote.auth_token.clone()))
    }
}
