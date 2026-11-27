use super::version::require_supported_version;
use crate::build;
use crate::deploy;
use crate::manifest::{DistroManifest, Manifest};
use crate::plan::resolve_install_plan;
use crate::preflight;
use crate::private;
use crate::process::SudoRunner;
use crate::process::SystemRunner;
use crate::source;
use crate::source::resolve_commit;
use crate::state;
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::path::Path;

pub(super) struct InstallOptions<'a> {
    pub requested: &'a [String],
    pub no_greeter: bool,
    pub latest: bool,
    pub yes: bool,
}

pub(super) fn install_plan(
    manifest: &Manifest,
    distro: &DistroManifest,
    os: &BTreeMap<String, String>,
    dry_run: bool,
    dotfiles_dir: &Path,
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
        println!("dry-run: would install packages, build 5 components and deploy to /usr/local, then apply dotfiles");
        println!("dry-run: no changes made");
        return Ok(());
    }
    if !options.yes {
        bail!("real install requires --yes; use --dry-run to inspect the plan");
    }

    let sudo = SudoRunner::new(SystemRunner);
    let mut packages = resolved.packages.clone();
    packages.extend(distro.private_build_packages.iter().cloned());
    packages.sort();
    packages.dedup();
    println!("installing host packages...");
    deploy::install_packages(distro, &packages, &sudo)?;
    let report = preflight::check(manifest, distro, &resolved.components, &SystemRunner)?;
    report.print();
    if report.has_blocking() {
        bail!("preflight still failing after package installation; no files were deployed");
    }

    let cache = source::cache_root()?;
    let private_env = if resolved.private_dependencies.is_empty() {
        Vec::new()
    } else {
        private::environment()
    };
    if !resolved.private_dependencies.is_empty() {
        private::build_all(manifest, &cache, &sudo)?;
    }
    let lock = manifest.lock()?;
    let order = build::build_order(manifest, &resolved.components)?;
    let mut components = Vec::new();
    for id in order {
        let component = manifest
            .components
            .get(&id)
            .with_context(|| format!("component '{id}' disappeared"))?;
        let commit = if options.latest {
            resolve_commit(&component.repo, &component.git_ref)?
        } else {
            lock.components[id.as_str()].commit.clone()
        };
        println!("building {id} @ {commit}...");
        let source_dir = source::acquire_component(component, &commit, &SystemRunner)?;
        let outcome = build::build_component(
            component,
            &commit,
            &source_dir,
            &cache,
            build::SYSTEM_PREFIX,
            &private_env,
            &SystemRunner,
        )?;
        components.push(deploy::DeployComponent {
            id: outcome.component,
            commit: outcome.commit,
            staging_dir: outcome.staging_dir,
            inventory: outcome.inventory,
        });
    }

    println!("deploying to /usr/local...");
    let writer = deploy::SudoWriter { runner: &sudo };
    deploy::deploy_components(
        Path::new("/"),
        &components,
        &packages,
        &distro.id,
        &state::state_path(),
        &writer,
        false,
    )?;
    super::config::config(dotfiles_dir, false)?;
    println!("install complete");
    Ok(())
}
