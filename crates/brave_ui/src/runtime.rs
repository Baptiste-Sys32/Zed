use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Debug)]
pub struct BraveRuntime {
    pub executable: Option<PathBuf>,
    pub user_data_dir: Option<PathBuf>,
}

impl BraveRuntime {
    pub fn detect() -> Self {
        Self {
            executable: detect_executable(),
            user_data_dir: default_user_data_dir(),
        }
    }

    pub fn launch_command(&self, remote_debugging_port: u16, url: &str) -> Option<Command> {
        let executable = self.executable.as_ref()?;
        let mut command = Command::new(executable);
        command
            .arg(format!("--remote-debugging-port={remote_debugging_port}"))
            .arg("--remote-debugging-address=127.0.0.1")
            .arg("--remote-allow-origins=http://127.0.0.1")
            .arg("--no-first-run");

        if let Some(user_data_dir) = &self.user_data_dir {
            command.arg(format!("--user-data-dir={}", user_data_dir.display()));
        }

        command.arg(url);
        Some(command)
    }
}

fn detect_executable() -> Option<PathBuf> {
    if let Ok(explicit) = env::var("ZED_BRAVE_PATH") {
        let path = PathBuf::from(explicit);
        if path.exists() {
            return Some(path);
        }
    }

    for candidate in executable_candidates() {
        if candidate.components().count() > 1 {
            if candidate.exists() {
                return Some(candidate);
            }
        } else if let Some(path) = find_on_path(&candidate) {
            return Some(path);
        }
    }

    None
}

fn executable_candidates() -> Vec<PathBuf> {
    if cfg!(target_os = "macos") {
        vec![
            "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser".into(),
            "brave-browser".into(),
            "brave".into(),
        ]
    } else if cfg!(target_os = "windows") {
        let mut candidates = Vec::new();
        if let Ok(program_files) = env::var("ProgramFiles") {
            candidates.push(
                Path::new(&program_files).join("BraveSoftware/Brave-Browser/Application/brave.exe"),
            );
        }
        if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
            candidates.push(
                Path::new(&local_app_data)
                    .join("BraveSoftware/Brave-Browser/Application/brave.exe"),
            );
        }
        candidates
    } else {
        vec![
            "brave-browser".into(),
            "brave-browser-stable".into(),
            "brave".into(),
            "/usr/bin/brave-browser".into(),
            "/snap/bin/brave".into(),
            "/var/lib/flatpak/exports/bin/com.brave.Browser".into(),
        ]
    }
}

fn default_user_data_dir() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        Some(paths::home_dir().join("Library/Application Support/BraveSoftware/Brave-Browser"))
    } else if cfg!(target_os = "windows") {
        env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|dir| dir.join("BraveSoftware/Brave-Browser/User Data"))
    } else {
        let config_home = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| paths::home_dir().join(".config"));
        Some(config_home.join("BraveSoftware/Brave-Browser"))
    }
}

fn find_on_path(executable: &Path) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|dir| dir.join(executable))
        .find(|candidate| candidate.exists())
}
