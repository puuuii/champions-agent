use std::path::{Path, PathBuf};

pub struct AppPaths {
    pub bundled_resources_dir: PathBuf,
    pub master_data_dir: PathBuf,
    pub model_dir: PathBuf,
    pub pokemon_images_dir: PathBuf,
    pub user_data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub debug_dir: PathBuf,
}

impl AppPaths {
    pub fn from_project_root(root: &Path) -> Self {
        let resources_dir = root.join("resources");
        Self {
            bundled_resources_dir: resources_dir.clone(),
            master_data_dir: resources_dir.join("master_data"),
            model_dir: resources_dir.join("models"),
            pokemon_images_dir: resources_dir.join("pokemon_images"),
            user_data_dir: root.join("user_data"),
            cache_dir: root.join("cache"),
            debug_dir: root.join("debug"),
        }
    }

    pub fn ensure_writable_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.user_data_dir)?;
        std::fs::create_dir_all(&self.cache_dir)?;
        std::fs::create_dir_all(&self.debug_dir)?;
        Ok(())
    }

    pub fn party_json_path(&self) -> PathBuf {
        self.user_data_dir.join("party.json")
    }

    pub fn usage_json_path(&self) -> PathBuf {
        self.cache_dir.join("usage.json")
    }
}
