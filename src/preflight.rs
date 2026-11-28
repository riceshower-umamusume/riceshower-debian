use crate::manifest::{BuildRequirement, ComponentManifest, DistroManifest, Manifest};
use crate::process::Runner;
use crate::version;
use anyhow::{bail, Context, Result};
use std::cmp::Ordering;
use std::env;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckScope {
    BuildPackage,
    RuntimePackage,
    Tool,
    PkgConfig,
    Toolchain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Ok,
    Installable,
    Missing,
    VersionMismatch,
    Private,
    ToolMissing,
    ToolchainUnsupported,
    PkgConfigMissing,
    PkgConfigVersion,
    PkgConfigInvalid,
}

impl CheckStatus {
    fn label(self) -> &'static str {
        match self {
            CheckStatus::Ok => "ok",
            CheckStatus::Installable => "installable",
            CheckStatus::Missing => "missing",
            CheckStatus::VersionMismatch => "version mismatch",
            CheckStatus::Private => "private",
            CheckStatus::ToolMissing => "tool missing",
            CheckStatus::ToolchainUnsupported => "toolchain unsupported",
            CheckStatus::PkgConfigMissing => "pkg-config missing",
            CheckStatus::PkgConfigVersion => "pkg-config version",
            CheckStatus::PkgConfigInvalid => "pkg-config invalid",
        }
    }

    fn blocks(self) -> bool {
        !matches!(
            self,
            CheckStatus::Ok | CheckStatus::Installable | CheckStatus::Private
        )
    }
}

#[derive(Debug, Clone)]
pub struct Check {
    pub scope: CheckScope,
    pub component: String,
    pub key: String,
    pub package: Option<String>,
    pub required: bool,
    pub status: CheckStatus,
    pub detail: String,
}

#[derive(Debug, Clone, Default)]
pub struct PreflightReport {
    pub entries: Vec<Check>,
}

impl PreflightReport {
    pub fn has_blocking(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.required && entry.status.blocks())
    }

    pub fn print(&self) {
        let count = |scope: CheckScope, status: CheckStatus| {
            self.entries
                .iter()
                .filter(|entry| entry.scope == scope && entry.status == status)
                .count()
        };

        println!("preflight:");
        println!(
            "  build packages: {} ok, {} installable, {} missing, {} version mismatch, {} private",
            count(CheckScope::BuildPackage, CheckStatus::Ok),
            count(CheckScope::BuildPackage, CheckStatus::Installable),
            count(CheckScope::BuildPackage, CheckStatus::Missing),
            count(CheckScope::BuildPackage, CheckStatus::VersionMismatch),
            count(CheckScope::BuildPackage, CheckStatus::Private),
        );
        println!(
            "  runtime packages: {} ok, {} installable, {} missing, {} version mismatch, {} private",
            count(CheckScope::RuntimePackage, CheckStatus::Ok),
            count(CheckScope::RuntimePackage, CheckStatus::Installable),
            count(CheckScope::RuntimePackage, CheckStatus::Missing),
            count(CheckScope::RuntimePackage, CheckStatus::VersionMismatch),
            count(CheckScope::RuntimePackage, CheckStatus::Private),
        );
        println!(
            "  tools: {} ok, {} missing",
            count(CheckScope::Tool, CheckStatus::Ok),
            count(CheckScope::Tool, CheckStatus::ToolMissing),
        );
        println!(
            "  pkg-config: {} ok, {} missing, {} version mismatch, {} invalid",
            count(CheckScope::PkgConfig, CheckStatus::Ok),
            count(CheckScope::PkgConfig, CheckStatus::PkgConfigMissing),
            count(CheckScope::PkgConfig, CheckStatus::PkgConfigVersion),
            count(CheckScope::PkgConfig, CheckStatus::PkgConfigInvalid),
        );
        println!(
            "  toolchain: {} ok, {} unsupported",
            count(CheckScope::Toolchain, CheckStatus::Ok),
            count(CheckScope::Toolchain, CheckStatus::ToolchainUnsupported),
        );

        for entry in &self.entries {
            if matches!(entry.status, CheckStatus::Ok | CheckStatus::Installable) {
                continue;
            }
            let package = entry
                .package
                .as_deref()
                .map(|package| format!(" -> {package}"))
                .unwrap_or_default();
            println!(
                "  [{}] {}: {}{} — {}",
                entry.status.label(),
                entry.component,
                entry.key,
                package,
                entry.detail
            );
        }
        println!(
            "  blocking: {}",
            if self.has_blocking() { "yes" } else { "no" }
        );
    }
}

