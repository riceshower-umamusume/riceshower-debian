use super::version::require_supported_version;
use crate::manifest::{DistroManifest, EcosystemLock, Manifest};
use crate::source::resolve_commit;
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::process::Command;

pub(super) fn update_plan(
    manifest: &Manifest,
    distro: &DistroManifest,
    os: &BTreeMap<String, String>,
    dry_run: bool,
    latest: bool,
) -> Result<()> {
    let version = require_supported_version(distro, os)?;
    let lock = manifest.lock()?;
    println!("update plan");
    println!("  distro: {} ({})", distro.name, distro.id);
    println!("  tier: {} (version {})", version.tier, version.id);
    println!("  pinned components: {}", lock.components.len());
    println!("  latest: {latest}");

    let resolved = if latest {
        if dry_run {
            println!("  action: resolve remote refs without writing the lockfile");
        } else {
            println!("  action: resolve remote refs and refresh the lockfile");
        }
        Some(resolve_latest_lock(manifest, &lock)?)
    } else {
        println!("  action: reuse the pinned lockfile");
        None
    };

    for id in &manifest.ecosystem.components {
        let pinned = &lock.components[id].commit;
        if let Some(updated) = &resolved {
            let remote = &updated.components[id].commit;
            let status = if remote == pinned {
                "up-to-date"
            } else {
                "update available"
            };
            println!("  - {id}: {pinned} -> {remote} ({status})");
        } else {
            println!("  - {id}: {pinned}");
        }
    }

    if dry_run {
        println!("dry-run: no changes made");
        return Ok(());
    }

    if let Some(mut updated) = resolved {
        let changed = manifest
            .ecosystem
            .components
            .iter()
            .any(|id| updated.components[id].commit != lock.components[id].commit);
        if changed {
            updated.generated = current_timestamp()?;
            manifest.save_lock(&updated)?;
            println!("lockfile refreshed: {}", manifest.ecosystem.lockfile);
        } else {
            println!("lockfile already up-to-date; no write needed");
        }
        println!("installed components were not changed; component deployment is not implemented");
        Ok(())
    } else {
        bail!(
            "component deployment requires the lifecycle engine; use --latest to refresh the lockfile"
        )
    }
}

fn resolve_latest_lock(manifest: &Manifest, lock: &EcosystemLock) -> Result<EcosystemLock> {
    let mut updated = lock.clone();
    for id in &manifest.ecosystem.components {
        let component = &manifest.components[id];
        let commit = resolve_commit(&component.repo, &component.git_ref)?;
        let locked = updated
            .components
            .get_mut(id)
            .with_context(|| format!("lockfile is missing component '{id}'"))?;
        locked.commit = commit;
    }
    Ok(updated)
}

fn current_timestamp() -> Result<String> {
    let output = Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .context("starting date to timestamp lockfile update")?;
    if !output.status.success() {
        bail!("date failed while creating lockfile timestamp");
    }
    let timestamp = String::from_utf8(output.stdout)
        .context("date returned non-UTF-8 timestamp")?
        .trim()
        .to_string();
    validate_utc_timestamp(&timestamp)?;
    Ok(timestamp)
}

fn validate_utc_timestamp(timestamp: &str) -> Result<()> {
    let bytes = timestamp.as_bytes();
    if bytes.len() != 20
        || [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18]
            .into_iter()
            .any(|index| !bytes[index].is_ascii_digit())
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'Z'
    {
        bail!("invalid ISO UTC timestamp: {timestamp}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_iso_utc_timestamps() {
        assert!(validate_utc_timestamp("2026-11-25T14:00:00Z").is_ok());
        assert!(validate_utc_timestamp("2026-11-25T14:00:00+07:00").is_err());
        assert!(validate_utc_timestamp("not-a-date").is_err());
    }
}
