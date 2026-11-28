use crate::atomic_file;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component as PathComponent, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Deserialize)]
pub struct DotfilesManifest {
    pub schema: u32,
    pub files: Vec<Dotfile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Dotfile {
    pub id: String,
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DotfilesState {
    pub schema: u32,
    pub generated: String,
    #[serde(default)]
    pub files: BTreeMap<String, InstalledDotfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledDotfile {
    pub source: String,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<String>,
}

pub fn load(root: &Path) -> Result<DotfilesManifest> {
    let path = root.join("manifest.toml");
    let content =
        fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let manifest: DotfilesManifest =
        toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;
    if manifest.schema != 1 {
        bail!("unsupported dotfiles manifest schema {}", manifest.schema);
    }
    for file in &manifest.files {
        validate_relative(&file.source)
            .with_context(|| format!("dotfile '{}' source is unsafe", file.id))?;
        validate_relative(&file.target)
            .with_context(|| format!("dotfile '{}' target is unsafe", file.id))?;
    }
    Ok(manifest)
}

pub fn apply(root: &Path, home: &Path, dry_run: bool) -> Result<()> {
    let manifest = load(root)?;
    let state_path = state_path()?;
    let mut state = load_state(&state_path)?;
    let timestamp = timestamp()?;
    let backup_root = state_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("backups")
        .join(&timestamp);
    let mut changed = 0usize;

    for file in &manifest.files {
        let source = root.join("files").join(&file.source);
        let target = home.join(&file.target);
        if !source.is_file() {
            bail!(
                "dotfile '{}' source is missing: {}",
                file.id,
                source.display()
            );
        }
        let target_parent = target
            .parent()
            .with_context(|| format!("dotfile '{}' target has no parent", file.id))?;
        let source_hash = hash_file(&source)?;

        if target.is_file() && hash_file(&target)? == source_hash {
            println!("  {} unchanged -> {}", file.id, target.display());
            continue;
        }
        if dry_run {
            println!("  {} write -> {}", file.id, target.display());
            changed += 1;
            continue;
        }

        fs::create_dir_all(target_parent)
            .with_context(|| format!("creating {}", target_parent.display()))?;
        let backup = if target.exists() {
            let backup_path = backup_root.join(&file.target);
            if let Some(parent) = backup_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("creating {}", parent.display()))?;
            }
            fs::copy(&target, &backup_path)
                .with_context(|| format!("backing up {}", target.display()))?;
            Some(backup_path.display().to_string())
        } else {
            None
        };

        let mode = fs::metadata(&target)
            .ok()
            .map(|metadata| metadata.permissions())
            .or_else(|| {
                fs::metadata(&source)
                    .ok()
                    .map(|metadata| metadata.permissions())
            });
        let content = fs::read(&source).with_context(|| format!("reading {}", source.display()))?;
        atomic_file::write(&target, &content)
            .with_context(|| format!("writing {}", target.display()))?;
        if let Some(mode) = mode {
            fs::set_permissions(&target, mode)
                .with_context(|| format!("preserving permissions on {}", target.display()))?;
        }
        state.files.insert(
            file.id.clone(),
            InstalledDotfile {
                source: file.source.clone(),
                sha256: source_hash,
                backup,
            },
        );
        changed += 1;
        println!("  {} applied -> {}", file.id, target.display());
    }

    if dry_run {
        println!("dry-run: {changed} dotfile(s) would change");
        return Ok(());
    }
    state.schema = 1;
    state.generated = timestamp;
    let content = toml::to_string_pretty(&state).context("serializing dotfiles state")?;
    atomic_file::write(&state_path, content.as_bytes())
        .with_context(|| format!("writing {}", state_path.display()))?;
    println!("  state: {}", state_path.display());
    Ok(())
}

pub fn write_display(home: &Path, content: &str, dry_run: bool) -> Result<()> {
    let target = home.join(".config/umbriel/display.toml");
    if target.is_file() {
        let existing =
            fs::read_to_string(&target).with_context(|| format!("reading {}", target.display()))?;
        if existing == content {
            println!("  display unchanged -> {}", target.display());
            return Ok(());
        }
    }
    if dry_run {
        println!("  display would write -> {}", target.display());
        return Ok(());
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    atomic_file::write(&target, content.as_bytes())
        .with_context(|| format!("writing {}", target.display()))?;
    println!("  display applied -> {}", target.display());
    Ok(())
}

pub fn state_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("XDG_STATE_HOME") {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            return Ok(path.join("riceshower-debian/dotfiles.toml"));
        }
    }
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join(".local/state/riceshower-debian/dotfiles.toml"))
}

fn load_state(path: &Path) -> Result<DotfilesState> {
    if !path.exists() {
        return Ok(DotfilesState::default());
    }
    let content =
        fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))
}

fn hash_file(path: &Path) -> Result<String> {
    let content = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&content);
    Ok(format!("{:x}", hasher.finalize()))
}

fn validate_relative(path: &str) -> Result<()> {
    let path = Path::new(path);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, PathComponent::Normal(_)))
    {
        bail!("path must contain only normal relative components");
    }
    Ok(())
}

fn timestamp() -> Result<String> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before Unix epoch")?
        .as_secs();
    Ok(seconds.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(root: &Path, home: &Path) {
        fs::create_dir_all(root.join("files")).unwrap();
        fs::write(root.join("files/bashrc"), "new\n").unwrap();
        fs::write(
            root.join("manifest.toml"),
            "schema = 1\n[[files]]\nid = \"bashrc\"\nsource = \"bashrc\"\ntarget = \".bashrc\"\n",
        )
        .unwrap();
        fs::create_dir_all(home).unwrap();
    }

    #[test]
    fn apply_backs_up_existing_and_skips_second_run() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("dotfiles");
        let home = directory.path().join("home");
        let state = directory.path().join("state");
        fixture(&root, &home);
        fs::write(home.join(".bashrc"), "old\n").unwrap();

        std::env::set_var("XDG_STATE_HOME", &state);
        apply(&root, &home, false).unwrap();
        assert_eq!(fs::read_to_string(home.join(".bashrc")).unwrap(), "new\n");
        let backup = state
            .join("riceshower-debian/backups")
            .read_dir()
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path()
            .join(".bashrc");
        assert_eq!(fs::read_to_string(backup).unwrap(), "old\n");

        apply(&root, &home, false).unwrap();
        assert_eq!(fs::read_to_string(home.join(".bashrc")).unwrap(), "new\n");
        std::env::remove_var("XDG_STATE_HOME");
    }

    #[test]
    fn rejects_unsafe_target() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("dotfiles");
        let home = directory.path().join("home");
        fs::create_dir_all(root.join("files")).unwrap();
        fs::write(root.join("files/bashrc"), "new\n").unwrap();
        fs::write(
            root.join("manifest.toml"),
            "schema = 1\n[[files]]\nid = \"bad\"\nsource = \"bashrc\"\ntarget = \"../escape\"\n",
        )
        .unwrap();
        fs::create_dir_all(&home).unwrap();
        assert!(load(&root).is_err());
    }

    #[test]
    fn write_display_does_not_touch_home_on_dry_run() {
        let directory = tempfile::tempdir().unwrap();
        let home = directory.path().join("home");
        fs::create_dir_all(&home).unwrap();
        write_display(&home, "[output.DP-1]\n", true).unwrap();
        assert!(!home.join(".config/umbriel/display.toml").exists());
    }
}