pub fn check(
    manifest: &Manifest,
    distro: &DistroManifest,
    selected: &[String],
    runner: &dyn Runner,
) -> Result<PreflightReport> {
    if distro.package_manager != "apt" {
        bail!("unsupported package manager '{}'", distro.package_manager);
    }
    let mut report = PreflightReport::default();
    for id in selected {
        let component = manifest
            .components
            .get(id)
            .with_context(|| format!("component '{id}' disappeared from the manifest"))?;
        check_component(&mut report, distro, component, runner)?;
    }
    Ok(report)
}

fn check_component(
    report: &mut PreflightReport,
    distro: &DistroManifest,
    component: &ComponentManifest,
    runner: &dyn Runner,
) -> Result<()> {
    for tool in &component.build_tools {
        let executable = tool_executable(tool);
        let available = executable_on_path(executable);
        report.entries.push(Check {
            scope: CheckScope::Tool,
            component: component.id.clone(),
            key: tool.clone(),
            package: None,
            required: true,
            status: if available {
                CheckStatus::Ok
            } else {
                CheckStatus::ToolMissing
            },
            detail: if available {
                format!("{executable} found in PATH")
            } else {
                format!("{executable} not found in PATH")
            },
        });
    }

    if let Some(standard) = component.cxx_standard {
        let (status, detail) = check_cxx(runner, standard);
        report.entries.push(Check {
            scope: CheckScope::Toolchain,
            component: component.id.clone(),
            key: format!("c++{standard}"),
            package: None,
            required: true,
            status,
            detail,
        });
    }

    for key in &component.build_dependencies {
        let requirement = component.build_requirements.get(key).with_context(|| {
            format!(
                "component '{}' has no requirement for '{key}'",
                component.id
            )
        })?;
        if distro.private_dependencies.contains(key) {
            report.entries.push(Check {
                scope: CheckScope::BuildPackage,
                component: component.id.clone(),
                key: key.clone(),
                package: None,
                required: requirement.required,
                status: CheckStatus::Private,
                detail: "private dependency; not installed by the package manager".to_string(),
            });
            continue;
        }
        let package = distro.package_map.get(key).with_context(|| {
            format!(
                "distro '{}' has no package mapping for dependency '{key}'",
                distro.id
            )
        })?;
        let state = package_state(
            runner,
            package,
            requirement.min_version.as_deref(),
            requirement.max_version.as_deref(),
        )?;
        let status = match state.status {
            PackageStatus::Ok => CheckStatus::Ok,
            PackageStatus::Installable => CheckStatus::Installable,
            PackageStatus::Missing => CheckStatus::Missing,
            PackageStatus::VersionMismatch => CheckStatus::VersionMismatch,
        };
        report.entries.push(Check {
            scope: CheckScope::BuildPackage,
            component: component.id.clone(),
            key: key.clone(),
            package: Some(package.clone()),
            required: requirement.required,
            status,
            detail: state.detail(),
        });
        if state.status == PackageStatus::Ok {
            check_pkg_config(report, component, package, requirement, runner);
        }
    }

    for key in &component.runtime_dependencies {
        if distro.private_dependencies.contains(key) {
            report.entries.push(Check {
                scope: CheckScope::RuntimePackage,
                component: component.id.clone(),
                key: key.clone(),
                package: None,
                required: true,
                status: CheckStatus::Private,
                detail: "private dependency; not installed by the package manager".to_string(),
            });
            continue;
        }
        let package = distro.package_map.get(key).with_context(|| {
            format!(
                "distro '{}' has no package mapping for dependency '{key}'",
                distro.id
            )
        })?;
        let state = package_state(runner, package, None, None)?;
        let status = match state.status {
            PackageStatus::Ok => CheckStatus::Ok,
            PackageStatus::Installable => CheckStatus::Installable,
            PackageStatus::Missing => CheckStatus::Missing,
            PackageStatus::VersionMismatch => CheckStatus::VersionMismatch,
        };
        report.entries.push(Check {
            scope: CheckScope::RuntimePackage,
            component: component.id.clone(),
            key: key.clone(),
            package: Some(package.clone()),
            required: true,
            status,
            detail: state.detail(),
        });
    }

    Ok(())
}

