use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "noctalia-setup",
    version,
    about = "Unified installer and lifecycle manager for the Noctalia ecosystem"
)]
pub struct Cli {
    /// Directory containing ecosystem.toml, components/ and distros/
    #[arg(long, global = true, default_value = "manifest", value_name = "DIR")]
    pub manifest_dir: PathBuf,

    /// Print the plan without changing the system
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Assume yes for interactive prompts
    #[arg(long, short = 'y', global = true)]
    pub yes: bool,

    /// Directory containing the dotfiles manifest and files
    #[arg(long, global = true, default_value = "dotfiles", value_name = "DIR")]
    pub dotfiles_dir: PathBuf,

    /// Directory containing app catalog manifests
    #[arg(long, global = true, default_value = "apps", value_name = "DIR")]
    pub apps_dir: PathBuf,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Install selected components for the detected distribution
    Install {
        #[arg(long, value_delimiter = ',')]
        components: Vec<String>,
        #[arg(long)]
        no_greeter: bool,
        #[arg(long)]
        latest: bool,
    },
    /// Refresh pinned source commits; installed component deployment is not implemented
    Update {
        #[arg(long)]
        latest: bool,
    },
    /// Inspect the detected distro, manifest, lockfile and state
    Doctor,
    /// Reconcile the installation with the lockfile and state
    Repair,
    /// Remove files and services created by this installer
    Uninstall {
        #[arg(long)]
        purge: bool,
    },
    /// Update the noctalia-setup executable
    SelfUpdate,
    /// Build and stage one component from the lockfile
    Build {
        component: String,
        #[arg(long)]
        commit: Option<String>,
    },
    /// Apply user dotfiles and detected display configuration
    Config,
    /// Print detected GPU/output/mode information
    Hardware,
    /// List the app catalog
    ListApps,
    /// List components from the manifest
    ListComponents,
    /// Adopt an installation created by the legacy scripts
    Adopt,
}
