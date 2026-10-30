use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{DateTime, Duration, Utc};
use rand::Rng;
use rusqlite::{params, Connection, Result as SqlResult};

pub struct ServerAuthDb {
    conn: Connection,
}

impl ServerAuthDb {
    pub fn new(db_path: &str) -> SqlResult<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS access_tokens (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL,
                repository TEXT NOT NULL,
                permission TEXT NOT NULL,
                token_hash TEXT NOT NULL,
                created_at TEXT NOT NULL,
                expires_at TEXT
            )",
            [],
        )?;
        Ok(Self { conn })
    }

    /// Gera um token restrito a um usuário, repositório e permissão
    pub fn create_token(
        &mut self,
        username: &str,
        repository: &str,
        permission: &str,
        expires_in_days: Option<i64>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let random_bytes: [u8; 32] = OsRng.gen();
        let raw_token = format!("ast_pat_{}", hex::encode(random_bytes));

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(raw_token.as_bytes(), &salt)
            .map_err(|e| {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                )) as Box<dyn std::error::Error>
            })?
            .to_string();

        let created_at = Utc::now().to_rfc3339();
        let expires_at =
            expires_in_days.map(|days| (Utc::now() + Duration::days(days)).to_rfc3339());

        self.conn.execute(
            "INSERT INTO access_tokens (username, repository, permission, token_hash, created_at, expires_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![username, repository, permission, password_hash, created_at, expires_at],
        )?;

        Ok(raw_token)
    }

    /// Valida se o token é legítimo E se possui permissão para o repositório alvo e ação requerida
    pub fn validate_access(
        &self,
        raw_token: &str,
        target_repo: &str,
        required_perm: &str,
    ) -> SqlResult<bool> {
        let mut stmt = self
            .conn
            .prepare("SELECT repository, permission, token_hash, expires_at FROM access_tokens")?;

        let token_iter = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;

        let argon2 = Argon2::default();

        for row_result in token_iter {
            let (repo, perm, stored_hash, expires_at) = row_result?;

            // 1. Verifica expiração
            if let Some(exp) = expires_at {
                if let Ok(exp_date) = DateTime::parse_from_rfc3339(&exp) {
                    if Utc::now() > exp_date {
                        continue;
                    }
                }
            }

            // 2. Valida o hash do Argon2
            if let Ok(parsed_hash) = PasswordHash::new(&stored_hash) {
                if argon2
                    .verify_password(raw_token.as_bytes(), &parsed_hash)
                    .is_ok()
                {
                    // 3. Valida se o token pertence ao repositório correto
                    if repo == target_repo || repo == "*" {
                        // "*" pode ser usado para tokens globais de admin se necessário
                        // 4. Valida a hierarquia de permissões (ex: 'admin' faz tudo, 'write' faz push/pull, 'read' só pull)
                        if Self::check_permission_level(&perm, required_perm) {
                            return Ok(true);
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    fn check_permission_level(granted: &str, required: &str) -> bool {
        match granted {
            "admin" => true,
            "write" => required == "write" || required == "read",
            "read" => required == "read",
            _ => false,
        }
    }
}