fn check_pkg_config(
    report: &mut PreflightReport,
    component: &ComponentManifest,
    package: &str,
    requirement: &BuildRequirement,
    runner: &dyn Runner,
) {
    for module in &requirement.pkg_config {
        let (status, detail) = match runner.run("pkg-config", &["--modversion", module]) {
            Ok(output) if output.success => {
                let actual = output.stdout.trim();
                if actual.is_empty() {
                    (
                        CheckStatus::PkgConfigInvalid,
                        format!("{module} returned an empty version"),
                    )
                } else {
                    pkg_config_version_status(module, actual, requirement)
                }
            }
            Ok(output) => {
                let stderr = output.stderr.trim();
                (
                    CheckStatus::PkgConfigMissing,
                    if stderr.is_empty() {
                        format!("{module} is not available")
                    } else {
                        format!("{module}: {stderr}")
                    },
                )
            }
            Err(error) => (CheckStatus::PkgConfigMissing, error.to_string()),
        };
        report.entries.push(Check {
            scope: CheckScope::PkgConfig,
            component: component.id.clone(),
            key: module.clone(),
            package: Some(package.to_string()),
            required: requirement.required,
            status,
            detail,
        });
    }
}

fn pkg_config_version_status(
    module: &str,
    actual: &str,
    requirement: &BuildRequirement,
) -> (CheckStatus, String) {
    if let Some(min) = &requirement.min_version {
        match version::compare(actual, min) {
            Ok(Ordering::Less) => {
                return (
                    CheckStatus::PkgConfigVersion,
                    format!("{module} {actual} is below {min}"),
                );
            }
            Ok(_) => {}
            Err(error) => {
                return (
                    CheckStatus::PkgConfigInvalid,
                    format!("{module} {actual}: {error}"),
                );
            }
        }
    }
    if let Some(max) = &requirement.max_version {
        match version::compare(actual, max) {
            Ok(Ordering::Less) => {}
            Ok(_) => {
                return (
                    CheckStatus::PkgConfigVersion,
                    format!("{module} {actual} is not below {max}"),
                );
            }
            Err(error) => {
                return (
                    CheckStatus::PkgConfigInvalid,
                    format!("{module} {actual}: {error}"),
                );
            }
        }
    }
    (CheckStatus::Ok, format!("{module} {actual}"))
}

fn check_cxx(runner: &dyn Runner, standard: u8) -> (CheckStatus, String) {
    if !executable_on_path("c++") {
        return (
            CheckStatus::ToolMissing,
            "c++ not found in PATH".to_string(),
        );
    }
    let flag = format!("-std=c++{standard}");
    match runner.run_with_stdin(
        "c++",
        &[&flag, "-fsyntax-only", "-x", "c++", "-"],
        "int main() { return 0; }\n",
    ) {
        Ok(output) if output.success => (CheckStatus::Ok, format!("c++ supports {flag}")),
        Ok(output) => {
            let details = if output.stderr.trim().is_empty() {
                output.stdout.trim()
            } else {
                output.stderr.trim()
            };
            (
                CheckStatus::ToolchainUnsupported,
                format!("c++ rejected {flag}: {details}"),
            )
        }
        Err(error) => (
            CheckStatus::ToolchainUnsupported,
            format!("could not run c++: {error}"),
        ),
    }
}

fn tool_executable(tool: &str) -> &str {
    match tool {
        "cxx" => "c++",
        _ => tool,
    }
}

fn executable_on_path(name: &str) -> bool {
    let Some(path) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&path).any(|directory| is_executable(&directory.join(name)))
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PackageStatus {
    Ok,
    Installable,
    Missing,
    VersionMismatch,
}

#[derive(Debug, Clone)]
struct PackageState {
    installed: Option<String>,
    candidate: Option<String>,
    status: PackageStatus,
}

impl PackageState {
    fn detail(&self) -> String {
        let mut parts = Vec::new();
        if let Some(version) = &self.installed {
            parts.push(format!("installed {version}"));
        }
        if let Some(version) = &self.candidate {
            parts.push(format!("candidate {version}"));
        }
        if parts.is_empty() {
            "not found in installed packages or repositories".to_string()
        } else {
            parts.join(", ")
        }
    }
}

