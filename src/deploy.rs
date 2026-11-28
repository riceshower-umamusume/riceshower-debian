#[cfg(test)]
use crate::atomic_file;
use crate::build::{StagedKind, StagingInventory};
use crate::manifest::DistroManifest;
use crate::process::Runner;
use crate::state::{self, InstalledComponent};
use anyhow::{bail, Context, Result};
#[cfg(test)]
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub trait DeployWriter {
    fn create_dir(&self, path: &Path, mode: u32) -> Result<()>;
    fn install_file(&self, source: &Path, target: &Path, mode: u32) -> Result<()>;
    fn symlink(&self, target: &Path, link: &Path) -> Result<()>;
    fn copy(&self, source: &Path, target: &Path) -> Result<()>;
    fn remove_file(&self, path: &Path) -> Result<()>;
    fn write_file(&self, path: &Path, content: &[u8], mode: u32) -> Result<()>;
}

#[cfg(test)]
pub struct DirectWriter;

#[cfg(test)]
impl DeployWriter for DirectWriter {
    fn create_dir(&self, path: &Path, mode: u32) -> Result<()> {
        fs::create_dir_all(path).with_context(|| format!("creating {}", path.display()))?;
        set_mode(path, mode)
    }

    fn install_file(&self, source: &Path, target: &Path, mode: u32) -> Result<()> {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        fs::copy(source, target)
            .with_context(|| format!("copying {} to {}", source.display(), target.display()))?;
        set_mode(target, mode)
    }

    fn symlink(&self, target: &Path, link: &Path) -> Result<()> {
        if let Some(parent) = link.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        let _ = fs::remove_file(link);
        std::os::unix::fs::symlink(target, link)
            .with_context(|| format!("linking {}", link.display()))
    }

    fn copy(&self, source: &Path, target: &Path) -> Result<()> {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        fs::copy(source, target)
            .with_context(|| format!("copying {} to {}", source.display(), target.display()))?;
        Ok(())
    }

    fn remove_file(&self, path: &Path) -> Result<()> {
        if path.exists() {
            fs::remove_file(path).with_context(|| format!("removing {}", path.display()))?;
        }
        Ok(())
    }

    fn write_file(&self, path: &Path, content: &[u8], mode: u32) -> Result<()> {
        atomic_file::write(path, content).with_context(|| format!("writing {}", path.display()))?;
        set_mode(path, mode)
    }
}

pub struct SudoWriter<'a> {
    pub runner: &'a dyn Runner,
}

impl SudoWriter<'_> {
    fn checked_stdin(&self, program: &str, args: &[&str], input: &str) -> Result<()> {
        let output = self.runner.run_with_stdin(program, args, input)?;
        if !output.success {
            let details = if output.stderr.trim().is_empty() {
                output.stdout.trim()
            } else {
                output.stderr.trim()
            };
            bail!("{program} failed: {details}");
        }
        Ok(())
    }

    fn checked(&self, program: &str, args: &[&str]) -> Result<()> {
        let output = self.runner.run(program, args)?;
        if !output.success {
            let details = if output.stderr.trim().is_empty() {
                output.stdout.trim()
            } else {
                output.stderr.trim()
            };
            bail!("{program} failed: {details}");
        }
        Ok(())
    }
}

impl DeployWriter for SudoWriter<'_> {
    fn create_dir(&self, path: &Path, mode: u32) -> Result<()> {
        let path = path.to_str().context("non-UTF-8 deploy path")?;
        let mode = format!("-m{mode:o}");
        self.checked("install", &["-d", &mode, "--", path])
    }

    fn install_file(&self, source: &Path, target: &Path, mode: u32) -> Result<()> {
        let source = source.to_str().context("non-UTF-8 staging path")?;
        let target = target.to_str().context("non-UTF-8 deploy path")?;
        let mode = format!("-m{mode:o}");
        self.checked("install", &["-D", &mode, "--", source, target])
    }

    fn symlink(&self, target: &Path, link: &Path) -> Result<()> {
        let target = target.to_str().context("non-UTF-8 symlink target")?;
        let link = link.to_str().context("non-UTF-8 deploy path")?;
        self.checked("ln", &["-sfn", "--", target, link])
    }

    fn copy(&self, source: &Path, target: &Path) -> Result<()> {
        let source = source.to_str().context("non-UTF-8 source path")?;
        let target = target.to_str().context("non-UTF-8 target path")?;
        self.checked("cp", &["-a", "--", source, target])
    }

    fn remove_file(&self, path: &Path) -> Result<()> {
        let path = path.to_str().context("non-UTF-8 deploy path")?;
        self.checked("rm", &["-f", "--", path])
    }

    fn write_file(&self, path: &Path, content: &[u8], _mode: u32) -> Result<()> {
        let path = path.to_str().context("non-UTF-8 deploy path")?;
        let content = std::str::from_utf8(content).context("non-UTF-8 state content")?;
        self.checked_stdin("tee", &[path], content)
    }
}

