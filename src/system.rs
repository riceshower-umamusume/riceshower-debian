use crate::process::Runner;
use crate::profile::ResolvedProfiles;
use anyhow::{Context, Result};
use std::fs;

const APT_SOURCES_LIST: &str = "/etc/apt/sources.list";
const APT_SOURCES_DIR: &str = "/etc/apt/sources.list.d";

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

fn apt_source_files() -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let main = Path::new(APT_SOURCES_LIST);
    if main.is_file() {
        files.push(main.to_path_buf());
    }

    let directory = Path::new(APT_SOURCES_DIR);
    if directory.is_dir() {
        for entry in fs::read_dir(directory)
            .with_context(|| format!("reading {}", directory.display()))?
        {
            let path = entry?.path();
            let supported_extension = matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("list" | "sources")
            );
            if supported_extension && path.is_file() {
                files.push(path);
            }
        }
    }

    files.sort();
    files.dedup();
    Ok(files)
}

fn ensure_components(runner: &dyn Runner, sources: &[String], dry_run: bool) -> Result<()> {
    let mut found_debian_source = false;
    for path in apt_source_files()? {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let Some(updated) = add_components(&content, sources) else {
            if contains_debian_archive_source(&content) {
                found_debian_source = true;
            }
            continue;
        };

        found_debian_source = true;
        if dry_run {
            println!(
                "system: would enable apt components {} in {}",
                sources.join(", "),
                path.display()
            );
            continue;
        }

        let path_string = path
            .to_str()
            .with_context(|| format!("non-UTF-8 APT source path: {}", path.display()))?;
        let output = runner.run_with_stdin("tee", &[path_string], &updated)?;
        if !output.success {
            anyhow::bail!(
                "writing {} failed: {}",
                path.display(),
                output.stderr.trim()
            );
        }
        println!(
            "system: enabled apt components {} in {}",
            sources.join(", "),
            path.display()
        );
    }

    if !found_debian_source {
        anyhow::bail!(
            "no active Debian archive entries found in /etc/apt/sources.list or /etc/apt/sources.list.d/*.list|*.sources; configure a Debian repository before installing"
        );
    }
    Ok(())
}

fn contains_debian_archive_source(content: &str) -> bool {
    if content.lines().any(|line| {
        line.trim_start()
            .split_once(':')
            .is_some_and(|(field, _)| field.eq_ignore_ascii_case("URIs"))
    }) {
        return deb822_uris(content)
            .iter()
            .any(|uri| is_debian_archive_uri(uri));
    }

    content.lines().any(|line| {
        let without_comment = line.split('#').next().unwrap_or_default();
        let tokens: Vec<&str> = without_comment.split_whitespace().collect();
        traditional_uri_index(&tokens)
            .and_then(|index| tokens.get(index))
            .is_some_and(|uri| is_debian_archive_uri(uri))
    })
}

fn add_components(content: &str, sources: &[String]) -> Option<String> {
    let is_deb822 = content.lines().any(|line| {
        line.trim_start()
            .split_once(':')
            .is_some_and(|(field, _)| field.eq_ignore_ascii_case("URIs"))
    });
    if is_deb822 {
        add_deb822_components(content, sources)
    } else {
        add_traditional_components(content, sources)
    }
}

fn is_debian_archive_uri(uri: &str) -> bool {
    let Some(authority) = uri.split_once("://").map(|(_, rest)| rest) else {
        return false;
    };
    let host = authority
        .split('/')
        .next()
        .unwrap_or_default()
        .split('@')
        .next_back()
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default()
        .trim_matches(['[', ']'])
        .to_ascii_lowercase();

    host == "debian.org" || host.ends_with(".debian.org")
}

fn traditional_uri_index(tokens: &[&str]) -> Option<usize> {
    if !matches!(tokens.first().copied(), Some("deb" | "deb-src")) {
        return None;
    }
    let mut index = 1;
    if tokens.get(index).is_some_and(|token| token.starts_with('[')) {
        while index < tokens.len() {
            let closes_options = tokens[index].ends_with(']');
            index += 1;
            if closes_options {
                break;
            }
        }
    }
    (index < tokens.len()).then_some(index)
}

fn add_traditional_components(content: &str, sources: &[String]) -> Option<String> {
    let mut changed = false;
    let mut output = Vec::new();

    for line in content.lines() {
        let (entry, comment) = match line.split_once('#') {
            Some((entry, comment)) => (entry, Some(comment)),
            None => (line, None),
        };
        let mut tokens: Vec<&str> = entry.split_whitespace().collect();
        let Some(uri_index) = traditional_uri_index(&tokens) else {
            output.push(line.to_string());
            continue;
        };
        let Some(uri) = tokens.get(uri_index) else {
            output.push(line.to_string());
            continue;
        };
        if !is_debian_archive_uri(uri) {
            output.push(line.to_string());
            continue;
        }

        let suite_index = uri_index + 1;
        if suite_index >= tokens.len() {
            output.push(line.to_string());
            continue;
        }
        let mut additions = Vec::new();
        for component in sources {
            if !tokens[suite_index + 1..]
                .iter()
                .any(|existing| *existing == component)
                && !additions.iter().any(|existing: &&String| *existing == component)
            {
                additions.push(component);
            }
        }
        if !additions.is_empty() {
            tokens.extend(additions.into_iter().map(String::as_str));
            changed = true;
            let mut rebuilt = tokens.join(" ");
            if let Some(comment) = comment {
                rebuilt.push_str(" #");
                rebuilt.push_str(comment);
            }
            output.push(rebuilt);
        } else {
            output.push(line.to_string());
        }
    }

    changed.then(|| {
        let mut updated = output.join("\n");
        if content.ends_with('\n') {
            updated.push('\n');
        }
        updated
    })
}

