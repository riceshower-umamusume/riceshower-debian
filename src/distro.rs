use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

pub fn parse_os_release(input: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    for raw in input.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(value);
        values.insert(key.trim().to_string(), value.to_string());
    }
    values
}

pub fn read_os_release() -> Result<BTreeMap<String, String>> {
    let content = fs::read_to_string("/etc/os-release").context("reading /etc/os-release")?;
    let mut values = parse_os_release(&content);
    if let Some(suite) = installed_apt_suite() {
        values.insert("DEBIAN_SUITE".to_string(), suite.to_string());
    }
    Ok(values)
}

fn installed_apt_suite() -> Option<&'static str> {
    let mut paths = vec![PathBuf::from("/etc/apt/sources.list")];
    if let Ok(entries) = fs::read_dir("/etc/apt/sources.list.d") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|ext| ext == "sources" || ext == "list")
            {
                paths.push(path);
            }
        }
    }
    let mut found = None;
    for path in paths {
        let Ok(content) = fs::read_to_string(path) else {
            continue;
        };
        for line in content.lines().map(str::trim) {
            if line.starts_with('#') {
                continue;
            }
            let tokens: Vec<&str> = if let Some(rest) = line.strip_prefix("Suites:") {
                rest.split_whitespace().collect()
            } else if let Some(rest) = line.strip_prefix("deb ") {
                rest.split_whitespace().skip(1).collect()
            } else {
                continue;
            };
            for token in tokens {
                match token.to_ascii_lowercase().as_str() {
                    "unstable" | "sid" => return Some("unstable"),
                    "testing" | "forky" => found = Some("testing"),
                    "stable" | "trixie" => found = found.or(Some("stable")),
                    _ => {}
                }
            }
        }
    }
    found
}

fn debian_suite(values: &BTreeMap<String, String>) -> &'static str {
    let version = values
        .get("VERSION_ID")
        .map(|value| value.to_ascii_lowercase());
    let codename = values
        .get("VERSION_CODENAME")
        .map(|value| value.to_ascii_lowercase());
    if let Some(suite) = values.get("DEBIAN_SUITE").map(|value| value.as_str()) {
        return match suite {
            "unstable" | "sid" => "debian-sid",
            "testing" | "forky" => "debian-testing",
            "stable" | "trixie" => "debian-stable",
            _ => "debian",
        };
    }
    let pretty = values
        .get("PRETTY_NAME")
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    if pretty.contains("/sid")
        || pretty
            .split(|character: char| !character.is_ascii_alphanumeric())
            .any(|token| token == "sid")
    {
        return "debian-sid";
    }
    if version
        .as_deref()
        .is_some_and(|value| value == "14" || value.starts_with("14."))
        || codename.as_deref() == Some("forky")
    {
        return "debian-testing";
    }
    if version
        .as_deref()
        .is_some_and(|value| value == "13" || value.starts_with("13."))
        || codename.as_deref() == Some("trixie")
    {
        return "debian-stable";
    }
    "debian"
}

pub fn detect_distro(values: &BTreeMap<String, String>) -> String {
    let id = values
        .get("ID")
        .map(String::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    match id.as_str() {
        "debian" => debian_suite(values),
        "" => "unknown",
        other => other,
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_debian_suites() {
        let stable = parse_os_release("ID=debian\nVERSION_ID=13\nVERSION_CODENAME=trixie\n");
        assert_eq!(detect_distro(&stable), "debian-stable");
        let testing = parse_os_release("ID=debian\nVERSION_ID=14\nVERSION_CODENAME=forky\nPRETTY_NAME=\"Debian GNU/Linux 14 (forky)\"\n");
        assert_eq!(detect_distro(&testing), "debian-testing");
        let sid = parse_os_release("ID=debian\nVERSION_ID=14\nVERSION_CODENAME=forky\nPRETTY_NAME=\"Debian GNU/Linux forky/sid\"\n");
        assert_eq!(detect_distro(&sid), "debian-sid");
        let testing_apt = parse_os_release("ID=debian\nVERSION_ID=14\nVERSION_CODENAME=forky\nPRETTY_NAME=\"Debian GNU/Linux forky/sid\"\nDEBIAN_SUITE=testing\n");
        assert_eq!(detect_distro(&testing_apt), "debian-testing");
    }
}
