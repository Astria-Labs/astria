use crate::config;
use std::collections::HashMap;
use crate::utils::file::FileUtils;

pub struct AddCommand;

impl AddCommand {

    pub fn execute(paths: Vec<String>) -> std::io::Result<()> {

        let current_dir = std::env::current_dir()?;
        let cfg = config::CONFIG_GLOBAL.get().unwrap();
        let dir = current_dir.join(&cfg.config_folder);

        if !dir.exists() {
            println!("Error: Astria repository not initialized.");
            return Ok(());
        }

        let abs_dir = std::fs::canonicalize(&dir)?;
        let project_root = abs_dir.parent().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "Invalid repository root")
        })?;

        // 1. Carrega o índice existente
        let index_path = dir.join("index");
        let mut index_map: HashMap<String, String> = if index_path.exists() {
            let data = std::fs::read(&index_path)?;
            bincode::deserialize(&data).unwrap_or_default()
        } else {
            HashMap::new()
        };

        let mut files_to_process = Vec::new();

        for path_str in paths {
            let target_path = if path_str == "." {
                project_root.to_path_buf()
            } else {
                project_root.join(&path_str)
            };

            if !target_path.exists() {
                println!("Error: Path '{}' not found.", path_str);
                continue;
            }

            if target_path.is_dir() {
                FileUtils::collect_files_recursively(&target_path, project_root, &mut files_to_process)?;
            } else if target_path.is_file() {
                if let Ok(rel_path) = target_path.strip_prefix(project_root) {
                    files_to_process.push(rel_path.to_path_buf());
                }
            }
        }

        let staged_entries: Vec<(String, String)> = files_to_process
            .iter()
            .filter_map(|file_rel_path| {
                let file_abs_path = project_root.join(file_rel_path);
                
                if !file_abs_path.is_file() {
                    return None;
                }

                let rel_path_str = file_rel_path.to_string_lossy().replace('\\', "/");
                
                Some((rel_path_str, "staged".to_string()))
            })
            .collect();

        let mut added_count = 0;
        for (rel_path_str, status) in staged_entries {
            println!("add '{}'", rel_path_str);
            index_map.insert(rel_path_str, status);
            added_count += 1;
        }

        if added_count > 0 {
            // 4. Salva o índice atualizado de forma atômica
            let serialized_index = bincode::serialize(&index_map)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;

            std::fs::write(&index_path, serialized_index)?;
            println!("Changes successfully staged.");
        } else {
            println!("No valid files added.");
        }
    
        Ok(())
    
    }
}