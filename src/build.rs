use crate::atomic_file;
use crate::manifest::{ComponentManifest, Manifest};
use crate::process::Runner;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Component as PathComponent, Path, PathBuf};
use std::time::Duration;

pub const SYSTEM_PREFIX: &str = "/usr/local";
const BUILD_TIMEOUT: Duration = Duration::from_secs(1800);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StagedKind {
    File,
    Symlink,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagedFile {
    pub path: String,
    pub kind: StagedKind,
    pub mode: u32,
    pub size: u64,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagingInventory {
    pub component: String,
    pub commit: String,
    pub prefix: String,
    pub files: Vec<StagedFile>,
}

#[derive(Debug, Clone)]
pub struct BuildOutcome {
    pub component: String,
    pub commit: String,
    pub source_dir: PathBuf,
    pub build_dir: PathBuf,
    pub staging_dir: PathBuf,
    pub inventory: StagingInventory,
    pub inventory_path: PathBuf,
}

pub fn build_component(
    component: &ComponentManifest,
    commit: &str,
    source_dir: &Path,
    cache_root: &Path,
    runner: &dyn Runner,
) -> Result<BuildOutcome> {
    if component.build_system != "meson" {
        bail!(
            "component '{}' uses unsupported build system '{}'",
            component.id,
            component.build_system
        );
    }
    validate_component_id(&component.id)?;
    validate_commit(commit)?;
    validate_build_options(&component.build_options)?;
    validate_artifacts(&component.artifacts)?;
    if !source_dir.is_dir() {
        bail!("source directory {} does not exist", source_dir.display());
    }

    let build_dir = cache_root.join("build").join(&component.id).join(commit);
    let staging_dir = cache_root.join("staging").join(&component.id).join(commit);
    reset_dir(&build_dir)?;
    reset_dir(&staging_dir)?;

    let setup = meson_setup_args(
        &build_dir,
        source_dir,
        &component.build_options,
        SYSTEM_PREFIX,
    )?;
    run_meson(runner, &setup, "meson setup")?;

    let compile = vec![
        "compile".to_string(),
        "-C".to_string(),
        path_string(&build_dir)?,
    ];
    run_meson(runner, &compile, "meson compile")?;

    let tests = vec![
        "test".to_string(),
        "-C".to_string(),
        path_string(&build_dir)?,
        "--print-errorlogs".to_string(),
    ];
    run_meson(runner, &tests, "meson test")?;

    let install = vec![
        "install".to_string(),
        "-C".to_string(),
        path_string(&build_dir)?,
        "--destdir".to_string(),
        path_string(&staging_dir)?,
    ];
    run_meson(runner, &install, "meson install")?;

    validate_declared_artifacts(component, &staging_dir, SYSTEM_PREFIX)?;
    let inventory = inventory_for(&component.id, commit, &staging_dir, SYSTEM_PREFIX)?;
    let inventory_path = cache_root
        .join("inventory")
        .join(&component.id)
        .join(format!("{commit}.toml"));
    write_inventory(&inventory_path, &inventory)?;

    Ok(BuildOutcome {
        component: component.id.clone(),
        commit: commit.to_string(),
        source_dir: source_dir.to_path_buf(),
        build_dir,
        staging_dir,
        inventory,
        inventory_path,
    })
}

pub fn build_order(manifest: &Manifest, selected: &[String]) -> Result<Vec<String>> {
    let mut states: BTreeMap<String, u8> = BTreeMap::new();
    let mut order = Vec::new();
    for id in selected {
        visit_build_component(manifest, selected, id, &mut states, &mut order)?;
    }
    Ok(order)
}

fn visit_build_component(
    manifest: &Manifest,
    selected: &[String],
    id: &str,
    states: &mut BTreeMap<String, u8>,
    order: &mut Vec<String>,
) -> Result<()> {
    match states.get(id).copied() {
        Some(2) => return Ok(()),
        Some(1) => bail!("cycle in build component dependencies at '{id}'"),
        _ => {}
    }
    let component = manifest
        .components
        .get(id)
        .with_context(|| format!("component '{id}' disappeared from the manifest"))?;
    states.insert(id.to_string(), 1);
    for dependency in &component.build_component_dependencies {
        if !selected.iter().any(|candidate| candidate == dependency) {
            bail!("build component dependency '{dependency}' of '{id}' is not selected");
        }
        visit_build_component(manifest, selected, dependency, states, order)?;
    }
    states.insert(id.to_string(), 2);
    order.push(id.to_string());
    Ok(())
}

pub fn meson_setup_args(
    build_dir: &Path,
    source_dir: &Path,
    options: &[String],
    prefix: &str,
) -> Result<Vec<String>> {
    validate_prefix(prefix)?;
    validate_build_options(options)?;
    let mut args = vec![
        "setup".to_string(),
        path_string(build_dir)?,
        path_string(source_dir)?,
        format!("--prefix={prefix}"),
        "--buildtype=release".to_string(),
    ];
    args.extend(options.iter().cloned());
    Ok(args)
}

pub fn validate_build_options(options: &[String]) -> Result<()> {
    for option in options {
        if !option.starts_with("-D") || option.len() == 2 || !option.contains('=') {
            bail!("invalid Meson option '{option}'");
        }
        if option
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte == 0)
        {
            bail!("Meson option '{option}' contains whitespace");
        }
        let (key, _value) = option[2..]
            .split_once('=')
            .with_context(|| format!("invalid Meson option '{option}'"))?;
        if key.is_empty()
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            bail!("invalid Meson option key in '{option}'");
        }
    }
    Ok(())
}

