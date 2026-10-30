use astria::objects::commit::commit::Commit;
use astria::utils::commit::CommitUtils;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_dag_multiple_parents_and_integrity() {
    let dir = tempdir().unwrap();
    let repo_path = dir.path();
    let objects_dir = repo_path.join("objects");
    fs::create_dir_all(&objects_dir).unwrap();

    // 1. Simulate mock parent hashes and tree hash in the CAS
    let parent_hash_1 = "abc123hashparent1";
    let parent_hash_2 = "def456hashparent2";
    let tree_hash = "789treehashxyz";

    // Create dummy files representing objects stored in the CAS to pass validation
    fs::write(objects_dir.join(parent_hash_1), b"mock commit 1").unwrap();
    fs::write(objects_dir.join(parent_hash_2), b"mock commit 2").unwrap();
    fs::write(objects_dir.join(tree_hash), b"mock tree").unwrap();

    // 2. Test object existence cryptographic validation
    assert!(CommitUtils::validate_object_exists(repo_path, parent_hash_1).is_ok());
    assert!(CommitUtils::validate_object_exists(repo_path, "phantom_hash").is_err());

    // 3. Create a commit with multiple parents (Merge DAG support)
    let commit = Commit::new(
        tree_hash.to_string(),
        vec![parent_hash_1.to_string(), parent_hash_2.to_string()],
        "Merge branch feature into main".to_string(),
        "Tiago Perrelli <tiago@valy.com>".to_string(),
    );

    // Validate if parents were correctly stored in the vector
    assert_eq!(commit.parents.len(), 2);
    assert_eq!(commit.parents[0], parent_hash_1);
    assert_eq!(commit.parents[1], parent_hash_2);
}
