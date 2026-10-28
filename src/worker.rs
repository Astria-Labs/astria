use std::fs;
use std::path::Path;
use std::process::Stdio;
use std::process::Command;
use crate::queue::TaskQueue;
use crate::parser::SemanticParser;
use crate::vector::{VectorStore, SemanticIndexer};
use crate::objects::tree::tree::Tree;
use crate::objects::astria::astria::Astria;

pub struct BackgroundWorker;

impl BackgroundWorker {

    pub fn run(repo_dir: &Path) -> std::io::Result<()> {
        if !repo_dir.exists() {
            println!("Error: Astria repository not initialized.");
            return Ok(());
        }

        let lock_file = repo_dir.join("worker.lock");

        if lock_file.exists() {
            if let Ok(pid_str) = fs::read_to_string(&lock_file) {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    let is_running = if cfg!(target_os = "windows") {
                        Self::kill_windows_process(&pid)
                    } else {
                        Self::kill_unix_process(&pid)
                    };

                    if is_running {
                        return Ok(());
                    } else {
                        let _ = fs::remove_file(&lock_file);
                    }
                } else {
                    let _ = fs::remove_file(&lock_file);
                }
            } else {
                let _ = fs::remove_file(&lock_file);
            }
        }

        if let Err(_) = fs::write(&lock_file, std::process::id().to_string()) {
            return Ok(());
        }

        struct LockGuard(std::path::PathBuf);
        impl Drop for LockGuard {
            fn drop(&mut self) {
                let _ = fs::remove_file(&self.0);
            }
        }
        let _guard = LockGuard(lock_file);

        println!("👷 Astria Background Worker started.");

        let repo_path = repo_dir.to_path_buf();

        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("❌ Fatal failure to start Worker runtime: {}", e);
                    return;
                }
            };

            let idle_limit = std::time::Duration::from_secs(30);
            let mut last_activity = std::time::Instant::now();

            rt.block_on(async {
                loop {
                    match TaskQueue::pop(&repo_path) {
                        Ok(Some((task_path, task))) => {
                            last_activity = std::time::Instant::now();
                            println!("⚙️ Processing semantic for commit: {}", &task.commit_hash[..7]);
                            let result = Self::process_commit_semantics(&repo_path, &task.tree_hash).await;

                            match result {
                                Ok(count) => {
                                    println!("✨ Finished: {} chunks indexed for commit {}", count, &task.commit_hash[..7]);
                                    let _ = fs::remove_file(task_path);
                                }
                                Err(e) => {
                                    eprintln!("❌ Failed to process commit semantics {}: {}", &task.commit_hash[..7], e);
                                    let _ = fs::remove_file(task_path);
                                }
                            }
                        }
                        Ok(None) => {
                            if last_activity.elapsed() > idle_limit {
                                println!("💤 Worker will terminated due to inactivity...");
                                break;
                            }
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        }
                        Err(e) => {
                            eprintln!("⚠️ Failed to read queue tasks: {}", e);
                            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                        }
                    }
                }
            });
        });

        // Mantém a thread principal do CLI viva para o worker continuar rodando
        loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
        }
    }

    /// Internal logic that reads a Tree and processes SemanticParser + VectorStore
    async fn process_commit_semantics(repo_dir: &Path, tree_hash: &str) -> Result<usize, Box<dyn std::error::Error>> {
        let tree_path = repo_dir.join("objects").join(tree_hash);
        if !tree_path.exists() {
            return Err(format!("Tree object {} not found", tree_hash).into());
        }

        let compressed_data = fs::read(&tree_path)?;
        let serialized_data = zstd::decode_all(&compressed_data[..])?;
        let main_object: Astria = bincode::deserialize(&serialized_data)?;
        let tree_obj: Tree = bincode::deserialize(&main_object.data)?;

        let mut parser = SemanticParser::new();
        let mut all_chunks = Vec::new();
        let project_root = repo_dir.parent().ok_or("Invalid repository parent path")?;

        for entry in &tree_obj.entries {
            if entry.name.ends_with(".rs") {
                let file_path = project_root.join(&entry.name);
                if let Ok(content) = std::fs::read_to_string(file_path) {
                    let chunks = parser.parse_rust_code(&content, &entry.name);
                    all_chunks.extend(chunks);
                }
            }
        }

        if !all_chunks.is_empty() {
            let db_path = repo_dir.to_str().ok_or("Invalid path string")?;
            let indexer: Box<dyn SemanticIndexer> = Box::new(
                VectorStore::new(db_path).await?
            );
            indexer.store_chunks(&all_chunks).await?;
        }

        Ok(all_chunks.len())
    }

    pub fn spawn(repo_dir: &Path) {
        let lock_file = repo_dir.join("worker.lock");

        if lock_file.exists() {
            return; 
        }

        if let Ok(exe_path) = std::env::current_exe() {
            let _ = Command::new(exe_path)
                .arg("worker")
                .arg("--daemon")
                .current_dir(repo_dir.parent().unwrap_or(repo_dir))
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
        }
    }

    fn kill_windows_process(pid: &u32) -> bool {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid)])
            .output()
            .map(|output| {
                let stdout = String::from_utf8_lossy(&output.stdout);
                stdout.contains(&pid.to_string())
            })
            .unwrap_or(false)

    }

    fn kill_unix_process(pid: &u32) -> bool {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}