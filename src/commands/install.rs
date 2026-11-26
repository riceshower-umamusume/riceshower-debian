use super::version::require_supported_version;
use crate::manifest::{DistroManifest, Manifest};
use crate::plan::resolve_install_plan;
use crate::preflight;
use crate::process::SystemRunner;
use crate::source::resolve_commit;
use anyhow::{bail, Result};
use std::collections::BTreeMap;

pub(super) struct InstallOptions<'a> {
    pub requested: &'a [String],
    pub no_greeter: bool,
    pub latest: bool,
}

pub(super) fn install_plan(
    manifest: &Manifest,
    distro: &DistroManifest,
    os: &BTreeMap<String, String>,
    dry_run: bool,
    options: InstallOptions<'_>,
) -> Result<()> {
    if distro.family == "nixos" {
        bail!("NixOS is managed declaratively; import nixosModules.default from the flake instead of running install");
    }
    let version = require_supported_version(distro, os)?;
    let resolved = resolve_install_plan(manifest, distro, options.requested, options.no_greeter)?;
    println!("install plan");
    println!(
        "  os: {}",
        os.get("PRETTY_NAME")
            .map(String::as_str)
            .unwrap_or("unknown")
    );
    println!("  distro: {} ({})", distro.name, distro.id);
    println!("  tier: {} (version {})", version.tier, version.id);
    if !version.dep_layer.is_empty() {
        println!("  dependency layer: {}", version.dep_layer.join(", "));
    }
    println!("  channel: {}", manifest.ecosystem.channel);
    println!("  latest: {}", options.latest);
    if options.no_greeter {
        println!("  greeter: skipped (--no-greeter)");
    }
    let lock = manifest.lock()?;
    println!("  components:");
    for id in &resolved.components {
        if let Some(component) = manifest.components.get(id) {
            let commit = if options.latest {
                resolve_commit(&component.repo, &component.git_ref)?
            } else {
                lock.components[id].commit.clone()
            };
            println!("    - {} ({}) @ {}", component.id, component.name, commit);
        }
    }
    println!("  package manager: {}", distro.package_manager);
    println!(
        "  declared host packages ({}): {}",
        resolved.packages.len(),
        resolved.packages.join(", ")
    );
    if !resolved.private_dependencies.is_empty() {
        println!(
            "  private build dependencies (not installed by this command): {}",
            resolved.private_dependencies.join(", ")
        );
    }
    let report = preflight::check(manifest, distro, &resolved.components, &SystemRunner)?;
    report.print();
    if dry_run {
        if report.has_blocking() {
            println!("dry-run: blocking preflight issues listed above; a real install would stop");
        }
        println!("dry-run: no changes made");
        Ok(())
    } else if report.has_blocking() {
        bail!("preflight failed; no packages or files were changed");
    } else {
        bail!("install engine is not implemented; resolved source references were not built or installed")
    }
}