fn deb822_uris(stanza: &str) -> Vec<String> {
    let mut uris = Vec::new();
    let mut in_uris = false;
    for line in stanza.lines() {
        if line.chars().next().is_some_and(char::is_whitespace) {
            if in_uris {
                uris.extend(line.split_whitespace().map(str::to_string));
            }
            continue;
        }
        let Some((field, value)) = line.split_once(':') else {
            in_uris = false;
            continue;
        };
        in_uris = field.eq_ignore_ascii_case("URIs");
        if in_uris {
            uris.extend(value.split_whitespace().map(str::to_string));
        }
    }
    uris
}

fn add_deb822_components(content: &str, sources: &[String]) -> Option<String> {
    let had_trailing_newline = content.ends_with('\n');
    let mut output_stanzas = Vec::new();
    let mut current = Vec::new();
    let mut changed = false;

    for line in content.lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                changed |= update_deb822_stanza(&mut current, sources);
                output_stanzas.push(current.join("\n"));
                current.clear();
            }
            output_stanzas.push(String::new());
        } else {
            current.push(line.to_string());
        }
    }
    if !current.is_empty() {
        changed |= update_deb822_stanza(&mut current, sources);
        output_stanzas.push(current.join("\n"));
    }

    if !changed {
        return None;
    }
    let mut updated = output_stanzas.join("\n");
    if had_trailing_newline {
        updated.push('\n');
    }
    Some(updated)
}

fn update_deb822_stanza(lines: &mut Vec<String>, sources: &[String]) -> bool {
    let uris: Vec<String> = deb822_uris(&lines.join("\n"));
    if !uris.iter().any(|uri| is_debian_archive_uri(uri)) {
        return false;
    }

    let Some(index) = lines.iter().position(|line| {
        line.split_once(':')
            .is_some_and(|(field, _)| field.eq_ignore_ascii_case("Components"))
    }) else {
        let mut components = sources.to_vec();
        components.sort();
        lines.push(format!("Components: {}", components.join(" ")));
        return true;
    };

    let mut end = index + 1;
    while end < lines.len() && lines[end].chars().next().is_some_and(char::is_whitespace) {
        end += 1;
    }
    let mut components: Vec<String> = lines[index]
        .split_once(':')
        .map(|(_, value)| value.split_whitespace().map(str::to_string).collect())
        .unwrap_or_default();
    for continuation in &lines[index + 1..end] {
        components.extend(continuation.split_whitespace().map(str::to_string));
    }

    let mut changed = false;
    for component in sources {
        if !components.iter().any(|existing| existing == component) {
            components.push(component.clone());
            changed = true;
        }
    }
    if changed {
        lines.splice(index..end, [format!("Components: {}", components.join(" "))]);
    }
    changed
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
    fn adds_missing_components_to_sid_sources_list() {
        let content = "# Debian Sid\ndeb http://deb.debian.org/debian sid main\n";
        let updated =
            add_components(content, &["contrib".to_string(), "non-free".to_string()]).unwrap();
        assert!(updated.contains("sid main contrib non-free"));
        assert!(add_components(&updated, &["contrib".to_string()]).is_none());
    }

    #[test]
    fn adds_components_to_deb822_and_preserves_firmware_component() {
        let content = "Types: deb\nURIs: http://deb.debian.org/debian\nSuites: sid\nComponents: main non-free-firmware\n";
        let updated = add_components(
            content,
            &["contrib".to_string(), "non-free".to_string()],
        )
        .unwrap();
        assert!(updated.contains("main non-free-firmware contrib non-free"));
        assert!(add_components(&updated, &["contrib".to_string()]).is_none());
    }

    #[test]
    fn adds_components_when_deb822_stanza_has_no_components_field() {
        let content = "Types: deb\nURIs: http://deb.debian.org/debian\nSuites: sid\n";
        let updated = add_components(content, &["contrib".to_string()]).unwrap();
        assert!(updated.contains("Components: contrib"));
    }

    #[test]
    fn does_not_modify_third_party_sources() {
        let content = "deb https://packages.example.com/debian stable main\n";
        assert!(add_components(content, &["contrib".to_string()]).is_none());
    }

    #[test]
    fn updates_only_official_stanza_in_mixed_deb822_file() {
        let content = concat!(
            "Types: deb\nURIs: https://packages.example.com/debian\n",
            "Suites: stable\nComponents: main\n\n",
            "Types: deb\nURIs: http://deb.debian.org/debian\n",
            "Suites: sid\nComponents: main\n"
        );
        let updated = add_components(content, &["contrib".to_string()]).unwrap();
        assert!(updated.contains("Suites: stable\nComponents: main\n"));
        assert!(updated.contains("Suites: sid\nComponents: main contrib"));
    }

    #[test]
    fn recognizes_debian_archive_hosts_without_touching_lookalikes() {
        assert!(is_debian_archive_uri("https://deb.debian.org/debian"));
        assert!(is_debian_archive_uri("https://security.debian.org/debian-security"));
        assert!(!is_debian_archive_uri("https://debian.org.example.com/debian"));
        assert!(!is_debian_archive_uri("https://example.com/debian"));
    }

    #[test]
    fn picks_microcode_by_cpu_vendor() {
        assert_eq!(microcode_package(Some("intel")), Some("intel-microcode"));
        assert_eq!(microcode_package(Some("amd")), Some("amd64-microcode"));
        assert_eq!(microcode_package(None), None);
    }
}