#[derive(Debug, Clone)]
pub struct DeployComponent {
    pub id: String,
    pub commit: String,
    pub staging_dir: PathBuf,
    pub inventory: StagingInventory,
}

struct Applied {
    target: PathBuf,
    backup: Option<PathBuf>,
    existed: bool,
    directory: bool,
}

pub fn deploy_components(
    root: &Path,
    components: &[DeployComponent],
    packages: &[String],
    distro: &str,
    state_path: &Path,
    writer: &dyn DeployWriter,
    dry_run: bool,
) -> Result<()> {
    let mut state = state::load(state_path)?.unwrap_or_default();
    state.distro = distro.to_string();
    state.packages = packages.to_vec();
    let timestamp = timestamp()?;
    let backup_root = state_path
        .parent()
        .context("state path has no parent")?
        .join("backups")
        .join(&timestamp);
    let mut applied: Vec<Applied> = Vec::new();

    let result = (|| -> Result<()> {
        for component in components {
            for file in &component.inventory.files {
                let relative = Path::new(&file.path);
                if relative.is_absolute()
                    || relative
                        .components()
                        .any(|part| !matches!(part, std::path::Component::Normal(_)))
                {
                    bail!("unsafe staged path '{}'", file.path);
                }
                let target = root.join(relative);
                if !target.starts_with(root) {
                    bail!("staged path escapes root: {}", target.display());
                }
                match file.kind {
                    StagedKind::Directory => {
                        let existed = target.exists();
                        if dry_run {
                            println!("  dir  {}", target.display());
                        } else {
                            writer.create_dir(&target, file.mode)?;
                        }
                        if !existed {
                            applied.push(Applied {
                                target,
                                backup: None,
                                existed: false,
                                directory: true,
                            });
                        }
                    }
                    StagedKind::File | StagedKind::Symlink => {
                        let existed = target.exists();
                        let backup = if existed {
                            let path = backup_root.join(&component.id).join(relative);
                            if !dry_run {
                                if let Some(parent) = path.parent() {
                                    writer.create_dir(parent, 0o755)?;
                                }
                                writer.copy(&target, &path)?;
                            }
                            Some(path)
                        } else {
                            None
                        };
                        if dry_run {
                            println!(
                                "  {}  {}",
                                if file.kind == StagedKind::File {
                                    "file"
                                } else {
                                    "link"
                                },
                                target.display()
                            );
                        } else {
                            if let Some(parent) = target.parent() {
                                writer.create_dir(parent, 0o755)?;
                            }
                            match file.kind {
                                StagedKind::File => writer.install_file(
                                    &component.staging_dir.join(relative),
                                    &target,
                                    file.mode,
                                )?,
                                StagedKind::Symlink => {
                                    let link =
                                        file.target.as_deref().context("symlink has no target")?;
                                    writer.symlink(Path::new(link), &target)?;
                                }
                                StagedKind::Directory => unreachable!(),
                            }
                        }
                        applied.push(Applied {
                            target,
                            backup,
                            existed,
                            directory: false,
                        });
                    }
                }
            }
            state.components.insert(
                component.id.clone(),
                InstalledComponent {
                    commit: component.commit.clone(),
                    method: "source".to_string(),
                    files: component
                        .inventory
                        .files
                        .iter()
                        .map(|file| root.join(&file.path).display().to_string())
                        .collect(),
                },
            );
        }
        if !dry_run {
            let content = toml::to_string_pretty(&state).context("serializing installer state")?;
            writer.write_file(state_path, content.as_bytes(), 0o644)?;
        }
        Ok(())
    })();

    if result.is_err() {
        for item in applied.iter().rev() {
            if item.directory {
                continue;
            }
            if let Some(backup) = &item.backup {
                let _ = writer.copy(backup, &item.target);
            } else if !item.existed {
                let _ = writer.remove_file(&item.target);
            }
        }
    }
    result
}

