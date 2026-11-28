use crate::atomic_file;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component as PathComponent, Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct EcosystemManifest {
    pub schema: u32,
    pub channel: String,
    pub lockfile: String,
    pub components: Vec<String>,
    pub distros: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ComponentManifest {
    pub id: String,
    pub name: String,
    pub repo: String,
    #[serde(rename = "ref")]
    pub git_ref: String,
    #[serde(default)]
    pub version_source: Option<String>,
    pub build_system: String,
    #[serde(default)]
    pub cxx_standard: Option<u8>,
    #[serde(default)]
    pub build_options: Vec<String>,
    #[serde(default)]
    pub build_tools: Vec<String>,
    #[serde(default)]
    pub build_dependencies: Vec<String>,
    #[serde(default)]
    pub build_requirements: BTreeMap<String, BuildRequirement>,
    #[serde(default)]
    pub build_component_dependencies: Vec<String>,
    #[serde(default)]
    pub component_dependencies: Vec<String>,
    #[serde(default)]
    pub runtime_dependencies: Vec<String>,
    #[serde(default)]
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub layer: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BuildRequirement {
    pub pkg_config: Vec<String>,
    #[serde(default)]
    pub min_version: Option<String>,
    #[serde(default)]
    pub max_version: Option<String>,
    #[serde(default = "default_true")]
    pub required: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct DistroManifest {
    pub id: String,
    pub name: String,
    pub package_manager: String,
    pub versions: Vec<DistroVersion>,
    #[serde(default)]
    pub package_map: BTreeMap<String, String>,
    #[serde(default)]
    pub private_dependencies: Vec<String>,
    #[serde(default)]
    pub private_build_packages: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DistroVersion {
    pub id: String,
    pub tier: u8,
    #[serde(default)]
    pub codename: Option<String>,
    #[serde(default)]
    pub rolling: bool,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub dep_layer: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EcosystemLock {
    pub schema: u32,
    pub generated: String,
    #[serde(default)]
    pub components: BTreeMap<String, LockedComponent>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LockedComponent {
    pub commit: String,
    #[serde(rename = "ref")]
    pub git_ref: String,
}

#[derive(Debug, Clone)]
pub struct Manifest {
    pub root: PathBuf,
    pub ecosystem: EcosystemManifest,
    pub components: BTreeMap<String, ComponentManifest>,
    pub distros: BTreeMap<String, DistroManifest>,
}

pub fn load_private(root: &Path) -> Result<BTreeMap<String, ComponentManifest>> {
    let directory = root.join("private");
    let mut components = BTreeMap::new();
    let mut paths: Vec<PathBuf> = fs::read_dir(&directory)
        .with_context(|| format!("reading {}", directory.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
        .collect();
    paths.sort();
    for path in paths {
        let component: ComponentManifest = read_toml(&path)?;
        if !matches!(component.build_system.as_str(), "meson" | "cmake") {
            bail!(
                "private component '{}' uses unsupported build system '{}'",
                component.id,
                component.build_system
            );
        }
        if components.insert(component.id.clone(), component).is_some() {
            bail!("duplicate private component id in {}", directory.display());
        }
    }
    Ok(components)
}

pub fn load(root: &Path) -> Result<Manifest> {
    let ecosystem_path = resolve_manifest_file(root, "ecosystem.toml")?;
    let ecosystem: EcosystemManifest = read_toml(&ecosystem_path)?;
    if ecosystem.schema != 1 {
        bail!(
            "unsupported manifest schema {} in {}",
            ecosystem.schema,
            ecosystem_path.display()
        );
    }
    validate_id_list("component", &ecosystem.components)?;
    validate_id_list("distro", &ecosystem.distros)?;
    validate_relative_path(&ecosystem.lockfile)
        .context("ecosystem.lockfile must be a relative path inside the manifest directory")?;
    if ecosystem.channel.trim().is_empty() {
        bail!("ecosystem channel must not be empty");
    }

    let components = load_components(root, &ecosystem.components)?;
    validate_component_dependencies(&components)?;
    validate_component_requirements(&components)?;
    let distros = load_distros(root, &ecosystem.distros)?;
    Ok(Manifest {
        root: root.to_path_buf(),
        ecosystem,
        components,
        distros,
    })
}

impl Manifest {
    pub fn lock(&self) -> Result<EcosystemLock> {
        let path = resolve_manifest_file(&self.root, &self.ecosystem.lockfile)?;
        let lock: EcosystemLock = read_toml(&path)?;
        self.validate_lock(&lock)?;
        Ok(lock)
    }

    pub fn save_lock(&self, lock: &EcosystemLock) -> Result<()> {
        self.validate_lock(lock)?;
        let path = resolve_manifest_file(&self.root, &self.ecosystem.lockfile)?;
        let content = toml::to_string_pretty(lock).context("serializing lockfile")?;
        atomic_file::write(&path, content.as_bytes())
            .with_context(|| format!("writing lockfile {}", path.display()))
    }

    fn validate_lock(&self, lock: &EcosystemLock) -> Result<()> {
        if lock.schema != 1 {
            bail!("unsupported lockfile schema {}", lock.schema);
        }
        if lock.generated.trim().is_empty() {
            bail!("lockfile generated timestamp must not be empty");
        }
        for id in &self.ecosystem.components {
            let component = lock
                .components
                .get(id)
                .with_context(|| format!("lockfile is missing component '{id}'"))?;
            if component.commit.len() != 40
                || !component
                    .commit
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit())
            {
                bail!("lockfile component '{id}' has an invalid commit SHA");
            }
            if component.git_ref.trim().is_empty() {
                bail!("lockfile component '{id}' has an empty ref");
            }
            if component.git_ref != self.components[id].git_ref {
                bail!("lockfile ref for component '{id}' does not match its manifest ref");
            }
        }
        if let Some(id) = lock
            .components
            .keys()
            .find(|id| !self.components.contains_key(*id))
        {
            bail!("lockfile references unknown component '{id}'");
        }
        Ok(())
    }
}

fn validate_id_list(kind: &str, ids: &[String]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for id in ids {
        if id.is_empty()
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            bail!("invalid {kind} id '{id}': use lowercase ASCII letters, digits and hyphens");
        }
        if !seen.insert(id) {
            bail!("duplicate {kind} id '{id}' in ecosystem.toml");
        }
    }
    Ok(())
}

fn resolve_manifest_file(root: &Path, relative: &str) -> Result<PathBuf> {
    let root = fs::canonicalize(root).context("resolving manifest root")?;
    let path = fs::canonicalize(root.join(relative))
        .with_context(|| format!("resolving manifest file '{relative}'"))?;
    if !path.starts_with(&root) {
        bail!("manifest file '{relative}' resolves outside the manifest directory");
    }
    Ok(path)
}

fn validate_relative_path(path: &str) -> Result<()> {
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

fn load_components(root: &Path, ids: &[String]) -> Result<BTreeMap<String, ComponentManifest>> {
    let mut components = BTreeMap::new();
    for id in ids {
        let relative = format!("components/{id}.toml");
        let path = resolve_manifest_file(root, &relative)?;
        let component: ComponentManifest = read_toml(&path)?;
        if component.id != *id {
            bail!(
                "component id mismatch in {}: expected {}, found {}",
                path.display(),
                id,
                component.id
            );
        }
        if component.name.trim().is_empty()
            || component.repo.trim().is_empty()
            || component.git_ref.trim().is_empty()
            || component.build_system.trim().is_empty()
        {
            bail!(
                "component '{id}' has an empty required field in {}",
                path.display()
            );
        }
        components.insert(id.clone(), component);
    }
    Ok(components)
}

fn load_distros(root: &Path, ids: &[String]) -> Result<BTreeMap<String, DistroManifest>> {
    let mut distros = BTreeMap::new();
    for id in ids {
        let relative = format!("distros/{id}.toml");
        let path = resolve_manifest_file(root, &relative)?;
        let distro: DistroManifest = read_toml(&path)?;
        if distro.id != *id {
            bail!(
                "distro id mismatch in {}: expected {}, found {}",
                path.display(),
                id,
                distro.id
            );
        }
        if distro.name.trim().is_empty()
            || distro.package_manager.trim().is_empty()
            || distro.versions.is_empty()
        {
            bail!(
                "distro '{id}' has an empty required field in {}",
                path.display()
            );
        }
        for (key, package) in &distro.package_map {
            validate_id_list("package map key", std::slice::from_ref(key))?;
            if package.trim().is_empty() {
                bail!("distro '{id}' has an empty package mapping for '{key}'");
            }
        }
        validate_id_list("private dependency", &distro.private_dependencies)?;
        if let Some(key) = distro
            .private_dependencies
            .iter()
            .find(|key| distro.package_map.contains_key(*key))
        {
            bail!(
                "distro '{id}' maps dependency '{key}' both as a package and a private dependency"
            );
        }
        let mut versions = BTreeSet::new();
        for version in &distro.versions {
            if version.id.trim().is_empty() || !(1..=3).contains(&version.tier) {
                bail!(
                    "distro '{id}' has an invalid version entry in {}",
                    path.display()
                );
            }
            if !versions.insert(&version.id) {
                bail!("distro '{id}' has duplicate version '{}'", version.id);
            }
        }
        distros.insert(id.clone(), distro);
    }
    Ok(distros)
}

fn validate_component_dependencies(components: &BTreeMap<String, ComponentManifest>) -> Result<()> {
    for component in components.values() {
        validate_id_list("build tool", &component.build_tools)?;
        validate_id_list("build dependency", &component.build_dependencies)?;
        validate_id_list(
            "build component dependency",
            &component.build_component_dependencies,
        )?;
        validate_id_list("component dependency", &component.component_dependencies)?;
        validate_id_list("runtime dependency", &component.runtime_dependencies)?;
        if !component.build_tools.contains(&component.build_system) {
            bail!(
                "component '{}' must declare its build system '{}' as a build tool",
                component.id,
                component.build_system
            );
        }
        for dependency in &component.build_component_dependencies {
            if dependency == &component.id {
                bail!("component '{}' cannot build-depend on itself", component.id);
            }
            if !components.contains_key(dependency) {
                bail!(
                    "component '{}' references unknown build component dependency '{dependency}'",
                    component.id
                );
            }
        }
        for dependency in &component.component_dependencies {
            if dependency == &component.id {
                bail!("component '{}' cannot depend on itself", component.id);
            }
            if !components.contains_key(dependency) {
                bail!(
                    "component '{}' references unknown component dependency '{dependency}'",
                    component.id
                );
            }
        }
    }
    Ok(())
}

fn validate_component_requirements(components: &BTreeMap<String, ComponentManifest>) -> Result<()> {
    for component in components.values() {
        if let Some(standard) = component.cxx_standard {
            if !(17..=26).contains(&standard) {
                bail!(
                    "component '{}' has unsupported cxx_standard {standard}",
                    component.id
                );
            }
        }
        for (key, requirement) in &component.build_requirements {
            if !component.build_dependencies.contains(key) {
                bail!(
                    "component '{}' has build requirement '{key}' that is not a build dependency",
                    component.id
                );
            }
            for module in &requirement.pkg_config {
                validate_pkg_config_module(module)?;
            }
            for version in [&requirement.min_version, &requirement.max_version]
                .into_iter()
                .flatten()
            {
                crate::version::parse(version).with_context(|| {
                    format!(
                        "component '{}' requirement '{key}' has invalid version constraint '{version}'",
                        component.id
                    )
                })?;
            }
            if let (Some(min), Some(max)) = (&requirement.min_version, &requirement.max_version) {
                if crate::version::compare(min, max)? != Ordering::Less {
                    bail!(
                        "component '{}' requirement '{key}' must have min_version less than max_version",
                        component.id
                    );
                }
            }
        }
        for key in &component.build_dependencies {
            if !component.build_requirements.contains_key(key) {
                bail!(
                    "component '{}' is missing a build requirement for '{key}'",
                    component.id
                );
            }
        }
    }
    Ok(())
}

fn validate_pkg_config_module(module: &str) -> Result<()> {
    if module.is_empty()
        || module.bytes().any(|byte| {
            !(byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.' | b'_'))
        })
    {
        bail!("invalid pkg-config module name '{module}'");
    }
    Ok(())
}

fn read_toml<T>(path: &Path) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let content =
        fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_manifest_loads() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("manifest");
        let manifest = load(&root).unwrap();
        assert_eq!(manifest.components.len(), 5);
        assert_eq!(manifest.distros.len(), 3);
        assert_eq!(manifest.ecosystem.channel, "main");
        assert!(manifest.components.contains_key("umbriel"));
        assert_eq!(manifest.distros["debian-stable"].versions[0].tier, 1);
    }

    #[test]
    fn private_manifest_loads_in_dependency_order() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("manifest");
        let private = load_private(&root).unwrap();
        assert_eq!(private.len(), 11);
        assert_eq!(private["wlroots"].build_system, "meson");
        let ids: Vec<String> = private.keys().cloned().collect();
        let order = crate::build::build_order_components(&private, &ids).unwrap();
        let position = |id: &str| order.iter().position(|item| item == id).unwrap();
        assert!(position("wayland") < position("wlroots"));
        assert!(position("libinput") < position("wlroots"));
        assert!(position("sdbus-cpp") < position("wireplumber"));
    }

    #[test]
    fn repository_manifest_declares_complete_build_requirements() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("manifest");
        let manifest = load(&root).unwrap();

        for component in manifest.components.values() {
            for key in &component.build_dependencies {
                assert!(
                    component.build_requirements.contains_key(key),
                    "component '{}' is missing requirement '{key}'",
                    component.id
                );
            }
        }
        assert_eq!(
            manifest.components["umbriel"].build_requirements["wlroots"]
                .min_version
                .as_deref(),
            Some("0.20.1")
        );
        assert_eq!(
            manifest.components["umbriel"].build_requirements["wlroots"]
                .max_version
                .as_deref(),
            Some("0.21.0")
        );
        assert_eq!(manifest.components["luciole"].cxx_standard, Some(23));
        assert_eq!(
            manifest.components["noctalia-greeter"].cxx_standard,
            Some(20)
        );
    }

    #[test]
    fn rejects_incomplete_or_invalid_build_requirements() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("manifest");
        let manifest = load(&root).unwrap();

        let mut component = manifest.components["umbriel"].clone();
        component.build_requirements.remove("wlroots");
        assert!(validate_component_requirements(&BTreeMap::from([(
            component.id.clone(),
            component
        )]))
        .is_err());

        let mut component = manifest.components["umbriel"].clone();
        component
            .build_requirements
            .get_mut("wlroots")
            .unwrap()
            .min_version = Some("0.21.0".to_string());
        assert!(validate_component_requirements(&BTreeMap::from([(
            component.id.clone(),
            component
        )]))
        .is_err());

        let mut component = manifest.components["umbriel"].clone();
        component.cxx_standard = Some(99);
        assert!(validate_component_requirements(&BTreeMap::from([(
            component.id.clone(),
            component
        )]))
        .is_err());
    }

    #[test]
    fn lockfile_loads_and_covers_every_component() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("manifest");
        let manifest = load(&root).unwrap();
        let lock = manifest.lock().unwrap();
        assert_eq!(lock.components.len(), 5);
        assert_eq!(lock.components["umbriel"].git_ref, "main");
    }

    fn copy_manifest_tree(source: &Path, destination: &Path) {
        fs::create_dir_all(destination).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_manifest_tree(&source_path, &destination_path);
            } else {
                fs::copy(source_path, destination_path).unwrap();
            }
        }
    }

    #[test]
    fn saves_valid_lockfile_atomically() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("manifest");
        let dir = tempfile::tempdir().unwrap();
        copy_manifest_tree(&source, dir.path());

        let manifest = load(dir.path()).unwrap();
        let mut lock = manifest.lock().unwrap();
        lock.generated = "2026-11-25T14:00:00Z".to_string();
        lock.components.get_mut("umbriel").unwrap().commit = "b".repeat(40);

        manifest.save_lock(&lock).unwrap();

        let saved = manifest.lock().unwrap();
        assert_eq!(saved.generated, lock.generated);
        assert_eq!(saved.components["umbriel"].commit, "b".repeat(40));
        assert_eq!(fs::read_dir(dir.path().join("lock")).unwrap().count(), 1);
    }

    #[test]
    fn refuses_invalid_lockfile_without_replacing_existing_file() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("manifest");
        let dir = tempfile::tempdir().unwrap();
        copy_manifest_tree(&source, dir.path());

        let manifest = load(dir.path()).unwrap();
        let original = manifest.lock().unwrap();
        let mut invalid = original.clone();
        invalid.components.get_mut("umbriel").unwrap().commit = "invalid".to_string();

        assert!(manifest.save_lock(&invalid).is_err());
        assert_eq!(
            manifest.lock().unwrap().components["umbriel"].commit,
            original.components["umbriel"].commit
        );
    }

    #[test]
    fn rejects_unsafe_and_duplicate_ids() {
        assert!(validate_id_list("component", &["../escape".to_string()]).is_err());
        assert!(
            validate_id_list("component", &["umbriel".to_string(), "umbriel".to_string()]).is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_lockfile_symlinks_that_escape_manifest_root() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("manifest");
        let lock_dir = root.join("lock");
        fs::create_dir_all(&lock_dir).unwrap();
        let outside = dir.path().join("outside.toml");
        fs::write(&outside, "schema = 1").unwrap();
        symlink(&outside, lock_dir.join("ecosystem.lock.toml")).unwrap();

        assert!(resolve_manifest_file(&root, "lock/ecosystem.lock.toml").is_err());
    }

    #[test]
    fn rejects_paths_that_escape_manifest_root() {
        assert!(validate_relative_path("../state.toml").is_err());
        assert!(validate_relative_path("/etc/passwd").is_err());
        assert!(validate_relative_path("lock/ecosystem.lock.toml").is_ok());
    }
}