fn package_state(
    runner: &dyn Runner,
    package: &str,
    min_version: Option<&str>,
    max_version: Option<&str>,
) -> Result<PackageState> {
    let installed = installed_apt(runner, package)?;
    let candidate = candidate_apt(runner, package)?;
    let installed_ok = match installed.as_deref() {
        Some(version) => satisfies(runner, version, min_version, max_version)?,
        None => false,
    };
    let candidate_ok = match candidate.as_deref() {
        Some(version) => satisfies(runner, version, min_version, max_version)?,
        None => false,
    };
    let status = if installed_ok {
        PackageStatus::Ok
    } else if candidate_ok {
        PackageStatus::Installable
    } else if installed.is_some() || candidate.is_some() {
        PackageStatus::VersionMismatch
    } else {
        PackageStatus::Missing
    };
    Ok(PackageState {
        installed,
        candidate,
        status,
    })
}

fn satisfies(
    runner: &dyn Runner,
    version: &str,
    min_version: Option<&str>,
    max_version: Option<&str>,
) -> Result<bool> {
    if let Some(min) = min_version {
        if compare_dpkg(runner, version, min)? == Ordering::Less {
            return Ok(false);
        }
    }
    if let Some(max) = max_version {
        if compare_dpkg(runner, version, max)? != Ordering::Less {
            return Ok(false);
        }
    }
    Ok(true)
}

fn installed_apt(runner: &dyn Runner, package: &str) -> Result<Option<String>> {
    let output = runner.run("dpkg-query", &["-W", "-f=${Status}\t${Version}\n", package])?;
    if !output.success {
        return Ok(None);
    }
    for line in output.stdout.lines() {
        if let Some((status, version)) = line.split_once('\t') {
            if status.split_whitespace().last() == Some("installed") {
                let version = version.trim();
                if !version.is_empty() {
                    return Ok(Some(version.to_string()));
                }
            }
        }
    }
    Ok(None)
}

fn candidate_apt(runner: &dyn Runner, package: &str) -> Result<Option<String>> {
    let output = runner.run("apt-cache", &["policy", package])?;
    if !output.success {
        return Ok(None);
    }
    for line in output.stdout.lines() {
        if let Some(value) = line.trim().strip_prefix("Candidate:") {
            let value = value.trim();
            if value == "(none)" {
                return Ok(None);
            }
            if !value.is_empty() {
                return Ok(Some(value.to_string()));
            }
        }
    }
    Ok(None)
}

