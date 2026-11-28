use super::version::matching_version;
use crate::manifest::{DistroManifest, Manifest};
use crate::plan::resolve_install_plan;
use crate::preflight;
use crate::process::SystemRunner;
use crate::state;
use anyhow::Result;
use std::collections::BTreeMap;

pub(super) fn doctor(
    manifest: &Manifest,
    distro_id: &str,
    distro: Option<&DistroManifest>,
    os: &BTreeMap<String, String>,
) -> Result<()> {
    println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    println!(
        "os: {}",
        os.get("PRETTY_NAME")
            .map(String::as_str)
            .unwrap_or("unknown")
    );
    println!("detected distro: {distro_id}");
    if let Some(distro) = distro {
        println!("package manager: {}", distro.package_manager);
        match matching_version(distro, os) {
            Some(version) => println!("matched version: {} (tier {})", version.id, version.tier),
            None => println!(
                "matched version: none; supported: {}",
                distro
                    .versions
                    .iter()
                    .map(|version| version.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
        for version in &distro.versions {
            let mut details = Vec::new();
            if let Some(codename) = &version.codename {
                details.push(format!("codename {codename}"));
            }
            if version.rolling {
                details.push("rolling".to_string());
            }
            if let Some(notes) = &version.notes {
                details.push(notes.clone());
            }
            if !version.dep_layer.is_empty() {
                details.push(format!("dep layer: {}", version.dep_layer.join(", ")));
            }
            if details.is_empty() {
                println!("  - {}: tier {}", version.id, version.tier);
            } else {
                println!(
                    "  - {}: tier {} ({})",
                    version.id,
                    version.tier,
                    details.join("; ")
                );
            }
        }
        if matching_version(distro, os).is_some() {
            match resolve_install_plan(manifest, distro, &[], false) {
                Ok(resolved) => {
                    match preflight::check(manifest, distro, &resolved.components, &SystemRunner) {
                        Ok(report) => report.print(),
                        Err(error) => println!("preflight: unavailable: {error:#}"),
                    }
                }
                Err(error) => println!("preflight: could not resolve plan: {error:#}"),
            }
        } else {
            println!("preflight: skipped; version not listed in manifest");
        }
    } else {
        println!("support: unsupported (no exact distro entry in manifest)");
    }
    let lock = manifest.lock()?;
    println!(
        "lock: {} components, generated {}",
        lock.components.len(),
        lock.generated
    );
    let state_path = state::state_path();
    match state::load(&state_path)? {
        Some(saved) => println!(
            "state: {} components installed ({})",
            saved.components.len(),
            state_path.display()
        ),
        None => println!("state: none ({})", state_path.display()),
    }
    Ok(())
}
