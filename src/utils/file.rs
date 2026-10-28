use std::path::{Path, PathBuf};

pub struct FileUtils;

impl FileUtils {

    pub fn collect_files_recursively(dir: &Path, project_root: &Path, collected: &mut Vec<PathBuf>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_name = entry.file_name();
            let name_lossy = file_name.to_string_lossy();

            if name_lossy.starts_with('.') {
                continue;
            }

            // 2. Ignora a pasta de build do Rust e outras lixeiras comuns
            if name_lossy == "target" || name_lossy == "node_modules" {
                continue;
            }

            if path.is_dir() {
                Self::collect_files_recursively(&path, project_root, collected)?;
            } else if path.is_file() {
                if let Ok(rel_path) = path.strip_prefix(project_root) {
                    collected.push(rel_path.to_path_buf());
                }
            }
        }
        Ok(())
    }
}