use crate::config;
use crate::objects::{
    astria::astria::Astria, commit::commit::Commit, object_type::ObjectType, tree::tree::Tree,
};

pub struct ShowCommand;

impl ShowCommand {
    pub fn execute(hash: String) -> std::io::Result<()> {
        let current_dir = std::env::current_dir()?;
        let cfg = config::CONFIG_GLOBAL.get().unwrap();
        let dir = current_dir.join(&cfg.config_folder);

        if !dir.exists() {
            println!("Error: Astria repository not initialized.");
            return Ok(());
        }

        let objects_dir = dir.join("objects");
        let mut target_path = objects_dir.join(&hash);
        if !target_path.exists() {
            if let Ok(entries) = std::fs::read_dir(&objects_dir) {
                let matching: Vec<_> = entries
                    .filter_map(Result::ok)
                    .filter(|e| e.file_name().to_string_lossy().starts_with(&hash))
                    .collect();
                if matching.len() == 1 {
                    target_path = matching[0].path();
                } else if matching.len() > 1 {
                    println!(
                        "Ambiguous hash prefix '{}'. Matches multiple objects.",
                        hash
                    );
                    return Ok(());
                } else {
                    println!("Object {} not found.", hash);
                    return Ok(());
                }
            } else {
                println!("Object {} not found.", hash);
                return Ok(());
            }
        }

        let compressed_data = std::fs::read(&target_path)?;
        let serialized_data = zstd::decode_all(&compressed_data[..])?;
        let main_object: Astria = bincode::deserialize(&serialized_data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        match main_object.obj_type {
            ObjectType::Commit => {
                let commit_obj: Commit = bincode::deserialize(&main_object.data)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

                println!("=== COMMIT OBJECT ===");
                println!("Tree Hash:   {}", commit_obj.tree_hash);

                if let Some(parent) = &commit_obj.parents.first() {
                    println!("Parent Hash: {}", parent);
                } else {
                    println!("Parent Hash: None");
                }

                println!("Author:      {}", commit_obj.author);
                println!("Timestamp:   {}", commit_obj.timestamp);
                println!("Message:     \n    {}", commit_obj.message);
            }
            ObjectType::Tree => {
                let tree_obj: Tree = bincode::deserialize(&main_object.data)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

                println!("=== TREE OBJECT ===");
                println!("Entries ({} files):", tree_obj.entries.len());
                for entry in tree_obj.entries {
                    println!(
                        "  mode: {:o} | hash: {} | path: {}",
                        entry.mode, entry.object_hash, entry.name
                    );
                }
            }
            ObjectType::Blob => {
                let raw_content: Vec<u8> = bincode::deserialize(&main_object.data)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

                println!("=== BLOB OBJECT ===");
                if let Ok(text) = String::from_utf8(raw_content.clone()) {
                    print!("{}", text);
                } else {
                    println!("[Binary data: {} bytes]", raw_content.len());
                }
            }
        }

        Ok(())
    }
}
