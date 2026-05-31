use serde::{Deserialize, Serialize};
use std::{env, fs, path::PathBuf};

const DEFAULT_PROFILE_ID: &str = "default";
const ACTIVE_PROFILE_FILE: &str = "active_profile";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CodexProfile {
    pub id: String,
    pub label: String,
    pub home: PathBuf,
    pub managed: bool,
}

#[derive(Clone, Debug)]
pub struct CodexProfileStore {
    root: PathBuf,
}

impl CodexProfileStore {
    pub fn new() -> Self {
        Self {
            root: paths::data_dir().join("codex").join("accounts"),
        }
    }

    pub fn active_profile(&self) -> CodexProfile {
        let active_id = fs::read_to_string(self.root.join(ACTIVE_PROFILE_FILE))
            .ok()
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| DEFAULT_PROFILE_ID.to_string());

        self.profiles()
            .into_iter()
            .find(|profile| profile.id == active_id)
            .unwrap_or_else(default_profile)
    }

    pub fn set_active_profile(&self, id: String) {
        let _ = fs::create_dir_all(&self.root);
        let _ = fs::write(self.root.join(ACTIVE_PROFILE_FILE), id);
    }

    pub fn profiles(&self) -> Vec<CodexProfile> {
        let mut profiles = vec![default_profile()];

        if let Ok(entries) = fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let Some(id) = path.file_name().and_then(|name| name.to_str()) else {
                    continue;
                };
                profiles.push(CodexProfile {
                    id: id.to_string(),
                    label: id.replace('-', " "),
                    home: path,
                    managed: true,
                });
            }
        }

        profiles
    }
}

fn default_profile() -> CodexProfile {
    let home = env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| paths::home_dir().join(".codex"));

    CodexProfile {
        id: DEFAULT_PROFILE_ID.to_string(),
        label: "Default".to_string(),
        home,
        managed: false,
    }
}
