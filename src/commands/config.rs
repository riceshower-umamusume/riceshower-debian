use crate::dotfiles;
use crate::hardware;
use crate::process::SystemRunner;
use anyhow::{Context, Result};
use std::path::Path;

pub(super) fn config(dotfiles_dir: &Path, dry_run: bool) -> Result<()> {
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    let home = Path::new(&home);
    let detected = hardware::detect(&SystemRunner)?;
    println!("hardware: gpu={}", detected.gpu);
    for output in &detected.outputs {
        if let Some(mode) = output.best_mode() {
            println!("  {}: {}", output.connector, mode.label());
        }
    }
    println!("dotfiles:");
    dotfiles::apply(dotfiles_dir, home, dry_run)?;
    let display = hardware::display_toml(&detected);
    if display.is_empty() {
        println!("  display: no running Umbriel output detected");
    } else {
        dotfiles::write_display(home, &display, dry_run)?;
    }
    Ok(())
}

pub(super) fn hardware_command() -> Result<()> {
    let detected = hardware::detect(&SystemRunner)?;
    println!("gpu: {}", detected.gpu);
    if detected.outputs.is_empty() {
        println!("outputs: unavailable (is Umbriel running?)");
    }
    for output in &detected.outputs {
        match output.best_mode() {
            Some(mode) => println!("  {}: {}", output.connector, mode.label()),
            None => println!("  {}: no modes", output.connector),
        }
    }
    let display = hardware::display_toml(&detected);
    if !display.is_empty() {
        println!("\nsuggested display config:\n{display}");
    }
    Ok(())
}