pub fn install_packages(
    distro: &DistroManifest,
    packages: &[String],
    runner: &dyn Runner,
) -> Result<()> {
    if packages.is_empty() {
        return Ok(());
    }
    if distro.package_manager != "apt" {
        bail!("unsupported package manager '{}'", distro.package_manager);
    }
    let (program, mut args): (&str, Vec<&str>) =
        ("apt-get", vec!["install", "-y", "--no-install-recommends"]);
    args.extend(packages.iter().map(String::as_str));
    let output = runner.run(program, &args)?;
    if !output.success {
        let details = if output.stderr.trim().is_empty() {
            output.stdout.trim()
        } else {
            output.stderr.trim()
        };
        bail!("package installation failed: {details}");
    }
    Ok(())
}

#[cfg(test)]
fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)
        .with_context(|| format!("reading metadata for {}", path.display()))?
        .permissions();
    permissions.set_mode(mode);
    fs::set_permissions(path, permissions)
        .with_context(|| format!("setting mode on {}", path.display()))
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
    use crate::build::StagedFile;
    use crate::process::CommandOutput;

    fn file(path: &str) -> StagedFile {
        StagedFile {
            path: path.to_string(),
            kind: StagedKind::File,
            mode: 0o644,
            size: 2,
            sha256: String::new(),
            target: None,
        }
    }

    fn component(staging: &Path) -> DeployComponent {
        DeployComponent {
            id: "umbriel".to_string(),
            commit: "a".repeat(40),
            staging_dir: staging.to_path_buf(),
            inventory: StagingInventory {
                component: "umbriel".to_string(),
                commit: "a".repeat(40),
                prefix: "/usr/local".to_string(),
                files: vec![file("usr/local/bin/tool")],
            },
        }
    }

    #[test]
    fn deploy_writes_files_and_state_with_direct_writer() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("root");
        let staging = directory.path().join("staging");
        let state = directory.path().join("state/state.toml");
        fs::create_dir_all(staging.join("usr/local/bin")).unwrap();
        fs::write(staging.join("usr/local/bin/tool"), "ok").unwrap();

        deploy_components(
            &root,
            &[component(&staging)],
            &["pkg".to_string()],
            "debian-sid",
            &state,
            &DirectWriter,
            false,
        )
        .unwrap();

        assert_eq!(
            fs::read_to_string(root.join("usr/local/bin/tool")).unwrap(),
            "ok"
        );
        let saved = state::load(&state).unwrap().unwrap();
        assert_eq!(saved.distro, "debian-sid");
        assert_eq!(saved.packages, vec!["pkg".to_string()]);
        assert_eq!(saved.components["umbriel"].commit, "a".repeat(40));
    }

    #[test]
    fn deploy_dry_run_does_not_touch_root() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("root");
        let staging = directory.path().join("staging");
        let state = directory.path().join("state/state.toml");
        fs::create_dir_all(staging.join("usr/local/bin")).unwrap();
        fs::write(staging.join("usr/local/bin/tool"), "ok").unwrap();

        deploy_components(
            &root,
            &[component(&staging)],
            &[],
            "debian-sid",
            &state,
            &DirectWriter,
            true,
        )
        .unwrap();
        assert!(!root.exists());
        assert!(!state.exists());
    }

    struct FakePackages;

    impl Runner for FakePackages {
        fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput> {
            assert_eq!(program, "apt-get");
            assert_eq!(args[0], "install");
            assert!(args.contains(&"pkg"));
            Ok(CommandOutput {
                success: true,
                code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            })
        }

        fn run_with_stdin(
            &self,
            _program: &str,
            _args: &[&str],
            _input: &str,
        ) -> Result<CommandOutput> {
            bail!("stdin is not used")
        }
    }

    #[test]
    fn install_packages_uses_apt() {
        let distro = DistroManifest {
            id: "debian-sid".to_string(),
            name: "Debian sid".to_string(),
            package_manager: "apt".to_string(),
            versions: Vec::new(),
            package_map: Default::default(),
            private_dependencies: Vec::new(),
            private_build_packages: Vec::new(),
        };
        install_packages(&distro, &["pkg".to_string()], &FakePackages).unwrap();
    }
}
