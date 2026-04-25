use std::path::PathBuf;
use tracing::{debug, info, warn};

pub struct WorkspaceManager {
    base_dir: PathBuf,
}

impl WorkspaceManager {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    pub fn prepare_workspace(&self,
        task_id: &str,
    ) -> PathBuf {
        let workspace_path = self.base_dir.join(task_id);

        if !workspace_path.exists() {
            std::fs::create_dir_all(&workspace_path)
                .expect("Failed to create workspace directory");
            info!("Created workspace at {:?}", workspace_path);
        } else {
            debug!("Workspace already exists at {:?}", workspace_path);
        }

        workspace_path
    }

    pub fn cleanup_workspace(&self,
        task_id: &str,
    ) -> anyhow::Result<()> {
        let workspace_path = self.base_dir.join(task_id);

        if workspace_path.exists() {
            std::fs::remove_dir_all(&workspace_path)?;
            info!("Cleaned up workspace at {:?}", workspace_path);
        } else {
            warn!(
                "Workspace does not exist, skipping cleanup: {:?}",
                workspace_path
            );
        }

        Ok(())
    }

    pub fn workspace_exists(&self, task_id: &str) -> bool {
        self.base_dir.join(task_id).exists()
    }

    pub fn get_workspace_path(&self,
        task_id: &str,
    ) -> PathBuf {
        self.base_dir.join(task_id)
    }
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new("./workspace")
    }
}
