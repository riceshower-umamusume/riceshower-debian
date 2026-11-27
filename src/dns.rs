use crate::process::Runner;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

const DROP_IN: &str = "/etc/systemd/resolved.conf.d/90-riceshower-debian.conf";

pub fn nameservers() -> Vec<String> {
    let content = fs::read_to_string("/run/systemd/resolve/resolv.conf")
        .or_else(|_| fs::read_to_string("/etc/resolv.conf"))
        .unwrap_or_default();
    parse_nameservers(&content)
}

fn parse_nameservers(content: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("nameserver ")
                .map(str::trim)
                .filter(|value| !value.is_empty() && *value != "127.0.0.53")
                .map(str::to_string)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_resolved_stub() {
        let content = "nameserver 127.0.0.53\nnameserver 10.0.0.1\nnameserver fec0::1\n";
        assert_eq!(parse_nameservers(content), vec!["10.0.0.1", "fec0::1"]);
    }
}

pub fn apply(runner: &dyn Runner, dry_run: bool) -> Result<()> {
    let servers = nameservers();
    if servers.is_empty() {
        println!("dns: no nameservers found; skipped");
        return Ok(());
    }
    let body = format!(
        "# Managed by riceshower-debian\n[Resolve]\nDNS={}\n",
        servers.join(" ")
    );
    if dry_run {
        println!("dns: would write {DROP_IN} with {}", servers.join(", "));
        return Ok(());
    }
    if let Some(parent) = Path::new(DROP_IN).parent() {
        let output = runner.run("mkdir", &["-p", &parent.display().to_string()])?;
        if !output.success {
            anyhow::bail!(
                "creating {} failed: {}",
                parent.display(),
                output.stderr.trim()
            );
        }
    }
    let temporary = format!("{DROP_IN}.tmp");
    let output = runner.run_with_stdin("tee", &[temporary.as_str()], &body)?;
    if !output.success {
        anyhow::bail!("writing {temporary} failed: {}", output.stderr.trim());
    }
    let output = runner.run("mv", &[temporary.as_str(), DROP_IN])?;
    if !output.success {
        anyhow::bail!("installing {DROP_IN} failed: {}", output.stderr.trim());
    }
    println!("dns: {DROP_IN} -> {}", servers.join(", "));
    Ok(())
}

pub fn read_systemd() -> Result<String> {
    fs::read_to_string(DROP_IN).with_context(|| format!("reading {DROP_IN}"))
}
