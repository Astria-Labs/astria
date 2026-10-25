use std::path::Path;
use std::collections::HashMap;
use crate::storage::scan_workspace_and_build_tree;

use crate::objects::{
    commit::commit::Commit, 
    astria::astria::Astria,
    tree::tree::Tree
};

/// Helper to resolve HEAD (supports direct commit hashes or symbolic refs like 'ref: refs/heads/main')
pub fn get_head_commit_hash(dir: &Path) -> std::io::Result<Option<String>> {
    let head_path = dir.join("HEAD");
    if !head_path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&head_path)?.trim().to_string();
    if content.is_empty() {
        return Ok(None);
    }

    if content.starts_with("ref: ") {
        let ref_path_str = &content[5..]; // strip "ref: "
        let ref_file = dir.join(ref_path_str);
        if ref_file.exists() {
            let hash = std::fs::read_to_string(&ref_file)?.trim().to_string();
            if hash.is_empty() { Ok(None) } else { Ok(Some(hash)) }
        } else {
            Ok(None)
        }
    } else {
        Ok(Some(content))
    }
}

/// Helper to update HEAD with a new commit hash
pub fn update_head(dir: &Path, commit_hash: &str) -> std::io::Result<()> {
    let head_path = dir.join("HEAD");
    if head_path.exists() {
        let content = std::fs::read_to_string(&head_path)?.trim().to_string();
        if content.starts_with("ref: ") {
            let ref_path_str = &content[5..];
            let ref_file = dir.join(ref_path_str);
            if let Some(parent) = ref_file.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&ref_file, commit_hash)?;
            return Ok(());
        }
    }
    // Fallback: write directly to HEAD
    std::fs::write(&head_path, commit_hash)?;
    Ok(())
}

/// Helper to load all the commits
pub fn load_commit_rust_files(
    dir: &std::path::Path,
    commit_hash: &str,
) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let mut files = HashMap::new();

    let commit_path = dir.join("objects").join(commit_hash);
    if !commit_path.exists() {
        return Ok(files);
    }

    let compressed = std::fs::read(&commit_path)?;
    let serialized = zstd::decode_all(&compressed[..])?;
    let main_object: Astria = bincode::deserialize(&serialized)?;
    let commit_obj: Commit = bincode::deserialize(&main_object.data)?;

    let tree_path = dir.join("objects").join(&commit_obj.tree_hash);
    if !tree_path.exists() {
        return Ok(files); // Retorna vazio se a árvore física não estiver no CAS
    }

    let tree_compressed = std::fs::read(&tree_path)?;
    let tree_serialized = match zstd::decode_all(&tree_compressed[..]) {
        Ok(s) => s,
        Err(_) => return Ok(files),
    };
    
    let tree_main_object: Astria = match bincode::deserialize(&tree_serialized) {
        Ok(v) => v,
        Err(_) => return Ok(files),
    };

    let tree_obj: Tree = match bincode::deserialize(&tree_main_object.data) {
        Ok(t) => t,
        Err(_) => return Ok(files),
    };

    for entry in tree_obj.entries {
        if entry.name.ends_with(".rs") {
            let blob_path = dir.join("objects").join(&entry.object_hash);
            if blob_path.exists() {
                if let Ok(blob_compressed) = std::fs::read(&blob_path) {
                    if let Ok(blob_serialized) = zstd::decode_all(&blob_compressed[..]) {
                        if let Ok(blob_main_object) = bincode::deserialize::<Astria>(&blob_serialized) {
                            if let Ok(raw_bytes) = bincode::deserialize::<Vec<u8>>(&blob_main_object.data) {
                                if let Ok(text) = String::from_utf8(raw_bytes) {
                                    files.insert(entry.name, text);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(files)
}

/// Helper to load current workspace .rs files
pub fn load_workspace_rust_files(
    current_dir: &std::path::Path,
    dir: &std::path::Path,
) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let mut files = HashMap::new();
    let (_, scanned_entries) = scan_workspace_and_build_tree(current_dir, dir)?;

    for entry in scanned_entries {
        if entry.name.ends_with(".rs") {
            let file_path = current_dir.join(&entry.name);
            if let Ok(content) = std::fs::read_to_string(file_path) {
                files.insert(entry.name, content);
            }
        }
    }

    Ok(files)
}

/// Helper to collect branches name recursively
pub fn collect_branch_names(base_dir: &std::path::Path, current_dir: &std::path::Path, list: &mut Vec<String>) -> std::io::Result<()> {
    if current_dir.is_dir() {
        for entry in std::fs::read_dir(current_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                collect_branch_names(base_dir, &path, list)?;
            } else if path.is_file() {
                if let Ok(rel) = path.strip_prefix(base_dir) {
                    if let Some(s) = rel.to_str() {
                        list.push(s.replace('\\', "/"));
                    }
                }
            }
        }
    }
    Ok(())
}