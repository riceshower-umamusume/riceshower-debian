use crate::process::Runner;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct AppManifest {
    pub id: String,
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub packages: Vec<String>,
    #[serde(default)]
    pub packages_by_arch: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub apt_source: Option<String>,
    #[serde(default)]
    pub apt_key_file: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub git_ref: Option<String>,
    #[serde(default)]
    pub git_commit: Option<String>,
    #[serde(default)]
    pub build_command: Vec<String>,
    #[serde(default)]
    pub install_command: Vec<String>,
    #[serde(default)]
    pub cargo_package: Option<String>,
    #[serde(default)]
    pub cargo_bin: Option<String>,
    #[serde(default)]
    pub flatpak_ref: Option<String>,
    #[serde(default)]
    pub flatpak_remote: Option<String>,
    #[serde(default)]
    pub appimage_url: Option<String>,
    #[serde(default)]
    pub appimage_sha256: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AppState {
    schema: u32,
    #[serde(default)]
    architecture: String,
    #[serde(default)]
    apps: BTreeMap<String, InstalledApp>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstalledApp {
    kind: String,
    version: String,
    reference: String,
    architecture: String,
    installed_at: String,
}

fn validate_id(id: &str) -> Result<()> {
    if id.is_empty() || id.bytes().any(|b| !(b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))) || id == "." || id == ".." {
        bail!("invalid app id '{id}'");
    }
    Ok(())
}
fn output_ok(out: crate::process::CommandOutput, context: &str) -> Result<String> {
    if !out.success {
        let detail = if out.stderr.trim().is_empty() { out.stdout.trim() } else { out.stderr.trim() };
        bail!("{context}: {detail}");
    }
    Ok(out.stdout)
}
fn run(runner: &dyn Runner, program: &str, args: &[&str], context: &str) -> Result<String> {
    output_ok(runner.run(program, args)?, context)
}
fn architecture(runner: &dyn Runner) -> Result<String> {
    let arch = run(runner, "dpkg", &["--print-architecture"], "detecting Debian architecture")?;
    let arch = arch.trim();
    if arch.is_empty() || !arch.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        bail!("dpkg returned invalid architecture '{arch}'");
    }
    Ok(arch.to_string())
}
fn home_state_path() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join(".local/state/noctalia-setup/apps.toml"))
}
fn save_state(path: &Path, state: &AppState) -> Result<()> {
    if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
    let content = toml::to_string_pretty(state).context("serializing app state")?;
    crate::atomic_file::write(path, content.as_bytes())
}
fn load_state(path: &Path) -> Result<AppState> {
    if !path.exists() { return Ok(AppState { schema: 1, ..AppState::default() }); }
    let content = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))
}
pub fn load(directory: &Path) -> Result<Vec<AppManifest>> {
    let mut apps = Vec::new();
    let mut seen = BTreeSet::new();
    for entry in fs::read_dir(directory).with_context(|| format!("reading app catalog {}", directory.display()))? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") { continue; }
        let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let app: AppManifest = toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        validate_id(&app.id)?;
        if !seen.insert(app.id.clone()) { bail!("duplicate app id '{}'", app.id); }
        if let Some(key) = &app.apt_key_file {
            let key = Path::new(key);
            if key.is_absolute() || key.components().any(|p| !matches!(p, std::path::Component::Normal(_))) {
                bail!("app '{}' has an unsafe key path", app.id);
            }
        }
        if app.kind == "appimage" && app.appimage_sha256.as_deref().map_or(true, |s| s.len()!=64 || !s.bytes().all(|b| b.is_ascii_hexdigit())) {
            bail!("app '{}' requires a valid 64-character appimage_sha256", app.id);
        }
        if app.kind == "source" && (app.source_url.is_none() || app.git_commit.as_deref().map_or(true, |s| s.len()!=40 || !s.bytes().all(|b| b.is_ascii_hexdigit())) || app.build_command.is_empty() || app.install_command.is_empty()) {
            bail!("source app '{}' requires source_url and a full 40-character git_commit pin", app.id);
        }
        apps.push(app);
    }
    apps.sort_by(|a,b| a.id.cmp(&b.id));
    Ok(apps)
}
fn install_apt(directory: &Path, app: &AppManifest, arch: &str, dry_run: bool, runner: &dyn Runner) -> Result<String> {
    let packages = app.packages_by_arch.get(arch).unwrap_or(&app.packages);
    if packages.is_empty() { bail!("app '{}' has no package for architecture {arch}", app.id); }
    if dry_run {
        println!("dry-run: apt install {}; architecture={arch}", packages.join(" "));
        if app.apt_source.is_some() { println!("dry-run: configure APT source and key for {}", app.id); }
        return Ok("apt-managed".into());
    }
    if let Some(source) = &app.apt_source {
        run(runner, "sudo", &["--","apt-get","install","-y","gpg","ca-certificates"], "installing APT prerequisites")?;
        let list = format!("/etc/apt/sources.list.d/noctalia-{}.list", app.id);
        let source = source.replace("arch=amd64", &format!("arch={arch}"));
        let out = runner.run_with_stdin("sudo", &["--","tee",&list], &format!("{source}\n"))?;
        output_ok(out, "writing APT source")?;
        if let Some(key_file) = &app.apt_key_file {
            let src = directory.join(key_file);
            if !src.is_file() { bail!("APT key file does not exist: {}", src.display()); }
            let dst = format!("/etc/apt/keyrings/{}.gpg", app.id);
            run(runner,"sudo",&["--","install","-d","-m","0755","/etc/apt/keyrings"],"creating APT keyring directory")?;
            run(runner,"sudo",&["--","install","-m","0644",src.to_str().context("non-UTF8 key path")?,&dst],"installing APT key")?;
        }
        run(runner,"sudo",&["--","apt-get","update"],"refreshing APT indexes")?;
    }
    let mut args = vec!["--","apt-get","install","-y","--no-install-recommends"];
    args.extend(packages.iter().map(String::as_str));
    run(runner,"sudo",&args,"installing APT application")?;
    let mut query = vec!["-W", "-f=${binary:Package}=${Version}"];
    query.extend(packages.iter().map(String::as_str));
    let versions = run(runner,"dpkg-query",&query,"recording installed APT package versions")?;
    Ok(versions.trim().replace('\n', ", "))
}
fn install_cargo(app: &AppManifest, arch: &str, dry_run: bool, runner: &dyn Runner) -> Result<String> {
    let url = app.source_url.as_deref().context("cargo app requires source_url")?;
    let rev = app.git_commit.as_deref().or(app.git_ref.as_deref()).context("cargo app requires pinned git_commit or git_ref")?;
    if dry_run { println!("dry-run: cargo install --git {url} --rev {rev}"); return Ok(rev.into()); }
    let mut args = vec!["install","--git",url,"--rev",rev,"--locked","--root"];
    let root = format!("{}/.local/opt/noctalia-apps/{}", std::env::var("HOME")?, app.id);
    args.push(&root);
    if let Some(package)=&app.cargo_package { args.extend(["--package",package]); }
    if let Some(bin)=&app.cargo_bin { args.extend(["--bin",bin]); }
    run(runner,"cargo",&args,"installing Cargo application")?;
    Ok(rev.into())
}
fn install_flatpak(app: &AppManifest, dry_run: bool, runner: &dyn Runner) -> Result<String> {
    let reference = app.flatpak_ref.as_deref().context("flatpak app requires flatpak_ref")?;
    let remote = app.flatpak_remote.as_deref().unwrap_or("flathub");
    if dry_run { println!("dry-run: flatpak install --user {remote} {reference}"); return Ok(reference.into()); }
    run(runner,"flatpak",&["remote-add","--user","--if-not-exists",remote,"https://dl.flathub.org/repo/flathub.flatpakrepo"],"configuring Flatpak remote")?;
    run(runner,"flatpak",&["install","--user","-y",remote,reference],"installing Flatpak application")?;
    Ok(reference.into())
}
fn install_appimage(app: &AppManifest, arch: &str, dry_run: bool, runner: &dyn Runner) -> Result<String> {
    let url = app.appimage_url.as_deref().context("appimage app requires appimage_url")?;
    let sha = app.appimage_sha256.as_deref().context("appimage app requires appimage_sha256")?;
    if dry_run { println!("dry-run: download {url}; verify SHA256 {sha}; install to ~/.local/opt/noctalia-apps/{}.AppImage",app.id); return Ok(sha.into()); }
    let root = PathBuf::from(std::env::var_os("HOME").context("HOME is not set")?).join(".local/opt/noctalia-apps");
    fs::create_dir_all(&root)?;
    let dest = root.join(format!("{}.AppImage",app.id));
    run(runner,"curl",&["-fL","--retry","3",url,"-o",dest.to_str().context("invalid AppImage path")?],"downloading AppImage")?;
    let check = runner.run_with_stdin("sha256sum",&["-c","-"],&format!("{sha}  {}\n",dest.display()))?;
    output_ok(check,"verifying AppImage SHA256")?;
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dest,fs::Permissions::from_mode(0o755))?;
    }
    let _ = arch;
    Ok(sha.into())
}
fn install_source(app: &AppManifest, dry_run: bool, runner: &dyn Runner) -> Result<String> {
    let url=app.source_url.as_deref().context("source app requires source_url")?;
    let commit=app.git_commit.as_deref().context("source app requires git_commit pin")?;
    let root=PathBuf::from(std::env::var_os("HOME").context("HOME is not set")?).join(".cache/noctalia-setup/apps").join(&app.id);
    if dry_run { println!("dry-run: git clone {url}; checkout {commit}; execute pinned build/install recipes"); return Ok(commit.into()); }
    if root.exists() { fs::remove_dir_all(&root).with_context(||format!("removing stale source {}",root.display()))?; }
    if let Some(parent)=root.parent(){fs::create_dir_all(parent)?;}
    run(runner,"git",&["clone","--no-checkout",url,root.to_str().context("invalid source path")?],"cloning source app")?;
    run(runner,"git",&["-C",root.to_str().unwrap(),"checkout","--detach",commit],"checking out pinned source commit")?;
    let actual=run(runner,"git",&["-C",root.to_str().unwrap(),"rev-parse","HEAD"],"verifying source commit")?;
    if actual.trim()!=commit { bail!("source checkout mismatch: expected {commit}, got {}",actual.trim()); }
    for command in [&app.build_command,&app.install_command] {
        for recipe in command {
            let script=recipe.replace("{source}",root.to_str().unwrap());
            run(runner,"sh",&["-eu","-c",&script],"running source build/install recipe")?;
        }
    }
    Ok(commit.into())
}
pub fn install(directory: &Path, id: &str, dry_run: bool, runner: &dyn Runner) -> Result<()> {
    let app=load(directory)?.into_iter().find(|a|a.id==id).with_context(||format!("unknown app '{id}'"))?;
    let arch=architecture(runner)?;
    println!("app: {} ({})",app.id,app.name);
    println!("  backend: {}; architecture: {}",app.kind,arch);
    let version=match app.kind.as_str() {
        "apt"=>install_apt(directory,&app,&arch,dry_run,runner)?,
        "cargo"=>install_cargo(&app,&arch,dry_run,runner)?,
        "flatpak"=>install_flatpak(&app,dry_run,runner)?,
        "appimage"=>install_appimage(&app,&arch,dry_run,runner)?,
        "source"=>install_source(&app,dry_run,runner)?,
        other=>bail!("app '{}' uses unsupported kind '{other}'",app.id),
    };
    if !dry_run {
        let path=home_state_path()?;
        let mut state=load_state(&path)?;
        state.schema=1;
        state.architecture=arch.clone();
        state.apps.insert(app.id.clone(),InstalledApp {
            kind:app.kind,version:version.clone(),reference:app.git_commit.or(app.git_ref).unwrap_or(version),
            architecture:arch,installed_at:format!("{:?}",std::time::SystemTime::now()),
        });
        save_state(&path,&state)?;
        println!("installed: {}; version/reference: {}",app.id,version);
        println!("state: {}",path.display());
    }
    Ok(())
}
pub fn print(directory: &Path) -> Result<()> {
    for app in load(directory)? {
        println!("{:<28} {:<10} {}",app.id,app.kind,app.name);
        if !app.repo.is_empty(){println!("  repository: {}",app.repo);}
        if !app.packages.is_empty(){println!("  packages: {}",app.packages.join(", "));}
        if let Some(notes)=&app.notes{println!("  notes: {notes}");}
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_rejects_unpinned_source_and_unverified_appimage() {
        let dir=tempfile::tempdir().unwrap();
        fs::write(dir.path().join("bad.toml"),"id='bad'\nname='Bad'\nkind='source'\nsource_url='https://example.invalid/repo.git'\n").unwrap();
        assert!(load(dir.path()).is_err());
        fs::write(dir.path().join("bad.toml"),"id='bad'\nname='Bad'\nkind='appimage'\nappimage_url='https://example.invalid/app.AppImage'\n").unwrap();
        assert!(load(dir.path()).is_err());
    }
    #[test]
    fn app_manifest_loads_legacy_apt_entries() {
        let dir=tempfile::tempdir().unwrap();
        fs::write(dir.path().join("apt.toml"),"id='apt-app'\nname='APT app'\nkind='apt'\nrepo='https://example.invalid'\npackages=['pkg']\n").unwrap();
        assert_eq!(load(dir.path()).unwrap()[0].kind,"apt");
    }
}
