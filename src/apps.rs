use anyhow::{Context, Result};
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
    pub notes: Option<String>,
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
        apps.push(app);
    }
    apps.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(apps)
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
