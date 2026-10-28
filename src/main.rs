mod parser;
mod storage;
mod vector;
mod config;
mod objects;
mod versioning;
mod worker;
mod queue;

use crate::objects::{
    astria::astria::Astria,
    blob::blob::Blob,
    commit::commit::Commit,
    tree::tree::Tree,
    tree::tree_entry::TreeEntry,
    object_type::ObjectType
};

use vector::VectorStore;
use crate::config::{Config, user::User};
use parser::SemanticParser;
use crate::versioning::utils;
use storage::init_repository;
use clap::{Parser, Subcommand};
use queue::TaskQueue;
use worker::BackgroundWorker;
use std::collections::{HashMap, BTreeSet};

#[derive(Parser)]
#[command(name = "ast")]
#[command(about = "A local-first, semantic-aware version control system in Rust", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new Astria repository
    Init,
    /// Record changes to the repository with AST semantic indexing
    #[command(visible_alias = "ci")]
    Commit {
        #[arg(short, long, default_value = "Auto commit from workspace scan")]
        message: String,
    },
    /// Show commit history logs
    Log,
    /// Show content of a Astria object by its hash
    Show {
        #[arg(value_name = "HASH")]
        hash: String,
    },
    /// Perform semantic AST diff between commits, workspace, or files
    Diff {
        /// Targets: can be 0, 1, or 2 arguments (commits, workspace, or file paths)
        #[arg(num_args(0..=2), value_name = "TARGET")]
        targets: Vec<String>,
    },
    /// Performs AI Semantic/Symbol search
    Query {
        /// Default search term (eg: symbol name)
        search: Option<String>,

        /// Filtrar apenas funções / métodos / subroutines
        #[arg(long, visible_alias = "fn")]
        func: bool,

        /// Filter only structs, classes or data types
        #[arg(long)]
        r#struct: bool, // 'r#struct' para escapar palavra reservada do Rust

        /// Filtrar apenas interfaces, traits ou contratos
        #[arg(long)]
        interface: bool,

        /// Filtrar apenas elementos públicos / exportados (--pub ou -p)
        #[arg(long, short = 'p')]
        pub_only: bool,

        /// Restrict the search to a specific file (e.g., src/main.rs)
        #[arg(long, short = 'f')]
        file: Option<String>,

        /// Search a specific commit instead of the current workspace (HEAD)
        #[arg(long, visible_alias = "ci")]
        commit: Option<String>,

        /// [AI Boost] Semantic search in natural language
        #[arg(long)]
        ai: Option<String>,
    },
    /// List or create branches
    #[command(visible_alias = "br")]
    Branch {
        #[arg(value_name = "BRANCH_NAME")]
        name: Option<String>,
    },
    /// Switch branch or restore context
    #[command(visible_alias = "co")]
    Checkout {
        #[arg(value_name = "BRANCH_OR_COMMIT")]
        name: String,
    },
    /// Show the current working tree status
    #[command(visible_alias = "st")]
    Status,
    /// Add file contents to the staging area
    Add {
        #[arg(value_name = "PATH")]
        path: String,
    },
    /// Unstace changes by removing a file from the staging index
    Reset {
        #[arg(value_name = "PATH")]
        path: String,
    },
    /// Join two development histories together
    Merge {
        #[arg(value_name = "BRANCH_OR_COMMIT")]
        target: String,
    },

    /// Configure user options (eg; name and email)
    Config {
        #[arg(long)]
        global: bool,

        #[arg(num_args = 1..=2)]
        args: Vec<String>,
    },

    Worker,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let config = Config {
        ignore_file: ".astriaignore".into(),
        config_folder: ".astria".into() 
    };

    config::CONFIG_GLOBAL.set(config).ok().expect("Failed to set the inicial global config.");
    let cfg = config::CONFIG_GLOBAL.get().unwrap();

    let cli = Cli::parse();
    let current_dir = std::env::current_dir()?;
    let dir = current_dir.join(&cfg.config_folder);

    match cli.command {
        Commands::Init => {
            init_repository(&current_dir)?;
        }
        
        Commands::Commit { message } => {
            if !dir.exists() {
                println!("Error: Astria repository not initialized. Run 'cargo run -- init' first.");
                return Ok(());
            }

            let index_path = dir.join("index");
            if !index_path.exists() {
                println!("No changes added to commit (use 'Astria add <file>').");
                return Ok(());
            }

            let index_data = std::fs::read(&index_path)?;
            let index_map: HashMap<String, String> = bincode::deserialize(&index_data)?;

            if index_map.is_empty() {
                println!("Nothing to commit, index is empty.");
                return Ok(());
            }

            let parent_hash = utils::get_head_commit_hash(&dir)?;

            // Builds tree entrances from index
            let mut entries = Vec::new();
            for (path, blob_hash) in index_map {
                entries.push(TreeEntry {
                    mode: 0o100644,
                    name: path,
                    object_hash: blob_hash,
                });
            }

            // Garantee deterministic ordering
            entries.sort_by(|a, b| a.name.cmp(&b.name));

            let tree = Tree { entries: entries.clone() };
            let tree_hash = tree.save(&dir)?;

            let author = User::get_user(&dir);
            let commit = Commit::new(
                tree_hash.clone(),
                parent_hash,
                message,
                author,
            );
            let commit_main_object = commit.to_astria_object();
            let commit_hash = commit_main_object.save(&dir)?;

            utils::update_head(&dir, &commit_hash)?;

            if let Err(_e) = TaskQueue::push(&dir, &commit_hash, &tree_hash) {
                eprintln!("⚠️ Warning: Commit has been created, but failed to be queued.")
            } else {
                BackgroundWorker::spawn(&dir);
            }

            let _ = std::fs::remove_file(&index_path);

            println!("\n ⚡ Commit created successfully: {}", commit_hash);
        }

        Commands::Query { 
            search,
            func,
            r#struct,
            interface,
            pub_only,
            file,
            commit,
            ai,
        } => {
            if !dir.exists() {
                println!("Error: Astria repository not initialized.");
                return Ok(());
            }

            if let Some(ref ai_query) = ai {
                let db_path_str = dir.to_str().ok_or("Invalid path string")?;
                let indexer: Box<dyn vector::SemanticIndexer> = Box::new(
                    VectorStore::new(db_path_str).await?
                );
                indexer.search(ai_query).await?;
                return Ok(());
            }
        
            let files = match commit {
                Some(ref commit_hash) => {
                    println!("🔎 Querying AST on commit {}...", &commit_hash[..7.min(commit_hash.len())]);
                    utils::load_commit_rust_files(&dir, commit_hash)?
                }
                None => {
                    println!("🔎 Querying AST on workspace...");
                    utils::load_workspace_rust_files(&current_dir, &dir)?
                }
            };

            let mut parser = SemanticParser::new();
            let mut matches_count = 0;
            let filter_types = func || r#struct || interface;

            for (file_path, content) in &files {
                if let Some(ref target_file) = file {
                    if !file_path.contains(target_file) {
                        continue;
                    }
                }

                let symbols = parser.extract_symbols(content);
                let mut file_header_printed = false;

                for (_key, sym) in symbols {
                    if pub_only && !sym.is_public {
                        continue;
                    }

                    if filter_types {
                        let matches_category = match sym.symbol_type.as_str() {
                            "function" | "method" if func => true,
                            "struct" | "class" | "enum" if r#struct => true,
                            "interface" | "trait" if interface => true,
                            _ => false,
                        };
                        if !matches_category {
                            continue;
                        }
                    }

                    if let Some(ref pat) = search {
                        let pat_lower = pat.to_lowercase();
                        let name_lower = sym.name.to_lowercase();
                        if !name_lower.contains(&pat_lower) {
                            continue;
                        }
                    }

                    if !file_header_printed {
                        println!("\n  📄 File: {}", file_path);
                        file_header_printed = true;
                    }

                    let vis_label = if sym.is_public { "[PUB]" } else { "[PRIV]" };
                    println!(
                        "    {} {:<10} {} (line {})",
                        vis_label,
                        sym.symbol_type,
                        sym.name,
                        sym.line_number
                    );

                    matches_count += 1;
                }
            }

            if matches_count == 0 {
                println!("No matching symbols found.");
            } else {
                println!("\nTotal matching symbols found: {}", matches_count);
            }
        }
        Commands::Log => {
            if !dir.exists() {
                println!("Error: Astria repository not initialized.");
                return Ok(());
            }

            let current_commit_opt = utils::get_head_commit_hash(&dir)?;
            let mut current_commit_hash = match current_commit_opt {
                Some(h) => h,
                None => {
                    println!("No commits found in this repository yet.");
                    return Ok(());
                }
            };

            println!("--------------------------------------------------");
            let mut commit_count = 0;

            while !current_commit_hash.is_empty() {
                commit_count += 1;
                
                let obj_path = dir.join("objects").join(&current_commit_hash);
                if !obj_path.exists() {
                    println!("Error: Commit object {} not found.", current_commit_hash);
                    break;
                }

                let compressed_data = std::fs::read(&obj_path)?;
                let serialized_data = zstd::decode_all(&compressed_data[..])?;
                let main_object: Astria = bincode::deserialize(&serialized_data)?;
                let commit_obj: Commit = bincode::deserialize(&main_object.data)?;

                let datetime = chrono::DateTime::from_timestamp(commit_obj.timestamp as i64, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "Unknown time".to_string());

                println!("commit {}", current_commit_hash);
                println!("Tree:   {}", commit_obj.tree_hash);
                println!("Author: {}", commit_obj.author);
                println!("Date:   {}", datetime);
                println!("\n    {}\n", commit_obj.message);

                if let Some(parent) = commit_obj.parent_hash {
                    current_commit_hash = parent;
                    println!("--------------------------------------------------");
                } else {
                    break;
                }
            }

            if commit_count == 0 {
                println!("No commit history available.");
            }
        }
        Commands::Show { hash } => {
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
                        .filter(|e| {
                            e.file_name()
                                .to_string_lossy()
                                .starts_with(&hash)
                        })
                        .collect();
                    if matching.len() == 1 {
                        target_path = matching[0].path();
                    } else if matching.len() > 1 {
                        println!("Ambiguous hash prefix '{}'. Matches multiple objects.", hash);
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
            let main_object: Astria = bincode::deserialize(&serialized_data)?;

            match main_object.obj_type {
                ObjectType::Commit => {
                    let commit_obj: Commit = bincode::deserialize(&main_object.data)?;
                    println!("=== COMMIT OBJECT ===");
                    println!("Tree Hash:   {}", commit_obj.tree_hash);

                    if let Some(parent) = &commit_obj.parent_hash {
                        println!("Parent Hash: {}", parent);
                    } else {
                        println!("Parent Hash: None");
                    }

                    println!("Author:      {}", commit_obj.author);
                    println!("Timestamp:   {}", commit_obj.timestamp);
                    println!("Message:     \n    {}", commit_obj.message);
                }
                ObjectType::Tree => {
                    let tree_obj: Tree = bincode::deserialize(&main_object.data)?;
                    println!("=== TREE OBJECT ===");
                    println!("Entries ({} files):", tree_obj.entries.len());
                    for entry in tree_obj.entries {
                        println!("  mode: {:o} | hash: {} | path: {}", entry.mode, entry.object_hash, entry.name);
                    }
                }
                ObjectType::Blob => {
                    let raw_content: Vec<u8> = bincode::deserialize(&main_object.data)?;
                    println!("=== BLOB OBJECT ===");
                    if let Ok(text) = String::from_utf8(raw_content.clone()) {
                        print!("{}", text);
                    } else {
                        println!("[Binary data: {} bytes]", raw_content.len());
                    }
                }
            }
        }
        Commands::Diff { targets } => {
            if !dir.exists() {
                println!("Error: Astria repository not initialized.");
                return Ok(());
            }

            let (files_old, files_new, label_old, label_new, file_filter) = match targets.len() {
                0 => {
                    let head_commit = match utils::get_head_commit_hash(&dir) {
                        Ok(Some(commit)) => commit,
                        Ok(None) => {
                            println!("No HEAD commits found yet. Run 'ast commit' to create your first commit.");
                            std::process::exit(0);
                        },
                        Err(e) => return Err(e.into()),
                    };
                    let f_head = utils::load_commit_rust_files(&dir, &head_commit)?;
                    let f_ws = utils::load_workspace_rust_files(&current_dir, &dir)?;
                    (f_head, f_ws, format!("HEAD ({})", &head_commit[..7]), "workspace".to_string(), None)
                }
                1 => {
                    let target = &targets[0];
                    let target_path = current_dir.join(target);
                    
                    if target_path.exists() && target_path.is_file() {
                        // Ex: "ast diff src/main.rs" -> Compara o arquivo atual com o HEAD
                        let head_commit = match utils::get_head_commit_hash(&dir) {
                            Ok(Some(commit)) => commit,
                            Ok(None) => {
                                println!("No HEAD commits found yet. Run 'ast commit' to create your first commit.");
                                std::process::exit(0);
                            },
                            Err(e) => return Err(e.into()),
                        };
                        let f_head = utils::load_commit_rust_files(&dir, &head_commit)?;
                        let f_ws = utils::load_workspace_rust_files(&current_dir, &dir)?;
                        (f_head, f_ws, format!("HEAD ({})", &head_commit[..7]), "workspace".to_string(), Some(target.clone()))
                    } else {
                        // Ex: "ast diff <COMMIT_HASH>" -> Compara o commit informado com o Workspace atual
                        let f_a = utils::load_commit_rust_files(&dir, target)?;
                        let f_ws = utils::load_workspace_rust_files(&current_dir, &dir)?;
                        (f_a, f_ws, format!("commit {}", &target[..7.min(target.len())]), "workspace".to_string(), None)
                    }
                }
                2 => {
                    let a = &targets[0];
                    let b = &targets[1];
                    let f_a = utils::load_commit_rust_files(&dir, a)?;
                    let f_b = utils::load_commit_rust_files(&dir, b)?;
                    (f_a, f_b, format!("commit {}", &a[..7.min(a.len())]), format!("commit {}", &b[..7.min(b.len())]), None)
                }
                _ => return Err("Too many arguments for diff.".into()),
            };

            println!("🔍 AST Semantic Diff: {} ➔ {}\n", label_old, label_new);

            let mut parser = SemanticParser::new();
            let mut all_files: BTreeSet<_> = files_old.keys().cloned().collect();
            all_files.extend(files_new.keys().cloned());

            let mut total_changes = 0;

            for file in all_files {
                if let Some(ref target) = file_filter {
                    if !file.contains(target) {
                        continue;
                    }
                }

                let empty = String::new();
                let code_old = files_old.get(&file).unwrap_or(&empty);
                let code_new = files_new.get(&file).unwrap_or(&empty);

                let symbols_old = parser.extract_symbols(code_old);
                let symbols_new = parser.extract_symbols(code_new);

                let mut file_printed = false;
                let mut matched_old = BTreeSet::new();
                let mut matched_new = BTreeSet::new();

                for (key, sym_new) in &symbols_new {
                    if let Some(sym_old) = symbols_old.get(key) {
                        matched_new.insert(key.clone());
                        matched_old.insert(key.clone());
                        if sym_old.content_hash != sym_new.content_hash {
                            if !file_printed {
                                println!("  File: {}", file);
                                file_printed = true;
                            }
                            println!("    [MODIFIED] {} '{}'", sym_new.symbol_type, sym_new.name);
                            total_changes += 1;
                        }
                    }
                }

                let unmatched_new: Vec<_> = symbols_new.iter().filter(|(k, _)| !matched_new.contains(*k)).collect();
                let mut consumed_old = BTreeSet::new();

                for (key_new, sym_new) in unmatched_new {
                    let found_old = symbols_old.iter().find(|(k_old, sym_old)| {
                        !matched_old.contains(*k_old) 
                        && !consumed_old.contains(*k_old)
                        && sym_old.symbol_type == sym_new.symbol_type
                        && sym_old.content_hash == sym_new.content_hash
                    });

                    if let Some((key_old, sym_old)) = found_old {
                        consumed_old.insert(key_old.clone());
                        matched_new.insert(key_new.clone());
                        matched_old.insert(key_old.clone());

                        if !file_printed {
                            println!("  File: {}", file);
                            file_printed = true;
                        }
                        println!("    [RENAMED]  {} '{}' ➔ '{}'", sym_new.symbol_type, sym_old.name, sym_new.name);
                        total_changes += 1;
                    }
                }

                for (key, sym_new) in &symbols_new {
                    if !matched_new.contains(key) {
                        if !file_printed {
                            println!("  File: {}", file);
                            file_printed = true;
                        }
                        println!("    [ADDED]    {} '{}'", sym_new.symbol_type, sym_new.name);
                        total_changes += 1;
                    }
                }

                for (key, sym_old) in &symbols_old {
                    if !matched_old.contains(key) {
                        if !file_printed {
                            println!("  File: {}", file);
                            file_printed = true;
                        }
                        println!("    [REMOVED]  {} '{}'", sym_old.symbol_type, sym_old.name);
                        total_changes += 1;
                    }
                }
            }

            if total_changes == 0 {
                println!("No AST structural changes detected.");
            } else {
                println!("\nTotal structural changes: {}", total_changes);
            }
        }
        Commands::Branch { name } => {
            if !dir.exists() {
                println!("Error: Astria repository not initialized.");
                return Ok(());
            }
            let heads_dir = dir.join("refs").join("heads");
            
            match name {
                Some(branch_name) => {
                    let head_commit = utils::get_head_commit_hash(&dir)?.unwrap_or_default();
                    let branch_path = heads_dir.join(&branch_name);
                    if let Some(parent) = branch_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }

                    std::fs::write(&branch_path, &head_commit)?;
                    if head_commit.is_empty() {
                        println!("Branch '{}' created (empty, no commits yet).", branch_name);
                    } else {        
                        println!("Branch '{}' created at commit {}", branch_name, &head_commit[..7.min(head_commit.len())]);
                    }
                }
                None => {
                    let current_head_content = std::fs::read_to_string(dir.join("HEAD"))?.trim().to_string();
                    let current_branch = if current_head_content.starts_with("ref: refs/heads/") {
                        current_head_content[16..].trim().to_string()
                    } else {
                        String::new()
                    };

                    if heads_dir.exists() {
                        println!("Branches:");
                        let mut branches = Vec::new();
                        utils::collect_branch_names(&heads_dir, &heads_dir, &mut branches)?;

                        branches.sort();

                        for branch_name in branches {
                            if branch_name == current_branch {
                                println!("* {}", branch_name);
                            } else {
                                println!("  {}", branch_name);
                            }
                        }
                    } else {
                        println!("No branches found.");
                    }
                }
            }
        }
        Commands::Checkout { name } => {
            if !dir.exists() {
                println!("Error: Astria repository not initialized.");
                return Ok(());
            }

            let heads_dir = dir.join("refs").join("heads");
            let target_branch_file = heads_dir.join(&name);

            if target_branch_file.is_file() {
                let head_path = dir.join("HEAD");
                std::fs::write(&head_path, format!("ref: refs/heads/{}", name))?;
                println!("Switched to branch '{}'", name);
            } else {
                let commit_path = dir.join("objects").join(&name);
                if commit_path.exists() {
                    let head_path = dir.join("HEAD");
                    std::fs::write(&head_path, &name)?;
                    println!("Switched to detached HEAD at commit {}", &name[..7.min(name.len())]);
                } else {
                    println!("Error: Branch or commit '{}' not found.", name);
                }
            }
        }

        Commands::Status => {

            if !dir.exists() {
                println!("Error: Astria repository not initialized.");
                return Ok(());
            }

            let head_content = std::fs::read_to_string(dir.join("HEAD"))?.trim().to_string();
            if head_content.starts_with("ref: refs/heads/") {
                println!("On branch {}", &head_content[16..]);
            } else {
                println!("HEAD detached at {}", &head_content[7.min(head_content.len())..]);
            }

            let head_commit_opt = utils::get_head_commit_hash(&dir)?;
            let committed_files = if let Some(ref head_commit) = head_commit_opt {
                utils::load_commit_rust_files(&dir, head_commit)?
            } else {
                HashMap::new()
            };

            let index_path = dir.join("index");
            let index_map: HashMap<String, String> = if index_path.exists() {
                let data = std::fs::read(&index_path).unwrap_or_default();
                bincode::deserialize(&data).unwrap_or_default()
            } else {
                HashMap::new()
            };

            let workspace_files = utils::load_workspace_rust_files(&current_dir, &dir)?;

            let mut staged = Vec::new();
            let mut modified = Vec::new();
            let mut untracked = Vec::new();

            for path in index_map.keys() {
                staged.push(path.clone());
            }

            for (path, content) in &workspace_files {
                if !index_map.contains_key(path) {
                    if let Some(old_content) = committed_files.get(path) {
                        if old_content != content {
                            modified.push(path.clone());
                        }
                    } else {
                        untracked.push(path.clone());
                    }
                }
            }

            if !staged.is_empty() {
                println!("\nChanges to be committed:");
                for f in &staged {
                    println!("  \x1b[32mstaged:     {}\x1b[0m", f); // Verde
                }
            }

            if !modified.is_empty() {
                println!("\nChanges not staged for commit:");
                for f in &modified {
                    println!("  \x1b[31mmodified:   {}\x1b[0m", f); // Vermelho
                }
            }

            if !untracked.is_empty() {
                println!("\nUntracked files:");
                for f in &untracked {
                    println!("  \x1b[31mnew file:   {}\x1b[0m", f); // Vermelho
                }
            }

            if staged.is_empty() && modified.is_empty() && untracked.is_empty() {
                println!("\nNothing to commit, working tree clean.");
            }
        }

        Commands::Add { path } => {
            if !dir.exists() {
                println!("Error: Astria repository not initialized.");
                return Ok(());
            }

            let file_path = current_dir.join(&path);
            if !file_path.exists() || !file_path.is_file() {
                println!("Error: File '{}' not found or is not a file.", path);
                return Ok(());
            }

            // 1. Lê o conteúdo e cria o Blob no CAS (.Astria/objects/)
            let content = std::fs::read(&file_path)?;
            let blob = Blob::new(content);
            let blob_hash = blob.save(&dir)?;

            // 2. Atualiza o arquivo de índice (.Astria/index)
            let index_path = dir.join("index");
            let mut index_map: HashMap<String, String> = if index_path.exists() {
                let data = std::fs::read(&index_path)?;
                bincode::deserialize(&data).unwrap_or_default()
            } else {
                HashMap::new()
            };

            let rel_path = path.replace('\\', "/");
            index_map.insert(rel_path.clone(), blob_hash.clone());

            let serialized_index = bincode::serialize(&index_map)?;
            std::fs::write(&index_path, serialized_index)?;

            println!("add '{}' (blob: {})", rel_path, &blob_hash[..7]);
        }

        Commands::Reset { path } => {
            if !dir.exists() {
                println!("Error: Astria repository not initialized.");
                return Ok(());
            }

            let index_path = dir.join("index");
            if !index_path.exists() {
                println!("No changes staged (index is empty).");
                return Ok(());
            }

            let data = std::fs::read(&index_path)?;
            let mut index_map: HashMap<String, String> = bincode::deserialize(&data).unwrap_or_default();

            let rel_path = path.replace('\\', "/");
            if index_map.remove(&rel_path).is_some() {
                let serialized_index = bincode::serialize(&index_map)?;
                std::fs::write(&index_path, serialized_index)?;
                println!("unstaged '{}'", rel_path);
            } else {
                println!("Error: File '{}' is not staged in index.", rel_path);
            }
        }

        Commands::Merge { target } => {
            if !dir.exists() {
                println!("Error: Astria repository not initialized.");
                return Ok(());
            }

            // 1. Resolves destination commit (can be a branch name or a hash number)
            let heads_dir = dir.join("refs").join("heads");
            let target_branch_file = heads_dir.join(&target);
            
            let target_commit_hash = if target_branch_file.exists() {
                std::fs::read_to_string(&target_branch_file)?.trim().to_string()
            } else {
                let commit_path = dir.join("objects").join(&target);
                if commit_path.exists() {
                    target.clone()
                } else {
                    println!("Error: Branch or commit '{}' not found.", target);
                    return Ok(());
                }
            };

            let current_head_content = std::fs::read_to_string(dir.join("HEAD"))?.trim().to_string();
            
            // 2. Check if we're on a branch or HEAD
            if current_head_content.starts_with("ref: refs/heads/") {
                let current_branch = current_head_content[16..].trim().to_string();
                let branch_file = heads_dir.join(&current_branch);
                
                // Updates current branch's pointer for destination commmit (Fast-Forward)
                std::fs::write(&branch_file, &target_commit_hash)?;
                println!("Updating {}..{}", &utils::get_head_commit_hash(&dir)?.unwrap_or_default()[..7], &target_commit_hash[..7]);
                println!("Fast-forward merge successful. Branch '{}' now points to {}", current_branch, &target_commit_hash[..7]);
            } else {
                // HEAD: only updates the HEAD
                utils::update_head(&dir, &target_commit_hash)?;
                println!("Fast-forward merge successful. HEAD is now at {}", &target_commit_hash[..7]);
            }

            // 3. Optional. Recovers the merged files in the commit for the current workspace
            let merged_files = utils::load_commit_rust_files(&dir, &target_commit_hash)?;
            for (file_path, content) in merged_files {
                let dest_path = current_dir.join(&file_path);
                if let Some(parent) = dest_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&dest_path, content)?;
            }

            println!("Workspace successfully synchronized with merged commit.");
            std::process::exit(0);
        }

        Commands::Config { global, args } => {
            if args.len() >= 2 {
                let key = &args[0];
                let raw = args[1..].join(" ");
                let value = raw.trim_matches('"').trim_matches('\'');

                match User::set(global, key, value) {
                    Ok(_) => println!("Your global configurations have been set."),
                    Err(e) => eprintln!("Failed to set: {}", e),
                }
                
            } else {
                println!("Usage: ast config [--global] user.name \"Your nane\"");
                println!("       ast config [--global] user.email \"your@email.com\"");
            }
        }

        Commands::Worker => {

            if !dir.exists() {
                println!("Error. Astria repository not initialized.");
                return Ok(());
            }

            let _ = BackgroundWorker::run(&dir);
        }
    }

    Ok(())
}