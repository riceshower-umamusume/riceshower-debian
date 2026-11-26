use crate::atomic_file;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const STATE_DIR: &str = "/var/lib/noctalia-setup";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct State {
    pub schema: u32,
    pub distro: String,
    #[serde(default)]
    pub components: BTreeMap<String, InstalledComponent>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledComponent {
    pub commit: String,
    pub method: String,
    #[serde(default)]
    pub files: Vec<String>,
}

pub fn state_path() -> PathBuf {
    Path::new(STATE_DIR).join("state.toml")
}

pub fn load(path: &Path) -> Result<Option<State>> {
    if !path.exists() {
        return Ok(None);
    }
    let content =
        fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let state = toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;
    Ok(Some(state))
}

pub fn save(path: &Path, state: &State) -> Result<()> {
    let content = toml::to_string_pretty(state).context("serializing state")?;
    atomic_file::write(path, content.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.toml");
        let mut state = State {
            schema: 1,
            distro: "debian".to_string(),
            ..State::default()
        };
        state.components.insert(
            "umbriel".to_string(),
            InstalledComponent {
                commit: "a909081".to_string(),
                method: "source".to_string(),
                files: vec!["/usr/bin/umbriel".to_string()],
            },
        );
        save(&path, &state).unwrap();
        let loaded = load(&path).unwrap().unwrap();
        assert_eq!(loaded.distro, "debian");
        assert_eq!(loaded.components["umbriel"].commit, "a909081");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_replace_keeps_existing_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.toml");
        let original = State {
            schema: 1,
            distro: "debian".to_string(),
            ..State::default()
        };
        save(&path, &original).unwrap();

        let invalid_target = dir.path().join("directory");
        fs::create_dir(&invalid_target).unwrap();
        assert!(save(&invalid_target, &original).is_err());
        assert_eq!(load(&path).unwrap().unwrap().distro, "debian");
    }
}
