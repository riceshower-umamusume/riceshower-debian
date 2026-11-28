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

    /// Directory containing install profiles
    #[arg(long, global = true, default_value = "profiles", value_name = "DIR")]
    pub profiles_dir: PathBuf,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Install selected components for the detected distribution
    Install {
        #[arg(long, value_delimiter = ',')]
        components: Vec<String>,
        #[arg(long)]
        greeter: bool,
        #[arg(long)]
        latest: bool,
        /// Install profiles (default: all)
        #[arg(long = "profile", value_delimiter = ',')]
        profiles: Vec<String>,
    },
    /// Refresh pinned source commits; installed component deployment is not implemented
    Update {
        #[arg(long)]
        latest: bool,
        /// Install profiles (default: all)
        #[arg(long = "profile", value_delimiter = ',')]
        profiles: Vec<String>,
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
    /// Build and stage one component or source tool
    Build {
        component: Option<String>,
        #[arg(long)]
        tool: Option<String>,
        #[arg(long)]
        commit: Option<String>,
    },
    /// Apply user dotfiles and detected display configuration
    Config,
    /// Print detected GPU/output/mode information
    Hardware,
    /// List the app catalog
    ListApps,
    /// Install one app from the catalog
    InstallApp { id: String },
    /// List components from the manifest
    ListComponents,
}