fn compare_dpkg(runner: &dyn Runner, left: &str, right: &str) -> Result<Ordering> {
    for (operator, ordering) in [("lt", Ordering::Less), ("eq", Ordering::Equal)] {
        let output = runner.run("dpkg", &["--compare-versions", left, operator, right])?;
        match output.code {
            Some(0) => return Ok(ordering),
            Some(1) => {}
            Some(code) => bail!("dpkg --compare-versions exited with {code}"),
            None => bail!("dpkg --compare-versions was terminated by a signal"),
        }
    }
    Ok(Ordering::Greater)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::CommandOutput;
    use anyhow::Result;

    struct FakeRunner<F>(F);

    impl<F> Runner for FakeRunner<F>
    where
        F: Fn(&str, &[&str]) -> Result<CommandOutput>,
    {
        fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput> {
            (self.0)(program, args)
        }

        fn run_with_stdin(
            &self,
            program: &str,
            args: &[&str],
            _input: &str,
        ) -> Result<CommandOutput> {
            (self.0)(program, args)
        }
    }

    fn output(success: bool, stdout: &str, stderr: &str) -> CommandOutput {
        CommandOutput {
            success,
            code: Some(if success { 0 } else { 1 }),
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
        }
    }

    fn check(scope: CheckScope, status: CheckStatus, required: bool) -> Check {
        Check {
            scope,
            component: "test".to_string(),
            key: "key".to_string(),
            package: None,
            required,
            status,
            detail: String::new(),
        }
    }

    #[test]
    fn apt_compare_uses_dpkg_ordering() {
        let runner = FakeRunner(|program: &str, args: &[&str]| {
            assert_eq!(program, "dpkg");
            let ordering = version::compare(args[1], args[3])?;
            let matches = match args[2] {
                "lt" => ordering == Ordering::Less,
                "eq" => ordering == Ordering::Equal,
                _ => false,
            };
            Ok(output(matches, "", ""))
        });
        assert_eq!(
            compare_dpkg(&runner, "1.2.3", "1.3").unwrap(),
            Ordering::Less
        );
        assert_eq!(
            compare_dpkg(&runner, "1.3", "1.3").unwrap(),
            Ordering::Equal
        );
        assert_eq!(
            compare_dpkg(&runner, "2.0", "1.3").unwrap(),
            Ordering::Greater
        );
    }

    #[test]
    fn package_state_distinguishes_missing_installed_and_upgradeable() {
        let runner = FakeRunner(|program: &str, args: &[&str]| match program {
            "dpkg-query" => Ok(output(true, "install ok installed\t1.2.3\n", "")),
            "apt-cache" => Ok(output(true, "  Candidate: 1.4.0\n", "")),
            "dpkg" => {
                let ordering = version::compare(args[1], args[3])?;
                let matches = match args[2] {
                    "lt" => ordering == Ordering::Less,
                    "eq" => ordering == Ordering::Equal,
                    _ => false,
                };
                Ok(output(matches, "", ""))
            }
            other => panic!("unexpected command {other}"),
        });

        let old = package_state(&runner, "libfoo", Some("1.3"), None).unwrap();
        assert_eq!(old.status, PackageStatus::Installable);
        assert!(old.detail().contains("installed 1.2.3"));

        let current = package_state(&runner, "libfoo", Some("1.0"), None).unwrap();
        assert_eq!(current.status, PackageStatus::Ok);

        let missing_runner = FakeRunner(|program: &str, _args: &[&str]| match program {
            "dpkg-query" | "apt-cache" => Ok(output(false, "", "not found")),
            other => panic!("unexpected command {other}"),
        });
        let missing = package_state(&missing_runner, "libmissing", None, None).unwrap();
        assert_eq!(missing.status, PackageStatus::Missing);
        assert!(missing.detail().contains("not found"));

        let installed_only_runner = FakeRunner(|program: &str, args: &[&str]| match program {
            "dpkg-query" => Ok(output(true, "install ok installed\t1.2.3\n", "")),
            "apt-cache" => Ok(output(true, "  Candidate: (none)\n", "")),
            "dpkg" => {
                let ordering = version::compare(args[1], args[3])?;
                Ok(output(ordering == Ordering::Less, "", ""))
            }
            other => panic!("unexpected command {other}"),
        });
        let installed_only =
            package_state(&installed_only_runner, "libfoo", Some("1.0"), None).unwrap();
        assert_eq!(installed_only.status, PackageStatus::Ok);
        assert!(installed_only.detail().contains("installed 1.2.3"));
    }

    #[test]
    fn pkg_config_version_range_is_inclusive_min_and_exclusive_max() {
        let requirement = BuildRequirement {
            pkg_config: vec!["libfoo".to_string()],
            min_version: Some("1.0".to_string()),
            max_version: Some("2.0".to_string()),
            required: true,
        };

        assert_eq!(
            pkg_config_version_status("libfoo", "1.0", &requirement).0,
            CheckStatus::Ok
        );
        assert_eq!(
            pkg_config_version_status("libfoo", "2.0", &requirement).0,
            CheckStatus::PkgConfigVersion
        );
        assert_eq!(
            pkg_config_version_status("libfoo", "0.9", &requirement).0,
            CheckStatus::PkgConfigVersion
        );
        assert_eq!(
            pkg_config_version_status("libfoo", "not-a-version", &requirement).0,
            CheckStatus::PkgConfigInvalid
        );
    }

    #[test]
    fn report_blocks_required_failures_but_not_installable_optional_or_private() {
        let mut report = PreflightReport::default();
        report.entries.push(check(
            CheckScope::BuildPackage,
            CheckStatus::Installable,
            true,
        ));
        report
            .entries
            .push(check(CheckScope::BuildPackage, CheckStatus::Private, true));
        report
            .entries
            .push(check(CheckScope::Tool, CheckStatus::ToolMissing, false));
        assert!(!report.has_blocking());

        report
            .entries
            .push(check(CheckScope::Tool, CheckStatus::ToolMissing, true));
        assert!(report.has_blocking());
    }
}
