use crate::apps;
use crate::distro;
use crate::hardware;
use crate::manifest::Manifest;
use crate::process::SystemRunner;
use anyhow::{bail, Result};
use std::path::Path;

pub(super) fn wizard(
    manifest: &Manifest,
    dry_run: bool,
    yes: bool,
    dotfiles_dir: &Path,
    apps_dir: &Path,
) -> Result<()> {
    println!("riceshower-debian wizard");
    let os = distro::read_os_release()?;
    let distro_id = distro::detect_distro(&os);
    println!("distro: {distro_id}");
    if let Some(distro) = manifest.distros.get(&distro_id) {
        if let Some(version) = super::version::matching_version(distro, &os) {
            println!("suite: {} (tier {})", version.id, version.tier);
        }
    }

    let detected = hardware::detect(&SystemRunner)?;
    println!("gpu: {}", detected.gpu);
    for output in &detected.outputs {
        if let Some(mode) = output.best_mode() {
            println!("  {}: {}", output.connector, mode.label());
        }
    }
    println!(
        "recommended gpu packages: {}",
        detected.gpu.recommended_packages().join(", ")
    );

    let nameservers = crate::dns::nameservers();
    println!(
        "dns: {}",
        if nameservers.is_empty() {
            "none".to_string()
        } else {
            nameservers.join(", ")
        }
    );

    println!("dotfiles:");
    if dry_run {
        println!("  would apply Umbriel/Noctalia/fcitx5/shell and display config");
    } else if yes {
        super::config::config(dotfiles_dir, false)?;
    } else {
        println!("  run ./install.sh config to apply");
    }

    println!("apps:");
    apps::print(apps_dir)?;

    if dry_run {
        println!("dry-run: no changes made");
        return Ok(());
    }
    if yes {
        println!("wizard: applying the full install plan");
        let os = distro::read_os_release()?;
        let distro_id = distro::detect_distro(&os);
        if let Some(distro) = manifest.distros.get(&distro_id) {
            super::install::install_plan(
                manifest,
                distro,
                &os,
                false,
                dotfiles_dir,
                super::install::InstallOptions {
                    requested: &[],
                    no_greeter: false,
                    latest: false,
                    yes: true,
                },
            )?;
        } else {
            bail!("distro '{distro_id}' is not in the manifest");
        }
    } else {
        println!("next: ./install.sh install --dry-run, then ./install.sh install --yes");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nameservers() {
        let content = "nameserver 10.0.0.1\n# comment\nnameserver 1.1.1.1\n";
        assert_eq!(parse_nameservers(content), vec!["10.0.0.1", "1.1.1.1"]);
    }
}
