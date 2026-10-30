use serde::{Deserialize, Serialize};
use serde_json;
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Task {
    SemanticIndex {
        commit_hash: String,
        tree_hash: String,
    },
    HydrateBlob {
        remote: String,
        url: String,
        repository: String,
        auth_token: String,
        path: String,
        hash: String,
    },
}

pub struct TaskQueue;

impl TaskQueue {
    /// Enfileira uma nova tarefa de forma atômica e rápida

    /// Queues a new task
    pub fn push(repo_dir: &Path, commit_hash: &str, tree_hash: &str) -> std::io::Result<()> {
        let queue_dir = repo_dir.join("queue");
        fs::create_dir_all(&queue_dir)?;

        let task = Task::SemanticIndex {
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
    pub fn push_hydrate(
        repo_dir: &Path,
        remote: &str,
        url: &str,
        repository: &str,
        auth_token: &str,
        path: &str,
        hash: &str,
    ) -> std::io::Result<()> {
        let queue_dir = repo_dir.join("queue");
        fs::create_dir_all(&queue_dir)?;

        let task = Task::HydrateBlob {
            remote: remote.to_string(),
            url: url.to_string(),
            repository: repository.to_string(),
            auth_token: auth_token.to_string(),
            path: path.to_string(),
            hash: hash.to_string(),
        };

        let timestamp = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let short_hash = &hash[..std::cmp::min(8, hash.len())];
        let task_filename = format!("{}_hydrate_{}_{}.json", timestamp, remote, short_hash);
        let task_path = queue_dir.join(task_filename);
        let data = serde_json::to_string(&task)?;

        fs::write(task_path, data)?;
        Ok(())
    }

    pub fn pop(repo_dir: &Path) -> std::io::Result<Option<(std::path::PathBuf, Task)>> {
        let queue_dir = repo_dir.join("queue");
        if !queue_dir.exists() {
            return Ok(None);
        }

        for entry in fs::read_dir(&queue_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(data) = fs::read_to_string(&path) {
                    if let Ok(task) = serde_json::from_str::<Task>(&data) {
                        return Ok(Some((path, task)));
                    }
                }
            }
        }

        Ok(None)
    }
}
