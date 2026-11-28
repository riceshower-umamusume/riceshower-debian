use crate::process::Runner;
use crate::profile::ResolvedProfiles;
use anyhow::{Context, Result};
use std::fs;

const DEBIAN_SOURCES: &str = "/etc/apt/sources.list.d/debian.sources";

pub fn prepare(
    profiles: &ResolvedProfiles,
    runner: &dyn Runner,
    dry_run: bool,
) -> Result<Vec<String>> {
    let mut extras = Vec::new();
    if !profiles.sources.is_empty() {
        ensure_components(runner, &profiles.sources, dry_run)?;
    }
    for arch in &profiles.multiarch {
        if !valid_arch(arch) {
            anyhow::bail!("invalid architecture '{arch}'");
        }
        if dry_run {
            println!("system: would add architecture {arch}");
        } else {
            let output = runner.run("dpkg", &["--add-architecture", arch])?;
            if !output.success {
                anyhow::bail!(
                    "dpkg --add-architecture {arch} failed: {}",
                    output.stderr.trim()
                );
            }
        }
    }
    if profiles.microcode {
        if let Some(package) = microcode_package(cpu_vendor()) {
            extras.push(package.to_string());
        }
    }
    if !dry_run && (!profiles.sources.is_empty() || !profiles.multiarch.is_empty()) {
        let output = runner.run("apt-get", &["update"])?;
        if !output.success {
            anyhow::bail!("apt-get update failed: {}", output.stderr.trim());
        }
    }
    Ok(extras)
}

fn valid_arch(arch: &str) -> bool {
    !arch.is_empty()
        && arch
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn ensure_components(runner: &dyn Runner, sources: &[String], dry_run: bool) -> Result<()> {
    let content =
        fs::read_to_string(DEBIAN_SOURCES).with_context(|| format!("reading {DEBIAN_SOURCES}"))?;
    let Some(updated) = add_components(&content, sources) else {
        return Ok(());
    };
    if dry_run {
        println!("system: would enable apt components {}", sources.join(", "));
        return Ok(());
    }
    let output = runner.run_with_stdin("tee", &[DEBIAN_SOURCES], &updated)?;
    if !output.success {
        anyhow::bail!("writing {DEBIAN_SOURCES} failed: {}", output.stderr.trim());
    }
    println!("system: enabled apt components {}", sources.join(", "));
    Ok(())
}

fn add_components(content: &str, sources: &[String]) -> Option<String> {
    let mut changed = false;
    let mut lines = Vec::new();
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("Components:") {
            let mut current: Vec<String> = rest.split_whitespace().map(str::to_string).collect();
            for component in sources {
                if !current.iter().any(|item| item == component) {
                    current.push(component.clone());
                    changed = true;
                }
            }
            lines.push(format!("Components: {}", current.join(" ")));
        } else {
            lines.push(line.to_string());
        }
    }
    if changed {
        Some(lines.join("\n") + "\n")
    } else {
        None
    }
}

fn cpu_vendor() -> Option<&'static str> {
    let content = fs::read_to_string("/proc/cpuinfo").ok()?;
    if content.contains("GenuineIntel") {
        Some("intel")
    } else if content.contains("AuthenticAMD") {
        Some("amd")
    } else {
        None
    }
}

fn microcode_package(vendor: Option<&str>) -> Option<&'static str> {
    match vendor {
        Some("intel") => Some("intel-microcode"),
        Some("amd") => Some("amd64-microcode"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_missing_apt_components_once() {
        let content = "Types: deb\nComponents: main non-free-firmware\n";
        let updated =
            add_components(content, &["contrib".to_string(), "non-free".to_string()]).unwrap();
        assert!(updated.contains("main non-free-firmware contrib non-free"));
        assert!(add_components(&updated, &["contrib".to_string()]).is_none());
    }

    #[test]
    fn picks_microcode_by_cpu_vendor() {
        assert_eq!(microcode_package(Some("intel")), Some("intel-microcode"));
        assert_eq!(microcode_package(Some("amd")), Some("amd64-microcode"));
        assert_eq!(microcode_package(None), None);
    }
}
