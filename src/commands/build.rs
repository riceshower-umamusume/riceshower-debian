use crate::build;
use crate::manifest::Manifest;
use crate::process::SystemRunner;
use crate::source;
use anyhow::{Context, Result};

pub(super) fn build_component(
    manifest: &Manifest,
    component_id: &str,
    commit: Option<&str>,
    dry_run: bool,
) -> Result<()> {
    let component = manifest
        .components
        .get(component_id)
        .with_context(|| format!("unknown component '{component_id}'"))?;
    let lock = manifest.lock()?;
    let commit = commit
        .map(str::to_string)
        .unwrap_or_else(|| lock.components[component_id].commit.clone());
    let cache = source::cache_root()?;

    println!("build plan");
    println!("  component: {} ({})", component.id, component.name);
    println!("  commit: {commit}");
    println!("  cache: {}", cache.display());
    if dry_run {
        println!("dry-run: source fetch and build were not run");
        return Ok(());
    }

    let source_dir = source::acquire_component(component, &commit, &SystemRunner)?;
    let outcome = build::build_component(
        component,
        &commit,
        &source_dir,
        &cache,
        build::SYSTEM_PREFIX,
        &[],
        &SystemRunner,
    )?;
    println!("source: {}", outcome.source_dir.display());
    println!("staging: {}", outcome.staging_dir.display());
    println!(
        "inventory: {} ({} entries)",
        outcome.inventory_path.display(),
        outcome.inventory.files.len()
    );
    Ok(())
}
