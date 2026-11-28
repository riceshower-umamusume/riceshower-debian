use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct Profile {
    pub schema: u32,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub includes: Vec<String>,
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub multiarch: Vec<String>,
    #[serde(default)]
    pub microcode: bool,
    #[serde(default)]
    pub source_tools: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ResolvedProfiles {
    pub names: Vec<String>,
    pub packages: Vec<String>,
    pub sources: Vec<String>,
    pub multiarch: Vec<String>,
    pub microcode: bool,
    pub source_tools: Vec<String>,
}

pub fn load(root: &Path) -> Result<BTreeMap<String, Profile>> {
    let mut profiles = BTreeMap::new();
    let entries = fs::read_dir(root).with_context(|| format!("reading {}", root.display()))?;
    for entry in entries {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        let content =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let profile: Profile =
            toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;
        if profile.schema != 1 {
            bail!("unsupported profile schema in {}", path.display());
        }
        if profiles.insert(profile.id.clone(), profile).is_some() {
            bail!("duplicate profile id in {}", root.display());
        }
    }
    Ok(profiles)
}

pub fn resolve(root: &Path, ids: &[String]) -> Result<ResolvedProfiles> {
    let profiles = load(root)?;
    let requested: Vec<String> = if ids.is_empty() {
        vec!["all".to_string()]
    } else {
        ids.to_vec()
    };
    let mut names = BTreeSet::new();
    let mut packages = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut multiarch = BTreeSet::new();
    let mut source_tools = BTreeSet::new();
    let mut microcode = false;
    let mut pending = requested;
    let mut seen = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        let profile = profiles
            .get(&id)
            .with_context(|| format!("unknown profile '{id}'"))?;
        names.insert(profile.name.clone());
        packages.extend(profile.packages.iter().cloned());
        sources.extend(profile.sources.iter().cloned());
        multiarch.extend(profile.multiarch.iter().cloned());
        source_tools.extend(profile.source_tools.iter().cloned());
        microcode |= profile.microcode;
        pending.extend(profile.includes.iter().cloned());
    }
    Ok(ResolvedProfiles {
        names: names.into_iter().collect(),
        packages: packages.into_iter().collect(),
        sources: sources.into_iter().collect(),
        multiarch: multiarch.into_iter().collect(),
        microcode,
        source_tools: source_tools.into_iter().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_profiles_resolve_all() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("profiles");
        let resolved = resolve(&root, &[]).unwrap();
        assert!(resolved.packages.contains(&"zsh".to_string()));
        assert!(resolved.packages.contains(&"steam-installer".to_string()));
        assert!(resolved.packages.contains(&"earlyoom".to_string()));
        assert!(resolved.microcode);
        assert!(!resolved.multiarch.is_empty());
        assert_eq!(resolved.source_tools, vec!["starship".to_string()]);
    }
}
