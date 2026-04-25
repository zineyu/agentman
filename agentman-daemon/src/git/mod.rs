pub mod workspace;

use std::path::Path;
use std::process::Command;
use thiserror::Error;
use tracing::{debug, error, info, warn};

#[derive(Debug, Error)]
pub enum GitError {
    #[error("Git command failed: {0}")]
    CommandFailed(String),

    #[error("Invalid repository URL: {0}")]
    InvalidUrl(String),

    #[error("Repository not found at {0}")]
    RepoNotFound(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("UTF-8 decode error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),
}

pub struct GitManager;

impl GitManager {
    pub fn new() -> Self {
        Self
    }

    pub fn clone_repo(
        &self,
        repo_url: &str,
        target_dir: &Path,
    ) -> Result<(), GitError> {
        info!("Cloning repository {} to {:?}", repo_url, target_dir);

        if repo_url.is_empty() {
            return Err(GitError::InvalidUrl("Empty repository URL".to_string()));
        }

        if let Some(parent) = target_dir.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let output = Command::new("git")
            .args(["clone", repo_url, &target_dir.to_string_lossy()])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Git clone failed: {}", stderr);
            return Err(GitError::CommandFailed(format!(
                "git clone failed: {}",
                stderr
            )));
        }

        info!("Successfully cloned repository to {:?}", target_dir);
        Ok(())
    }

    pub fn checkout_branch(
        &self,
        repo_dir: &Path,
        branch: &str,
    ) -> Result<(), GitError> {
        info!("Checking out branch '{}' in {:?}", branch, repo_dir);

        if branch.is_empty() {
            return Err(GitError::CommandFailed("Empty branch name".to_string()));
        }

        if !repo_dir.exists() {
            return Err(GitError::RepoNotFound(
                repo_dir.to_string_lossy().to_string(),
            ));
        }

        let branch_exists = self.branch_exists(repo_dir, branch)?;
        let args = if branch_exists {
            vec!["checkout", branch]
        } else {
            vec!["checkout", "-b", branch]
        };

        let output = Command::new("git")
            .current_dir(repo_dir)
            .args(args)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Git checkout failed: {}", stderr);
            return Err(GitError::CommandFailed(format!(
                "git checkout failed: {}",
                stderr
            )));
        }

        info!(
            "Successfully {} branch '{}'",
            if branch_exists { "checked out" } else { "created" },
            branch
        );
        Ok(())
    }

    pub fn get_current_commit(
        &self,
        repo_dir: &Path,
    ) -> Result<String, GitError> {
        debug!("Getting current commit hash for {:?}", repo_dir);

        if !repo_dir.exists() {
            return Err(GitError::RepoNotFound(
                repo_dir.to_string_lossy().to_string(),
            ));
        }

        let output = Command::new("git")
            .current_dir(repo_dir)
            .args(["rev-parse", "HEAD"])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::CommandFailed(format!(
                "git rev-parse failed: {}",
                stderr
            )));
        }

        let commit_hash = String::from_utf8(output.stdout)?.trim().to_string();
        debug!("Current commit hash: {}", commit_hash);

        Ok(commit_hash)
    }

    fn branch_exists(
        &self,
        repo_dir: &Path,
        branch: &str,
    ) -> Result<bool, GitError> {
        let output = Command::new("git")
            .current_dir(repo_dir)
            .args(["branch", "--list", branch])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::CommandFailed(format!(
                "git branch --list failed: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8(output.stdout)?;
        Ok(!stdout.trim().is_empty())
    }

    pub fn get_remote_url(&self, repo_dir: &Path) -> Result<String, GitError> {
        let output = Command::new("git")
            .current_dir(repo_dir)
            .args(["remote", "get-url", "origin"])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(GitError::CommandFailed(format!(
                "git remote get-url failed: {}",
                stderr
            )));
        }

        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    }

    pub fn config_user(
        &self,
        repo_dir: &Path,
        name: &str,
        email: &str,
    ) -> Result<(), GitError> {
        let output_name = Command::new("git")
            .current_dir(repo_dir)
            .args(["config", "user.name", name])
            .output()?;

        if !output_name.status.success() {
            let stderr = String::from_utf8_lossy(&output_name.stderr);
            return Err(GitError::CommandFailed(format!(
                "git config user.name failed: {}",
                stderr
            )));
        }

        let output_email = Command::new("git")
            .current_dir(repo_dir)
            .args(["config", "user.email", email])
            .output()?;

        if !output_email.status.success() {
            let stderr = String::from_utf8_lossy(&output_email.stderr);
            return Err(GitError::CommandFailed(format!(
                "git config user.email failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    pub fn pull(&self, repo_dir: &Path, branch: &str) -> Result<(), GitError> {
        info!("Pulling latest changes for branch '{}'", branch);

        let output = Command::new("git")
            .current_dir(repo_dir)
            .args(["pull", "origin", branch])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Git pull had issues: {}", stderr);
        } else {
            info!("Successfully pulled latest changes");
        }

        Ok(())
    }
}

impl Default for GitManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
