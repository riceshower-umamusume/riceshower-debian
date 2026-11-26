use crate::manifest::ComponentManifest;
use crate::process::Runner;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn resolve_commit(repository: &str, git_ref: &str) -> Result<String> {
    validate_github_repository(repository)?;
    validate_branch_name(git_ref)?;

    let expected_ref = format!("refs/heads/{git_ref}");
    let output = Command::new("git")
        .args([
            "ls-remote",
            "--exit-code",
            "--heads",
            repository,
            &expected_ref,
        ])
        .output()
        .with_context(|| format!("starting git ls-remote for {repository}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let details = stderr.trim();
        if details.is_empty() {
            bail!("remote branch '{git_ref}' was not found in {repository}");
        }
        bail!("could not resolve '{git_ref}' in {repository}: {details}");
    }

    let stdout =
        String::from_utf8(output.stdout).context("git ls-remote returned non-UTF-8 output")?;
    parse_ls_remote(&stdout, &expected_ref)
}

fn validate_github_repository(repository: &str) -> Result<()> {
    let path = repository
        .strip_prefix("https://github.com/")
        .context("component repository must use an https://github.com/ URL")?;
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut parts = path.split('/');
    let owner = parts.next().unwrap_or_default();
    let name = parts.next().unwrap_or_default();
    let valid_segment = |value: &str| {
        !value.is_empty()
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' || byte == b'.'
            })
            && value != "."
            && value != ".."
    };

    if !valid_segment(owner) || !valid_segment(name) || parts.next().is_some() {
        bail!("invalid GitHub repository URL: {repository}");
    }
    Ok(())
}

fn validate_branch_name(git_ref: &str) -> Result<()> {
    if git_ref.is_empty()
        || git_ref.starts_with('/')
        || git_ref.ends_with('/')
        || git_ref.contains("//")
        || git_ref.contains("..")
        || git_ref.contains("@{")
        || git_ref.ends_with('.')
        || git_ref.bytes().any(|byte| {
            !(byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/'))
        })
    {
        bail!("invalid branch name: {git_ref}");
    }
    Ok(())
}

fn parse_ls_remote(output: &str, expected_ref: &str) -> Result<String> {
    let mut resolved = None;
    for line in output.lines().filter(|line| !line.is_empty()) {
        let (commit, reference) = line
            .split_once('\t')
            .with_context(|| format!("malformed git ls-remote line: {line}"))?;
        if reference != expected_ref {
            continue;
        }
        if resolved.is_some() {
            bail!("git ls-remote returned more than one result for {expected_ref}");
        }
        if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            bail!("git ls-remote returned an invalid commit SHA for {expected_ref}");
        }
        resolved = Some(commit.to_ascii_lowercase());
    }

    resolved.with_context(|| format!("git ls-remote returned no result for {expected_ref}"))
}

pub fn acquire_component(
    component: &ComponentManifest,
    commit: &str,
    runner: &dyn Runner,
) -> Result<PathBuf> {
    validate_github_repository(&component.repo)?;
    fetch_snapshot(
        runner,
        &component.repo,
        &component.id,
        commit,
        &cache_root()?,
    )
}

pub fn cache_root() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("XDG_CACHE_HOME") {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            return Ok(path.join("noctalia-setup"));
        }
    }
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join(".cache/noctalia-setup"))
}

fn fetch_snapshot(
    runner: &dyn Runner,
    repository: &str,
    component_id: &str,
    commit: &str,
    cache_root: &Path,
) -> Result<PathBuf> {
    validate_cache_segment(component_id)?;
    validate_commit(commit)?;

    let component_dir = cache_root.join("sources").join(component_id);
    let destination = component_dir.join(commit);
    if destination.exists() {
        verify_checkout(runner, &destination, commit).with_context(|| {
            format!(
                "cached source at {} cannot be reused; remove it manually",
                destination.display()
            )
        })?;
        return Ok(destination);
    }

    fs::create_dir_all(&component_dir)
        .with_context(|| format!("creating source cache {}", component_dir.display()))?;
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary =
        component_dir.join(format!(".{commit}.{}.{}.tmp", std::process::id(), sequence));
    if temporary.exists() {
        fs::remove_dir_all(&temporary)
            .with_context(|| format!("removing stale {}", temporary.display()))?;
    }

    if let Err(error) = clone_commit(runner, repository, commit, &temporary) {
        let _ = fs::remove_dir_all(&temporary);
        return Err(error);
    }

    if destination.exists() {
        let _ = fs::remove_dir_all(&temporary);
        verify_checkout(runner, &destination, commit)?;
        return Ok(destination);
    }

    fs::rename(&temporary, &destination).with_context(|| {
        format!(
            "moving source cache {} to {}",
            temporary.display(),
            destination.display()
        )
    })?;
    verify_checkout(runner, &destination, commit)?;
    Ok(destination)
}

