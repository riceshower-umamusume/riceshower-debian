use crate::manifest::{DistroManifest, DistroVersion};
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;

pub(super) fn require_supported_version<'a>(
    distro: &'a DistroManifest,
    os: &BTreeMap<String, String>,
) -> Result<&'a DistroVersion> {
    let version = matching_version(distro, os).with_context(|| {
        format!(
            "version of {} is not listed in the manifest; see docs/design.md",
            distro.name
        )
    })?;
    if version.tier >= 3 {
        bail!(
            "{} {} is Tier {}; main-channel install/update is not supported",
            distro.name,
            version.id,
            version.tier
        );
    }
    Ok(version)
}

pub(super) fn matching_version<'a>(
    distro: &'a DistroManifest,
    os: &BTreeMap<String, String>,
) -> Option<&'a DistroVersion> {
    let version = os.get("VERSION_ID").map(|value| value.to_ascii_lowercase());
    let codename = os
        .get("VERSION_CODENAME")
        .map(|value| value.to_ascii_lowercase());
    let pretty_name = os
        .get("PRETTY_NAME")
        .map(|value| value.to_ascii_lowercase());
    // Prefer explicit release and codename matches; use pretty-name parsing only as fallback.
    if let Some(matched) = distro.versions.iter().find(|candidate| {
        let id = candidate.id.to_ascii_lowercase();
        version.as_deref().is_some_and(|value| {
            value == id
                || value
                    .strip_prefix(&id)
                    .is_some_and(|suffix| suffix.starts_with('.'))
        })
    }) {
        return Some(matched);
    }

    if let Some(matched) = distro.versions.iter().find(|candidate| {
        codename.as_deref().is_some_and(|value| {
            candidate
                .codename
                .as_deref()
                .is_some_and(|expected| value.eq_ignore_ascii_case(expected))
        })
    }) {
        return Some(matched);
    }

    if let Some(matched) = distro.versions.iter().find(|candidate| {
        let id = candidate.id.to_ascii_lowercase();
        let matches = |value: &str| {
            value == id
                || value
                    .strip_prefix(&id)
                    .is_some_and(|suffix| suffix.starts_with('.'))
        };
        pretty_name.as_deref().is_some_and(|pretty| {
            pretty
                .split(|character: char| !character.is_ascii_alphanumeric() && character != '.')
                .any(matches)
        })
    }) {
        return Some(matched);
    }

    distro.versions.iter().find(|candidate| candidate.rolling)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn distro(versions: Vec<DistroVersion>) -> DistroManifest {
        DistroManifest {
            id: "example".to_string(),
            name: "Example".to_string(),
            family: "debian".to_string(),
            package_manager: "apt".to_string(),
            versions,
            package_map: BTreeMap::new(),
            private_dependencies: Vec::new(),
            private_build_packages: Vec::new(),
        }
    }

    fn version(id: &str, rolling: bool, codename: Option<&str>) -> DistroVersion {
        DistroVersion {
            id: id.to_string(),
            tier: 1,
            codename: codename.map(str::to_string),
            rolling,
            notes: None,
            dep_layer: Vec::new(),
        }
    }

    #[test]
    fn debian_suites_match_their_manifest_entries() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("manifest");
        let manifest = crate::manifest::load(&root).unwrap();

        let stable = BTreeMap::from([
            ("VERSION_ID".to_string(), "13".to_string()),
            ("VERSION_CODENAME".to_string(), "trixie".to_string()),
        ]);
        assert_eq!(
            matching_version(&manifest.distros["debian-stable"], &stable)
                .unwrap()
                .id,
            "13"
        );

        let testing = BTreeMap::from([
            ("VERSION_ID".to_string(), "14".to_string()),
            ("VERSION_CODENAME".to_string(), "forky".to_string()),
        ]);
        assert_eq!(
            matching_version(&manifest.distros["debian-testing"], &testing)
                .unwrap()
                .id,
            "14"
        );

        let sid = BTreeMap::from([
            ("VERSION_ID".to_string(), "14".to_string()),
            ("VERSION_CODENAME".to_string(), "forky".to_string()),
            (
                "PRETTY_NAME".to_string(),
                "Debian GNU/Linux forky/sid".to_string(),
            ),
        ]);
        assert_eq!(
            matching_version(&manifest.distros["debian-sid"], &sid)
                .unwrap()
                .id,
            "sid"
        );
    }

    #[test]
    fn explicit_release_is_not_masked_by_rolling_entry() {
        let distro = distro(vec![
            version("sid", true, None),
            version("trixie", false, Some("trixie")),
        ]);
        let os = BTreeMap::from([
            ("VERSION_ID".to_string(), "13".to_string()),
            ("VERSION_CODENAME".to_string(), "trixie".to_string()),
        ]);
        assert_eq!(matching_version(&distro, &os).unwrap().id, "trixie");
    }

    #[test]
    fn codename_match_takes_precedence_over_pretty_name_tokens() {
        let distro = distro(vec![
            version("sid", true, None),
            version("trixie", false, Some("trixie")),
        ]);
        let os = BTreeMap::from([
            ("VERSION_CODENAME".to_string(), "trixie".to_string()),
            (
                "PRETTY_NAME".to_string(),
                "Debian GNU/Linux sid".to_string(),
            ),
        ]);
        assert_eq!(matching_version(&distro, &os).unwrap().id, "trixie");
    }

    #[test]
    fn rolling_is_used_only_when_no_release_or_codename_matches() {
        let distro = distro(vec![
            version("rolling", true, None),
            version("stable", false, Some("stable")),
        ]);
        let os = BTreeMap::from([("VERSION_ID".to_string(), "unknown".to_string())]);
        assert_eq!(matching_version(&distro, &os).unwrap().id, "rolling");
    }

    #[test]
    fn caramos_version_is_detected_from_pretty_name() {
        let distro = distro(vec![version("1.5", false, None)]);
        let os = BTreeMap::from([("PRETTY_NAME".to_string(), "CaramOS 1.5.1".to_string())]);
        assert_eq!(matching_version(&distro, &os).unwrap().id, "1.5");
    }

    #[test]
    fn tier_three_versions_are_rejected_for_install_and_update() {
        let distro = distro(vec![DistroVersion {
            tier: 3,
            ..version("24.04", false, Some("noble"))
        }]);
        let os = BTreeMap::from([("VERSION_ID".to_string(), "24.04".to_string())]);
        assert!(require_supported_version(&distro, &os).is_err());
    }

    #[test]
    fn release_prefix_requires_a_version_boundary() {
        let distro = distro(vec![version("24.0", false, None)]);
        let os = BTreeMap::from([("VERSION_ID".to_string(), "24.01".to_string())]);
        assert!(matching_version(&distro, &os).is_none());
    }
}
