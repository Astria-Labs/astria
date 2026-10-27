use std::fs;
use serde_json;
use std::path::Path;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IndexingTask {
    pub commit_hash: String,
    pub tree_hash: String,
}

pub struct TaskQueue;

impl TaskQueue {
    /// Enfileira uma nova tarefa de forma atômica e rápida

    /// Queues a new task 
    pub fn push(repo_dir: &Path, commit_hash: &str, tree_hash: &str) -> std::io::Result<()> {
        let queue_dir = repo_dir.join("queue");
        fs::create_dir_all(&queue_dir)?;

        let task = IndexingTask {
            commit_hash: commit_hash.to_string(),
            tree_hash: tree_hash.to_string(),
        };

    
        // Unique ID based on nanosecond timestamp + commit suffix
        let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let short_hash = &commit_hash[..std::cmp::min(8, commit_hash.len())];
        let task_filename = format!("{}_{}.json", timestamp, short_hash);
        
        let task_path = queue_dir.join(task_filename);
        let data = serde_json::to_string(&task)?;
        
        fs::write(task_path, data)?;
        Ok(())
    }

    /// Returns the next pending task in the queue
    pub fn pop(repo_dir: &Path) -> std::io::Result<Option<(std::path::PathBuf, IndexingTask)>> {
        let queue_dir = repo_dir.join("queue");
        if !queue_dir.exists() {
            return Ok(None);
        }

        for entry in fs::read_dir(&queue_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(data) = fs::read_to_string(&path) {
                    if let Ok(task) = serde_json::from_str::<IndexingTask>(&data) {
                        return Ok(Some((path, task)));
                    }
                }
            }
        }

        Ok(None)
    }
}