fn clone_commit(
    runner: &dyn Runner,
    repository: &str,
    commit: &str,
    destination: &Path,
) -> Result<()> {
    fs::create_dir_all(destination)
        .with_context(|| format!("creating {}", destination.display()))?;
    let path = destination
        .to_str()
        .context("source cache path is not valid UTF-8")?;

    run_git(
        runner,
        &["init", "--quiet", path],
        Duration::from_secs(60),
        "git init",
    )?;
    run_git(
        runner,
        &["-C", path, "remote", "add", "origin", repository],
        Duration::from_secs(30),
        "git remote add",
    )?;
    run_git(
        runner,
        &[
            "-C", path, "fetch", "--quiet", "--depth", "1", "origin", commit,
        ],
        Duration::from_secs(120),
        "git fetch",
    )?;
    run_git(
        runner,
        &["-C", path, "checkout", "--quiet", "--detach", commit],
        Duration::from_secs(60),
        "git checkout",
    )?;
    verify_checkout(runner, destination, commit)?;

    if destination.join(".gitmodules").exists() {
        bail!("component source declares submodules; submodules are not allowed by policy");
    }
    if let Ok(attributes) = fs::read_to_string(destination.join(".gitattributes")) {
        if attributes.lines().any(|line| line.contains("filter=lfs")) {
            bail!("component source uses git-lfs; git-lfs is not allowed by policy");
        }
    }
    Ok(())
}

fn run_git(runner: &dyn Runner, args: &[&str], timeout: Duration, action: &str) -> Result<()> {
    let output = runner.run_with_timeout("git", args, timeout)?;
    if !output.success {
        let details = output.stderr.trim();
        if details.is_empty() {
            bail!("{action} failed");
        }
        bail!("{action} failed: {details}");
    }
    Ok(())
}

fn verify_checkout(runner: &dyn Runner, path: &Path, commit: &str) -> Result<()> {
    let path = path
        .to_str()
        .context("source cache path is not valid UTF-8")?;
    let head = runner.run_with_timeout(
        "git",
        &["-C", path, "rev-parse", "HEAD"],
        Duration::from_secs(30),
    )?;
    if !head.success {
        bail!("could not read HEAD for {path}: {}", head.stderr.trim());
    }
    if !head.stdout.trim().eq_ignore_ascii_case(commit) {
        bail!(
            "source cache HEAD {} does not match locked commit {commit}",
            head.stdout.trim()
        );
    }

    let status = runner.run_with_timeout(
        "git",
        &["-C", path, "status", "--porcelain"],
        Duration::from_secs(30),
    )?;
    if !status.success {
        bail!(
            "could not inspect source cache {path}: {}",
            status.stderr.trim()
        );
    }
    if !status.stdout.trim().is_empty() {
        bail!("source cache {path} has local changes");
    }
    Ok(())
}

fn validate_commit(commit: &str) -> Result<()> {
    if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("invalid commit SHA '{commit}'");
    }
    Ok(())
}