fn validate_artifacts(artifacts: &[String]) -> Result<()> {
    for artifact in artifacts {
        validate_relative_path(Path::new(artifact))
            .with_context(|| format!("invalid declared artifact '{artifact}'"))?;
    }
    Ok(())
}

fn validate_declared_artifacts(
    component: &ComponentManifest,
    staging_dir: &Path,
    prefix: &str,
) -> Result<()> {
    let prefix_dir = staging_dir.join(prefix.trim_start_matches('/'));
    for artifact in &component.artifacts {
        validate_relative_path(Path::new(artifact))?;
        let path = prefix_dir.join(artifact);
        if fs::symlink_metadata(&path).is_err() {
            bail!(
                "component '{}' did not install declared artifact '{}'",
                component.id,
                artifact
            );
        }
    }
    Ok(())
}

pub fn inventory_for(
    component: &str,
    commit: &str,
    staging_dir: &Path,
    prefix: &str,
) -> Result<StagingInventory> {
    validate_component_id(component)?;
    validate_commit(commit)?;
    validate_prefix(prefix)?;
    let root = fs::canonicalize(staging_dir)
        .with_context(|| format!("resolving staging root {}", staging_dir.display()))?;
    let mut files = Vec::new();
    walk_inventory(&root, &root, &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(StagingInventory {
        component: component.to_string(),
        commit: commit.to_string(),
        prefix: prefix.to_string(),
        files,
    })
}

fn walk_inventory(root: &Path, current: &Path, files: &mut Vec<StagedFile>) -> Result<()> {
    for entry in fs::read_dir(current).with_context(|| format!("reading {}", current.display()))? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("reading metadata for {}", path.display()))?;
        let relative = path
            .strip_prefix(root)
            .with_context(|| format!("{} is outside {}", path.display(), root.display()))?;
        let relative = validate_relative_path(relative)?;

        if metadata.file_type().is_symlink() {
            let target = fs::read_link(&path)
                .with_context(|| format!("reading symlink {}", path.display()))?;
            if target.is_absolute() {
                bail!("staged symlink {} has an absolute target", path.display());
            }
            let resolved = fs::canonicalize(&path)
                .with_context(|| format!("resolving symlink {}", path.display()))?;
            if !resolved.starts_with(root) {
                bail!(
                    "staged symlink {} points outside the staging root",
                    path.display()
                );
            }
            let target = target.to_string_lossy().into_owned();
            files.push(StagedFile {
                path: relative,
                kind: StagedKind::Symlink,
                mode: file_mode(&metadata),
                size: 0,
                sha256: hash_bytes(target.as_bytes()),
                target: Some(target),
            });
        } else if metadata.is_dir() {
            files.push(StagedFile {
                path: relative,
                kind: StagedKind::Directory,
                mode: file_mode(&metadata),
                size: 0,
                sha256: String::new(),
                target: None,
            });
            walk_inventory(root, &path, files)?;
        } else if metadata.is_file() {
            files.push(StagedFile {
                path: relative,
                kind: StagedKind::File,
                mode: file_mode(&metadata),
                size: metadata.len(),
                sha256: hash_file(&path)?,
                target: None,
            });
        } else {
            bail!("unsupported staged file type at {}", path.display());
        }
    }
    Ok(())
}

