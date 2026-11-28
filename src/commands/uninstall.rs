use crate::dns;
use crate::manifest::DistroManifest;
use crate::process::{Runner, SudoRunner, SystemRunner};
use crate::state;
use anyhow::{bail, Result};
use std::path::Path;

pub(super) fn uninstall_plan(distro: &DistroManifest, dry_run: bool, purge: bool) -> Result<()> {
    let state_path = state::state_path();
    let saved = match state::load(&state_path)? {
        Some(saved) => saved,
        None => {
            println!(
                "uninstall: no state at {}; nothing to remove",
                state_path.display()
            );
            return Ok(());
        }
    };
    let files: Vec<&String> = saved
        .components
        .values()
        .flat_map(|component| component.files.iter())
        .filter(|file| !Path::new(file).is_dir())
        .collect();
    println!("uninstall plan");
    println!("  distro: {} ({})", distro.name, distro.id);
    println!("  installed components: {}", saved.components.len());
    println!("  files: {}", files.len());
    println!("  purge dependencies: {purge}");
    for file in &files {
        println!("  remove {file}");
    }
    if dry_run {
        println!("dry-run: no changes made");
        return Ok(());
    }

    let sudo = SudoRunner::new(SystemRunner);
    let mut failures = Vec::new();
    for file in files {
        if !Path::new(file).exists() {
            continue;
        }
        let output = sudo.run("rm", &["-f", "--", file])?;
        if !output.success {
            failures.push(format!("{file}: {}", output.stderr.trim()));
        }
    }
    if purge && !saved.packages.is_empty() {
        let mut args = vec!["remove", "-y"];
        args.extend(saved.packages.iter().map(String::as_str));
        let output = sudo.run("apt-get", &args)?;
        if !output.success {
            failures.push(format!("apt-get remove: {}", output.stderr.trim()));
        }
    }
    if !failures.is_empty() {
        bail!("uninstall incomplete:\n{}", failures.join("\n"));
    }
    let output = sudo.run("rm", &["-f", "--", dns::DROP_IN])?;
    if !output.success {
        failures.push(format!(
            "removing {}: {}",
            dns::DROP_IN,
            output.stderr.trim()
        ));
    }
    let output = sudo.run("rm", &["-f", "--", &state_path.display().to_string()])?;
    if !output.success {
        bail!("removing state failed: {}", output.stderr.trim());
    }
    println!("uninstall complete");
    Ok(())
}
