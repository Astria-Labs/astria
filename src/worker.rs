use std::fs;
use std::path::Path;
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

        println!("👷 Astria Background Worker iniciado. Monitorando fila de tarefas...");

        let repo_path = repo_dir.to_path_buf();

        // Roda em uma thread nativa do SO, totalmente isolada de qualquer contexto CLI/Tokio pré-existente
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("❌ Falha fatal ao iniciar runtime do Worker: {}", e);
                    return;
                }
            };

            rt.block_on(async {
                loop {
                    match TaskQueue::pop(&repo_path) {
                        Ok(Some((task_path, task))) => {
                            println!("⚙️ Processing semantic for commit: {}", &task.commit_hash[..7]);
                            
                            let result = Self::process_commit_semantics(&repo_path, &task.tree_hash).await;

                            match result {
                                Ok(count) => {
                                    println!("✨ Concluído: {} chunks indexados para o commit {}", count, &task.commit_hash[..7]);
                                    let _ = fs::remove_file(task_path);
                                }
                                Err(e) => {
                                    eprintln!("❌ Erro ao processar semântica do commit {}: {}", &task.commit_hash[..7], e);
                                    let _ = fs::remove_file(task_path);
                                }
                            }
                        }
                        Ok(None) => {
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        }
                        Err(e) => {
                            eprintln!("⚠️ Erro ao ler a fila de tarefas: {}", e);
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
}