fn write_inventory(path: &Path, inventory: &StagingInventory) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let content = toml::to_string_pretty(inventory).context("serializing staging inventory")?;
    atomic_file::write(path, content.as_bytes())
        .with_context(|| format!("writing inventory {}", path.display()))
}

fn run_meson(runner: &dyn Runner, args: &[String], action: &str) -> Result<()> {
    let arguments: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = runner.run_streaming("meson", &arguments, BUILD_TIMEOUT)?;
    if !output.success {
        let details = if output.stderr.trim().is_empty() {
            output.stdout.trim()
        } else {
            output.stderr.trim()
        };
        if details.is_empty() {
            bail!("{action} failed");
        }
        bail!("{action} failed: {details}");
    }
    Ok(())
}

fn reset_dir(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path).with_context(|| format!("removing {}", path.display()))?;
    }
    fs::create_dir_all(path).with_context(|| format!("creating {}", path.display()))
}

fn path_string(path: &Path) -> Result<String> {
    path.to_str()
        .map(str::to_string)
        .with_context(|| format!("path {} is not valid UTF-8", path.display()))
}

fn validate_prefix(prefix: &str) -> Result<()> {
    if !prefix.starts_with('/') || prefix.contains("..") || prefix.contains(char::is_whitespace) {
        bail!("invalid installation prefix '{prefix}'");
    }
    validate_relative_path(Path::new(prefix.trim_start_matches('/')))
        .context("invalid installation prefix")?;
    Ok(())
}

fn validate_relative_path(path: &Path) -> Result<String> {
    if path.as_os_str().is_empty() {
        bail!("path must not be empty");
    }
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            PathComponent::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            _ => bail!("path {} is not a normal relative path", path.display()),
        }
    }
    if parts.is_empty() {
        bail!("path must not be empty");
    }
    Ok(parts.join("/"))
}

fn validate_commit(commit: &str) -> Result<()> {
    if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("invalid commit SHA '{commit}'");
    }
    Ok(())
}

