use std::fs;
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    name: String,
    email: String
}

impl User {
    pub fn set(global: bool, key_path: &str, value: &str) -> std::io::Result<()> {
        let target_path = if global {
            let home = Self::get_home_dir().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "Home directory not found")
            })?;
            home.join(".astriaconfig")
        } else {
            // Assume que estamos na raiz do repo local
            Path::new(".astria").join("config")
        };

        // Garante que o diretório pai existe (especialmente pro global)
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Lê o conteúdo atual ou começa vazio
        let content = if target_path.exists() {
            fs::read_to_string(&target_path)?
        } else {
            String::new()
        };

        let mut lines: Vec<String> = content.lines().map(String::from).collect();
        let mut updated = false;
        let mut in_target_section = false;

        // Ex: key_path = "user.name" -> section = "user", key = "name"
        let parts: Vec<&str> = key_path.split('.').collect();
        if parts.len() != 2 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Formato inválido. Use user.name ou user.email",
            ));
        }
        let section = parts[0].to_lowercase();
        let key = parts[1].to_lowercase();

        for line in &mut lines {
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                in_target_section = trimmed.to_lowercase().contains(&section);
                continue;
            }

            if in_target_section && trimmed.contains('=') {
                if let Some((k, _)) = trimmed.split_once('=') {
                    if k.trim().to_lowercase() == key {
                        *line = format!("\t{} = {}", key, value);
                        updated = true;
                    }
                }
            }
        }

        // Se a seção ou chave não existia, adiciona ao final
        if !updated {
            if !lines.iter().any(|l| l.to_lowercase().contains(&format!("[{}]", section))) {
                lines.push(format!("[{}]", section));
            }
            lines.push(format!("\t{} = {}", key, value));
        }

        // Grava de volta no arquivo
        let new_content = lines.join("\n") + "\n";
        fs::write(&target_path, new_content)?;

        println!("Configuração salva com sucesso em: {:?}", target_path);
        Ok(())
    }

    pub fn get_user(dir: &Path) -> String {
    
        if let (Ok(name), Ok(email)) = (std::env::var("ASTRIA_AUTHOR_NAME"), std::env::var("ASTRIA_AUTHOR_EMAIL")) {
            if !name.trim().is_empty() && !email.trim().is_empty() {
                return format!("{} <{}>", name.trim(), email.trim());
            }
        }
    
        if let (Ok(name), Ok(email)) = (std::env::var("GIT_AUTHOR_NAME"), std::env::var("GIT_AUTHOR_EMAIL")) {
            if !name.trim().is_empty() && !email.trim().is_empty() {
                return format!("{} <{}>", name.trim(), email.trim());
            }
        }
    
        let local_config = dir.join("config");
        if let Some(author) = Self::read_user_from_ini(&local_config) {
            return author;
        }
    
        if let Some(home) = Self::get_home_dir() {
            let global_config_1 = home.join(".astriaconfig");
            if let Some(author) = Self::read_user_from_ini(&global_config_1) {
                return author;
            }
            let global_config_2 = home.join(".config").join("astria").join("config");
            if let Some(author) = Self::read_user_from_ini(&global_config_2) {
                return author;
            }
        }
    
        "Unknown <unknown@astria.org>".to_string()
    }
    
    fn read_user_from_ini(path: &Path) -> Option<String> {
        if !path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(path).ok()?;
        
        let mut name: Option<String> = None;
        let mut email: Option<String> = None;
        let mut in_user_section = false;
    
        for line in content.lines() {
            let line = line.trim();

            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                let section = line[1..line.len() - 1].trim().to_lowercase();
                in_user_section = section == "user";
                continue;
            }

            if in_user_section {
                if let Some((k, v)) = line.split_once('=') {
                    let k = k.trim().to_lowercase();
                    let v = v.trim().trim_matches('"').trim_matches('\'').to_string();
                    if k == "name" && !v.is_empty() {
                        name = Some(v);
                    } else if k == "email" && !v.is_empty() {
                        email = Some(v);
                    }
                }
            }
        }
    
        match (name, email) {
            (Some(n), Some(e)) => Some(format!("{} <{}>", n, e)),
            (Some(n), None) => Some(n),
            _ => None,
        }
    }
    
    fn get_home_dir() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
    }
}
