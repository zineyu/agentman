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
            info!("{}", rust_i18n::t!("workspace.created", path = workspace_path.display()));
        } else {
            debug!("{}", rust_i18n::t!("workspace.already_exists", path = workspace_path.display()));
        }

        workspace_path
    }

    pub fn cleanup_workspace(&self,
        task_id: &str,
    ) -> anyhow::Result<()> {
        let workspace_path = self.base_dir.join(task_id);

        if workspace_path.exists() {
            std::fs::remove_dir_all(&workspace_path)?;
            info!("{}", rust_i18n::t!("workspace.cleaned_up", path = workspace_path.display()));
        } else {
            warn!(
                "{}",
                rust_i18n::t!("workspace.does_not_exist", path = workspace_path.display())
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
