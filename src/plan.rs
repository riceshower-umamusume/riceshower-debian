use crate::manifest::{DistroManifest, Manifest};
use anyhow::{bail, Context, Result};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedInstallPlan {
    pub components: Vec<String>,
    pub packages: Vec<String>,
    pub private_dependencies: Vec<String>,
}

pub fn resolve_install_plan(
    manifest: &Manifest,
    distro: &DistroManifest,
    requested: &[String],
    no_greeter: bool,
) -> Result<ResolvedInstallPlan> {
    let initial = if requested.is_empty() {
        manifest.ecosystem.components.clone()
    } else {
        requested.to_vec()
    };

    let mut pending = Vec::with_capacity(initial.len());
    for id in initial {
        if !manifest.components.contains_key(&id) {
            bail!("unknown component: {id}");
        }
        if !(no_greeter && id == "noctalia-greeter") {
            pending.push(id);
        }
    }

    // A component may have a runtime relationship back to another component.
    // Track visited IDs so the dependency closure remains finite.
    let mut selected = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if no_greeter && id == "noctalia-greeter" {
            continue;
        }
        if !selected.insert(id.clone()) {
            continue;
        }
        let component = manifest
            .components
            .get(&id)
            .with_context(|| format!("component '{id}' disappeared from the manifest"))?;
        pending.extend(component.component_dependencies.iter().cloned());
    }

    // Keep manifest order stable; dependencies do not imply build ordering.
    let components: Vec<String> = manifest
        .ecosystem
        .components
        .iter()
        .filter(|id| selected.contains(*id))
        .cloned()
        .collect();

    let mut package_keys = BTreeSet::new();
    for id in &components {
        let component = &manifest.components[id];
        package_keys.extend(component.build_tools.iter().cloned());
        package_keys.extend(component.build_dependencies.iter().cloned());
        package_keys.extend(component.runtime_dependencies.iter().cloned());
    }

    let mut packages = BTreeSet::new();
    let mut private_dependencies = BTreeSet::new();
    for key in package_keys {
        if distro.private_dependencies.contains(&key) {
            private_dependencies.insert(key);
            continue;
        }
        let package = distro.package_map.get(&key).with_context(|| {
            format!(
                "distro '{}' has no package mapping for dependency '{key}'",
                distro.id
            )
        })?;
        if package.trim().is_empty() {
            bail!(
                "distro '{}' has an empty package mapping for dependency '{key}'",
                distro.id
            );
        }
        packages.insert(package.clone());
    }

    Ok(ResolvedInstallPlan {
        components,
        packages: packages.into_iter().collect(),
        private_dependencies: private_dependencies.into_iter().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn repository_manifest() -> Manifest {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("manifest");
        crate::manifest::load(&root).unwrap()
    }

    #[test]
    fn selecting_umbriel_includes_its_component_dependency_once() {
        let manifest = repository_manifest();
        let distro = &manifest.distros["debian-sid"];
        let plan =
            resolve_install_plan(&manifest, distro, &["umbriel".to_string()], false).unwrap();

        assert_eq!(
            plan.components,
            vec![
                "umbriel".to_string(),
                "xdg-desktop-portal-umbriel".to_string()
            ]
        );
        assert!(plan.packages.contains(&"meson".to_string()));
        assert!(plan.packages.contains(&"libwayland-bin".to_string()));
        assert!(plan.packages.contains(&"libwlroots-0.20-dev".to_string()));
        assert!(plan.packages.contains(&"xwayland".to_string()));
        assert!(plan.packages.contains(&"xdg-desktop-portal".to_string()));
    }

    #[test]
    fn stable_private_stack_dependencies_are_not_misreported_as_apt_packages() {
        let manifest = repository_manifest();
        let distro = &manifest.distros["debian-stable"];
        let plan = resolve_install_plan(&manifest, distro, &[], false).unwrap();

        for dependency in [
            "wlroots",
            "wayland-client",
            "wayland-server",
            "wayland-cursor",
            "wayland-protocols",
            "libinput",
            "xkbcommon",
            "pixman",
            "libdrm",
            "libdisplay-info",
            "libei",
            "sdbus-cpp",
            "wireplumber-dev",
            "wireplumber",
        ] {
            assert!(
                plan.private_dependencies.contains(&dependency.to_string()),
                "missing private dependency {dependency}"
            );
        }
        for package in [
            "libwlroots-0.20-dev",
            "libwayland-dev",
            "wayland-protocols",
            "libinput-dev",
            "libxkbcommon-dev",
            "libpixman-1-dev",
            "libdrm-dev",
            "libdisplay-info-dev",
            "libei-dev",
            "libsdbus-c++-dev",
            "libwireplumber-0.5-dev",
            "wireplumber",
        ] {
            assert!(
                !plan.packages.contains(&package.to_string()),
                "private dependency leaked into apt package plan: {package}"
            );
        }
    }

    #[test]
    fn no_greeter_excludes_component_and_its_runtime_package() {
        let manifest = repository_manifest();
        let distro = &manifest.distros["debian-sid"];
        let plan = resolve_install_plan(&manifest, distro, &[], true).unwrap();

        assert!(!plan.components.iter().any(|id| id == "noctalia-greeter"));
        assert!(!plan.packages.iter().any(|package| package == "greetd"));
    }

    #[test]
    fn every_imperative_distro_resolves_all_declared_dependencies() {
        let manifest = repository_manifest();
        for distro in manifest.distros.values() {
            if distro.family == "nixos" {
                continue;
            }
            resolve_install_plan(&manifest, distro, &[], false)
                .unwrap_or_else(|error| panic!("{}: {error:#}", distro.id));
        }
    }

    #[test]
    fn missing_package_mapping_is_an_error_not_a_silent_omission() {
        let manifest = repository_manifest();
        let mut distro = manifest.distros["debian-sid"].clone();
        distro.package_map.remove("xwayland");

        let error =
            resolve_install_plan(&manifest, &distro, &["umbriel".to_string()], false).unwrap_err();
        assert!(error.to_string().contains("xwayland"));
    }

    #[test]
    fn unknown_component_is_rejected() {
        let manifest = repository_manifest();
        let distro = &manifest.distros["debian-sid"];
        assert!(resolve_install_plan(&manifest, distro, &["unknown".to_string()], false).is_err());
    }

    #[test]
    fn ui_consumers_declare_direct_ui_libraries() {
        let manifest = repository_manifest();
        for id in ["xdg-desktop-portal-umbriel", "luciole"] {
            let dependencies = &manifest.components[id].build_dependencies;
            for key in [
                "cairo",
                "pango",
                "freetype",
                "fontconfig",
                "harfbuzz",
                "epoxy",
                "xkbcommon",
            ] {
                assert!(
                    dependencies
                        .iter()
                        .any(|dependency| dependency.as_str() == key),
                    "component '{id}' is missing direct UI dependency '{key}'"
                );
            }
        }
    }
}
