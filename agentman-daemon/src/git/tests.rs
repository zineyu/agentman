use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

use super::{GitError, GitManager};

#[test]
fn test_clone_invalid_url() {
    let git = GitManager::new();
    let temp_dir = TempDir::new().unwrap();
    let result = git.clone_repo("", temp_dir.path());
    assert!(result.is_err());
    match result.unwrap_err() {
        GitError::InvalidUrl(msg) => assert!(msg.contains("Empty")),
        other => panic!("Expected InvalidUrl error, got: {:?}", other),
    }
}

#[test]
fn test_checkout_empty_branch() {
    let git = GitManager::new();
    let temp_dir = TempDir::new().unwrap();
    let result = git.checkout_branch(temp_dir.path(), "");
    assert!(result.is_err());
    match result.unwrap_err() {
        GitError::CommandFailed(msg) => assert!(msg.contains("Empty")),
        other => panic!("Expected CommandFailed error, got: {:?}", other),
    }
}

#[test]
fn test_checkout_nonexistent_repo() {
    let git = GitManager::new();
    let nonexistent = Path::new("/tmp/nonexistent_repo_for_testing_12345");
    let result = git.checkout_branch(nonexistent, "main");
    assert!(result.is_err());
    match result.unwrap_err() {
        GitError::RepoNotFound(path) => {
            assert!(path.contains("nonexistent_repo_for_testing_12345"))
        }
        other => panic!("Expected RepoNotFound error, got: {:?}", other),
    }
}

#[test]
#[ignore = "requires git to be installed"]
fn test_clone_and_checkout_success() {
    let git = GitManager::new();
    let source_dir = TempDir::new().unwrap();
    let init_output = Command::new("git")
        .args(["init"])
        .current_dir(source_dir.path())
        .output()
        .expect("git init failed - is git installed?");
    assert!(init_output.status.success(), "git init failed");
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(source_dir.path())
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(source_dir.path())
        .output()
        .unwrap();
    std::fs::write(source_dir.path().join("README.md"), "# Test Repository").unwrap();
    let add_output = Command::new("git")
        .args(["add", "."])
        .current_dir(source_dir.path())
        .output()
        .expect("git add failed");
    assert!(add_output.status.success(), "git add failed");

    let commit_output = Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(source_dir.path())
        .output()
        .expect("git commit failed");
    assert!(
        commit_output.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&commit_output.stderr)
    );
    let target_dir = TempDir::new().unwrap();
    let cloned_path = target_dir.path().join("cloned");
    let result = git.clone_repo(source_dir.path().to_str().unwrap(), &cloned_path);
    assert!(result.is_ok(), "Clone failed: {:?}", result);
    assert!(cloned_path.exists());
    assert!(cloned_path.join(".git").exists());
    assert!(cloned_path.join("README.md").exists());
    let result = git.checkout_branch(&cloned_path, "test-branch");
    assert!(result.is_ok(), "Checkout failed: {:?}", result);
    let branch_output = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(&cloned_path)
        .output()
        .unwrap();
    let current_branch = String::from_utf8_lossy(&branch_output.stdout).trim().to_string();
    assert_eq!(current_branch, "test-branch");
}