fn validate_cache_segment(component_id: &str) -> Result<()> {
    if component_id.is_empty()
        || !component_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        bail!("invalid component id '{component_id}' for source cache");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_the_requested_remote_branch() {
        let output = concat!(
            "a909081a934a80bb7685080884ffaaa30c857e4c\trefs/heads/main\n",
            "2dfc0b5b6343165dc77dcd378717b4d372f791a5\trefs/heads/develop\n",
        );
        assert_eq!(
            parse_ls_remote(output, "refs/heads/main").unwrap(),
            "a909081a934a80bb7685080884ffaaa30c857e4c"
        );
    }

    #[test]
    fn rejects_missing_or_malformed_remote_results() {
        assert!(parse_ls_remote("", "refs/heads/main").is_err());
        assert!(parse_ls_remote("not-a-sha\trefs/heads/main\n", "refs/heads/main").is_err());
    }

    #[test]
    fn only_accepts_public_github_repository_urls() {
        assert!(validate_github_repository("https://github.com/noctalia-dev/umbriel").is_ok());
        assert!(validate_github_repository("https://github.com/noctalia-dev/umbriel.git").is_ok());
        assert!(validate_github_repository("file:///tmp/umbriel").is_err());
        assert!(validate_github_repository("https://example.com/noctalia-dev/umbriel").is_err());
        assert!(
            validate_github_repository("https://github.com/noctalia-dev/umbriel/extra").is_err()
        );
    }

    #[test]
    fn rejects_unsafe_branch_names() {
        assert!(validate_branch_name("main").is_ok());
        assert!(validate_branch_name("release/1.x").is_ok());
        assert!(validate_branch_name("../main").is_err());
        assert!(validate_branch_name("-upload-pack=evil").is_err());
        assert!(validate_branch_name("main with spaces").is_err());
    }

    use crate::process::SystemRunner;

    fn git(directory: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(directory)
            .args(args)
            .output()
            .expect("starting git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn git_stdout(directory: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(directory)
            .args(args)
            .output()
            .expect("starting git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    fn fixture(submodules: bool) -> (tempfile::TempDir, PathBuf, String) {
        let directory = tempfile::tempdir().unwrap();
        let remote = directory.path().join("remote.git");
        let work = directory.path().join("work");
        fs::create_dir_all(&work).unwrap();

        let output = Command::new("git")
            .args(["init", "--bare"])
            .arg(&remote)
            .output()
            .expect("starting git init --bare");
        assert!(output.status.success());
        git(
            &remote,
            &["config", "uploadpack.allowAnySHA1InWant", "true"],
        );
        git(&work, &["init"]);
        fs::write(work.join("file.txt"), "one").unwrap();
        if submodules {
            fs::write(
                work.join(".gitmodules"),
                "[submodule \"x\"]\n\tpath = x\n\turl = https://example.com/x\n",
            )
            .unwrap();
        }
        git(&work, &["add", "."]);
        git(
            &work,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-m",
                "initial",
            ],
        );
        let commit = git_stdout(&work, &["rev-parse", "HEAD"]).trim().to_string();
        git(
            &work,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        git(&work, &["push", "origin", "HEAD:refs/heads/main"]);
        (directory, remote, commit)
    }

    #[test]
    fn fetches_exact_commit_and_reuses_cache() {
        let (directory, remote, commit) = fixture(false);
        let cache = directory.path().join("cache");
        let runner = SystemRunner;

        let checkout = fetch_snapshot(
            &runner,
            remote.to_str().unwrap(),
            "umbriel",
            &commit,
            &cache,
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(checkout.join("file.txt")).unwrap(),
            "one"
        );
        assert_eq!(git_stdout(&checkout, &["rev-parse", "HEAD"]).trim(), commit);

        let reused = fetch_snapshot(
            &runner,
            remote.to_str().unwrap(),
            "umbriel",
            &commit,
            &cache,
        )
        .unwrap();
        assert_eq!(checkout, reused);
    }

    #[test]
    fn rejects_dirty_cached_checkout() {
        let (directory, remote, commit) = fixture(false);
        let cache = directory.path().join("cache");
        let runner = SystemRunner;
        let checkout = fetch_snapshot(
            &runner,
            remote.to_str().unwrap(),
            "umbriel",
            &commit,
            &cache,
        )
        .unwrap();

        fs::write(checkout.join("dirty.txt"), "changed").unwrap();
        assert!(fetch_snapshot(
            &runner,
            remote.to_str().unwrap(),
            "umbriel",
            &commit,
            &cache,
        )
        .is_err());
    }

    #[test]
    fn upstream_branch_move_does_not_change_cached_source() {
        let (directory, remote, commit) = fixture(false);
        let cache = directory.path().join("cache");
        let runner = SystemRunner;
        let checkout = fetch_snapshot(
            &runner,
            remote.to_str().unwrap(),
            "umbriel",
            &commit,
            &cache,
        )
        .unwrap();

        let work = directory.path().join("work");
        fs::write(work.join("second.txt"), "two").unwrap();
        git(&work, &["add", "."]);
        git(
            &work,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-m",
                "second",
            ],
        );
        git(&work, &["push", "origin", "HEAD:refs/heads/main"]);

        let reused = fetch_snapshot(
            &runner,
            remote.to_str().unwrap(),
            "umbriel",
            &commit,
            &cache,
        )
        .unwrap();
        assert_eq!(checkout, reused);
        assert_eq!(fs::read_to_string(reused.join("file.txt")).unwrap(), "one");
        assert!(!reused.join("second.txt").exists());
    }

    #[test]
    fn missing_commit_leaves_no_destination_or_temp() {
        let (directory, remote, _) = fixture(false);
        let cache = directory.path().join("cache");
        let runner = SystemRunner;
        let missing = "0".repeat(40);

        assert!(fetch_snapshot(
            &runner,
            remote.to_str().unwrap(),
            "umbriel",
            &missing,
            &cache,
        )
        .is_err());
        let component = cache.join("sources/umbriel");
        assert!(!component.join(&missing).exists());
        if component.exists() {
            assert_eq!(fs::read_dir(component).unwrap().count(), 0);
        }
    }

    #[test]
    fn rejects_repository_with_submodules() {
        let (directory, remote, commit) = fixture(true);
        let cache = directory.path().join("cache");
        let runner = SystemRunner;

        let error = fetch_snapshot(
            &runner,
            remote.to_str().unwrap(),
            "umbriel",
            &commit,
            &cache,
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("submodules"));
        assert!(!cache.join("sources/umbriel").join(&commit).exists());
    }
}
