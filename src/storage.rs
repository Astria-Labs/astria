use std::fs;
use std::path::Path;
use std::borrow::Cow;

use crate::objects::{
    blob::blob::Blob,
    tree::tree::Tree,
    tree::tree_entry::TreeEntry
};

fn load_ignore_patterns(base_dir: &Path) -> Vec<String> {

    let cfg = crate::config::get();
    let ignore_path = base_dir.join(&cfg.ignore_file);
    if let Ok(content) = fs::read_to_string(ignore_path) {
        content
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect()
    } else {
        vec![
            "target".to_string(), 
            Cow::Borrowed(&cfg.config_folder).to_string()
        ]
    }
}

fn is_path_ignored(rel_path: &str, patterns: &[String]) -> bool {
    let normalized_path = rel_path.replace('\\', "/");
    for pattern in patterns {
        let pat = pattern.trim_end_matches('/');
        
        // Match exato ou diretório pai
        if normalized_path == pat || normalized_path.starts_with(&format!("{}/", pat)) {
            return true;
        }
        
        // Match simples de extensão (ex: *.log)
        if pat.starts_with('*') {
            let suffix = &pat[1..];
            if normalized_path.ends_with(suffix) {
                return true;
            }
        }
    }
    false
}

pub fn init_repository(current_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let cfg = crate::config::get();
    let dir = current_dir.join(&cfg.config_folder);
    let objects_dir = dir.join("objects");
    let vectors_dir = dir.join("vectors");
    let head_path = dir.join("HEAD");

    if dir.exists() {
        println!("Repository already initialized in this directory.");
        return Ok(());
    }

    fs::create_dir_all(&objects_dir)?;
    fs::create_dir_all(&vectors_dir)?;

    let ignore_path = current_dir.join(&cfg.ignore_file);
    if !ignore_path.exists() {
        let default_ignore = "*.log\n/target/\n.astria/\n.fastembed_cache/\n.DS_Store\n";
        fs::write(current_dir.join(&cfg.ignore_file), default_ignore)?;
    }

    if !head_path.exists() {
        fs::write(&head_path, "ref: refs/heads/main")?;
    }

    println!("Initialized empty Astria repository in {}", dir.display());
    Ok(())
}

pub fn scan_workspace_and_build_tree(
    current_dir: &Path,
    main_dir: &Path,
) -> Result<(String, Vec<TreeEntry>), Box<dyn std::error::Error>> {
    let mut entries = Vec::new();
    let ignore_patterns = load_ignore_patterns(current_dir);

    let ignore_folders = ["target", "node_modules", ".git", ".astria"];

    fn visit_dirs(
        dir: &Path,
        base_dir: &Path,
        main_dir: &Path,
        ignore_patterns: &[String],
        ignore_folders: &[&str],
        entries: &mut Vec<TreeEntry>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if dir.is_dir() {

            let cfg = crate::config::get();
            
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                let file_name = entry.file_name();
                let name_str = file_name.to_string_lossy();

                if path.is_dir() && ignore_folders.contains(&name_str.as_ref()) {
                    continue;
                }

                let rel_path = path
                    .strip_prefix(base_dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");

                if (name_str.starts_with('.') && name_str != cfg.ignore_file.to_string()) 
                    || is_path_ignored(&rel_path, ignore_patterns) {
                    continue;
                }

                if path.is_dir() {
                    visit_dirs(&path, base_dir, main_dir, ignore_patterns, ignore_folders, entries)?;
                } else if path.is_file() {
                    let content = fs::read(&path)?;
                    let blob = Blob::new(content);
                    let blob_hash = blob.save(main_dir)?;

                    entries.push(TreeEntry {
                        mode: 0o100644,
                        name: rel_path,
                        object_hash: blob_hash,
                    });
                }
            }
        }
        Ok(())
    }

    visit_dirs(current_dir, current_dir, main_dir, &ignore_patterns, &ignore_folders, &mut entries)?;

    entries.sort_by(|a, b| a.name.cmp(&b.name));

    let tree = Tree { entries: entries.clone() };
    let tree_hash = tree.save(main_dir)?;

    Ok((tree_hash, entries))
}