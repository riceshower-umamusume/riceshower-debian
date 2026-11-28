use anyhow::{Context, Result};
use std::path::PathBuf;

pub fn home() -> Result<PathBuf> {
    let value = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(value))
}

pub fn state_home() -> Result<PathBuf> {
    if let Some(value) = std::env::var_os("XDG_STATE_HOME") {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            return Ok(path);
        }
    }
    Ok(home()?.join(".local/state"))
}

pub fn cache_home() -> Result<PathBuf> {
    if let Some(value) = std::env::var_os("XDG_CACHE_HOME") {
        let path = PathBuf::from(value);
        if path.is_absolute() {
            return Ok(path);
        }
    }
    Ok(home()?.join(".cache"))
}
