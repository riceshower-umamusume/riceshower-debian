use crate::process::Runner;
use anyhow::{Context, Result};
use std::env;
use std::path::Path;

const ZSH: &str = "/usr/bin/zsh";

pub fn ensure_zsh(runner: &dyn Runner, dry_run: bool) -> Result<()> {
    if !Path::new(ZSH).exists() {
        println!("shell: zsh not installed; leaving the current shell unchanged");
        return Ok(());
    }
    let user = env::var("USER").context("USER is not set")?;
    if let Ok(output) = runner.run("getent", &["passwd", &user]) {
        if output.success {
            if let Some(shell) = output.stdout.trim().rsplit(':').next() {
                if shell.trim() == ZSH {
                    println!("shell: zsh is already the default for {user}");
                    return Ok(());
                }
            }
        }
    }
    if dry_run {
        println!("shell: would set {user}'s default shell to {ZSH}");
        return Ok(());
    }
    let output = runner.run("chsh", &["-s", ZSH, &user])?;
    if !output.success {
        anyhow::bail!("chsh -s {ZSH} {user} failed: {}", output.stderr.trim());
    }
    println!("shell: {user}'s default shell is now {ZSH} (log out/in, or run exec zsh)");
    Ok(())
}