fn validate_component_id(component: &str) -> Result<()> {
    if component.is_empty()
        || !component
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        bail!("invalid component id '{component}'");
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("reading {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_digest(hasher))
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_digest(hasher)
}

fn hex_digest(hasher: Sha256) -> String {
    use std::fmt::Write;
    let digest = hasher.finalize();
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn file_mode(metadata: &fs::Metadata) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o7777
    }
    #[cfg(not(unix))]
    {
        if metadata.is_dir() {
            0o755
        } else {
            0o644
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{CommandOutput, Runner};

    fn output(success: bool, stdout: &str, stderr: &str) -> CommandOutput {
        CommandOutput {
            success,
            code: Some(if success { 0 } else { 1 }),
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
        }
    }

    struct FakeMeson {
        artifacts: Vec<String>,
    }

    impl Runner for FakeMeson {
        fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput> {
            if program != "meson" {
                bail!("unexpected command {program}");
            }
            if args.first() == Some(&"install") {
                let destination = args
                    .iter()
                    .position(|argument| *argument == "--destdir")
                    .and_then(|index| args.get(index + 1))
                    .context("missing --destdir")?;
                let destination = Path::new(destination);
                for artifact in &self.artifacts {
                    let path = destination.join("usr/local").join(artifact);
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(&path, format!("artifact {artifact}"))?;
                }
            }
            Ok(output(true, "", ""))
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

    struct FailingMeson;

    impl Runner for FailingMeson {
        fn run(&self, _program: &str, _args: &[&str]) -> Result<CommandOutput> {
            Ok(output(false, "", "simulated build failure"))
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

    fn repository() -> crate::manifest::Manifest {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("manifest");
        crate::manifest::load(&root).unwrap()
    }

    #[test]
    fn constructs_meson_argv_without_shell() {
        let args = meson_setup_args(
            Path::new("/tmp/build"),
            Path::new("/tmp/source"),
            &["-Dtests=disabled".to_string()],
            "/usr/local",
        )
        .unwrap();
        assert_eq!(args[0], "setup");
        assert_eq!(args[1], "/tmp/build");
        assert_eq!(args[2], "/tmp/source");
        assert!(args.contains(&"--prefix=/usr/local".to_string()));
        assert!(args.contains(&"-Dtests=disabled".to_string()));
    }

    #[test]
    fn build_order_ignores_runtime_component_cycle() {
        let manifest = repository();
        let selected = manifest.ecosystem.components.clone();
        let order = build_order(&manifest, &selected).unwrap();
        assert_eq!(order, selected);
    }

    #[test]
    fn rejects_invalid_meson_options() {
        assert!(validate_build_options(&["--tests=disabled".to_string()]).is_err());
        assert!(validate_build_options(&["-Dtests".to_string()]).is_err());
        assert!(validate_build_options(&["-Dtests=disabled".to_string()]).is_ok());
        assert!(validate_build_options(&["-Dfoo=bar baz".to_string()]).is_err());
    }

    #[test]
    fn rejects_unsafe_artifacts() {
        assert!(validate_artifacts(&["../escape".to_string()]).is_err());
        assert!(validate_artifacts(&["/absolute".to_string()]).is_err());
        assert!(validate_artifacts(&["bin/tool".to_string()]).is_ok());
    }

    #[test]
    fn build_and_inventory_with_fake_meson() {
        let manifest = repository();
        let component = manifest.components["umbriel"].clone();
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("marker"), "source").unwrap();
        let cache = directory.path().join("cache");

        let outcome = build_component(
            &component,
            &"a".repeat(40),
            &source,
            &cache,
            &FakeMeson {
                artifacts: component.artifacts.clone(),
            },
        )
        .unwrap();

        assert!(outcome.staging_dir.join("usr/local/bin/umbriel").is_file());
        assert!(outcome
            .inventory
            .files
            .iter()
            .any(|file| file.path == "usr/local/bin/umbriel"));
        assert!(outcome.inventory_path.is_file());
        assert_eq!(fs::read_to_string(source.join("marker")).unwrap(), "source");
        let saved: StagingInventory =
            toml::from_str(&fs::read_to_string(&outcome.inventory_path).unwrap()).unwrap();
        assert_eq!(saved, outcome.inventory);
    }

    #[test]
    fn failed_build_keeps_source_and_removes_staging() {
        let manifest = repository();
        let component = manifest.components["umbriel"].clone();
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("marker"), "source").unwrap();
        let cache = directory.path().join("cache");

        assert!(
            build_component(&component, &"b".repeat(40), &source, &cache, &FailingMeson,).is_err()
        );
        assert_eq!(fs::read_to_string(source.join("marker")).unwrap(), "source");
        assert!(!cache
            .join("staging/umbriel")
            .join("b".repeat(40))
            .join("usr/local/bin/umbriel")
            .exists());
    }

    #[test]
    fn inventory_hashes_files_and_symlinks() {
        let directory = tempfile::tempdir().unwrap();
        let staging = directory.path().join("staging");
        fs::create_dir_all(staging.join("usr/local/bin")).unwrap();
        fs::write(staging.join("usr/local/bin/tool"), "hello").unwrap();
        let inventory = inventory_for("umbriel", &"c".repeat(40), &staging, "/usr/local").unwrap();
        let tool = inventory
            .files
            .iter()
            .find(|file| file.path == "usr/local/bin/tool")
            .unwrap();
        assert_eq!(tool.kind, StagedKind::File);
        assert_eq!(
            tool.sha256,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink("tool", staging.join("usr/local/bin/link")).unwrap();
            let inventory =
                inventory_for("umbriel", &"c".repeat(40), &staging, "/usr/local").unwrap();
            let link = inventory
                .files
                .iter()
                .find(|file| file.path == "usr/local/bin/link")
                .unwrap();
            assert_eq!(link.kind, StagedKind::Symlink);
            assert_eq!(link.target.as_deref(), Some("tool"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_that_escapes_staging() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let staging = directory.path().join("staging");
        fs::create_dir_all(staging.join("usr/local/bin")).unwrap();
        fs::write(directory.path().join("outside"), "outside").unwrap();
        symlink("../../../outside", staging.join("usr/local/bin/escape")).unwrap();

        assert!(inventory_for("umbriel", &"d".repeat(40), &staging, "/usr/local",).is_err());
    }
}
