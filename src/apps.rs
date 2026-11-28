use crate::process::Runner;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct AppManifest {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub repo: String,
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub apt_source: Option<String>,
    #[serde(default)]
    pub apt_key_file: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

fn validate_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id
            .bytes()
            .any(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')))
        || id == "."
        || id == ".."
    {
        bail!("invalid app id '{id}'");
    }
    Ok(())
}

pub fn load(directory: &Path) -> Result<Vec<AppManifest>> {
    let mut apps = Vec::new();
    let entries = fs::read_dir(directory)
        .with_context(|| format!("reading app catalog {}", directory.display()))?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("toml") {
            continue;
        }
        let content =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let app: AppManifest =
            toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;
        validate_id(&app.id)?;
        if let Some(key) = &app.apt_key_file {
            let key = Path::new(key);
            if key.is_absolute()
                || key
                    .components()
                    .any(|part| !matches!(part, std::path::Component::Normal(_)))
            {
                bail!("app '{}' has an unsafe key path", app.id);
            }
        }
        apps.push(app);
    }
    apps.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(apps)
}

pub fn install(directory: &Path, id: &str, dry_run: bool, runner: &dyn Runner) -> Result<()> {
    let app = load(directory)?
        .into_iter()
        .find(|app| app.id == id)
        .with_context(|| format!("unknown app '{id}'"))?;
    if app.kind != "apt" {
        bail!("app '{}' uses unsupported kind '{}'", app.id, app.kind);
    }
    if app.packages.is_empty() {
        bail!("app '{}' has no packages", app.id);
    }
    println!("app: {} ({})", app.id, app.name);
    println!("  repository: {}", app.repo);
    println!("  packages: {}", app.packages.join(", "));
    if dry_run {
        println!(
            "dry-run: sudo apt-get install -y --no-install-recommends {}",
            app.packages.join(" ")
        );
        return Ok(());
    }
    if let Some(source) = &app.apt_source {
        let output = runner.run(
            "sudo",
            &[
                "--",
                "apt-get",
                "install",
                "-y",
                "--no-install-recommends",
                "gpg",
                "curl",
                "ca-certificates",
            ],
        )?;
        if !output.success {
            bail!(
                "installing apt key prerequisites failed: {}",
                output.stderr.trim()
            );
        }
        let list = format!("/etc/apt/sources.list.d/noctalia-{}.list", app.id);
        let output =
            runner.run_with_stdin("sudo", &["--", "tee", &list], &format!("{source}\n"))?;
        if !output.success {
            bail!("writing {list} failed: {}", output.stderr.trim());
        }
        if let Some(key_file) = &app.apt_key_file {
            let source = directory.join(key_file);
            let destination = format!("/etc/apt/keyrings/{}.gpg", app.id);
            let output = runner.run(
                "sudo",
                &[
                    "--",
                    "install",
                    "-m",
                    "0644",
                    &source.display().to_string(),
                    &destination,
                ],
            )?;
            if !output.success {
                bail!("installing app key failed: {}", output.stderr.trim());
            }
        }
        let output = runner.run("sudo", &["--", "apt-get", "update"])?;
        if !output.success {
            bail!("apt-get update failed: {}", output.stderr.trim());
        }
    }
    let mut args = vec!["--", "apt-get", "install", "-y", "--no-install-recommends"];
    args.extend(app.packages.iter().map(String::as_str));
    let output = runner.run("sudo", &args)?;
    if !output.success {
        let details = if output.stderr.trim().is_empty() {
            output.stdout.trim()
        } else {
            output.stderr.trim()
        };
        bail!("apt-get failed for '{}': {details}", app.id);
    }
    println!("installed: {}", app.id);
    Ok(())
}

pub fn print(directory: &Path) -> Result<()> {
    for app in load(directory)? {
        println!(
            "{:<28} {:<10} {:<24} {}",
            app.id, app.kind, app.name, app.repo
        );
        if !app.packages.is_empty() {
            println!("  packages: {}", app.packages.join(", "));
        }
        if let Some(notes) = &app.notes {
            println!("  notes: {notes}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::CommandOutput;
    use std::fs;

    struct FakeSudo;

    impl Runner for FakeSudo {
        fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput> {
            assert_eq!(program, "sudo");
            assert_eq!(args[0], "--");
            assert_eq!(args[1], "apt-get");
            assert!(args.contains(&"fcitx5-lotus"));
            Ok(CommandOutput {
                success: true,
                code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            })
        }

        fn run_with_stdin(
            &self,
            _program: &str,
            _args: &[&str],
            _input: &str,
        ) -> Result<CommandOutput> {
            bail!("stdin is not used")
        }
    }

    #[test]
    fn install_app_uses_sudo_apt() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("lotus.toml"),
            "id = \"fcitx5-lotus\"\nname = \"Lotus\"\nkind = \"apt\"\nrepo = \"https://example.invalid\"\npackages = [\"fcitx5-lotus\"]\n",
        )
        .unwrap();
        install(directory.path(), "fcitx5-lotus", false, &FakeSudo).unwrap();
    }
}
