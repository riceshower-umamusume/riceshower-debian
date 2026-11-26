use crate::manifest::DistroManifest;
use crate::state;
use anyhow::{bail, Result};

pub(super) fn uninstall_plan(distro: &DistroManifest, dry_run: bool, purge: bool) -> Result<()> {
    if distro.family == "nixos" {
        bail!("NixOS removal is declarative; remove the module import instead");
    }
    let state_path = state::state_path();
    match state::load(&state_path)? {
        Some(saved) => {
            println!("uninstall plan");
            println!("  distro: {} ({})", distro.name, distro.id);
            println!("  installed components: {}", saved.components.len());
            println!("  purge dependencies: {purge}");
        }
        None => println!(
            "uninstall: no state at {}; nothing to remove",
            state_path.display()
        ),
    }
    if dry_run {
        println!("dry-run: no changes made");
        Ok(())
    } else {
        bail!(
            "uninstall engine is not implemented in this scaffold; run with --dry-run for the plan"
        )
    }
}
