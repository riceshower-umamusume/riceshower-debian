use crate::apps;
use crate::distro;
use crate::hardware;
use crate::manifest::Manifest;
use crate::process::SystemRunner;
use anyhow::Result;
use std::fs;
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

    let nameservers = read_nameservers()?;
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

    if !dry_run {
        println!("next: ./install.sh install --dry-run, then ./install.sh install --yes");
    } else {
        println!("dry-run: no changes made");
    }
    Ok(())
}

fn read_nameservers() -> Result<Vec<String>> {
    let content = fs::read_to_string("/etc/resolv.conf").unwrap_or_default();
    Ok(parse_nameservers(&content))
}

fn parse_nameservers(content: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("nameserver ")
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .collect()
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
