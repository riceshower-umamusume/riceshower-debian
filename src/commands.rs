use crate::apps;
use crate::cli::{Cli, Command};
use crate::distro;
use crate::manifest::{self, DistroManifest, Manifest};
use crate::process::SystemRunner;
use anyhow::{Context, Result};
mod build;
mod config;
mod doctor;
mod install;
mod uninstall;
mod update;
mod version;
mod wizard;

use build::build_component;
use build::build_tool;
use doctor::doctor;
use install::{install_plan, InstallOptions};
use uninstall::uninstall_plan;
use update::update_plan;
use wizard::wizard;

pub fn run(cli: Cli) -> Result<()> {
    let Cli {
        manifest_dir,
        dotfiles_dir,
        profiles_dir,
        apps_dir,
        dry_run,
        yes,
        command,
        ..
    } = cli;
    let manifest = manifest::load(&manifest_dir)
        .with_context(|| format!("loading manifest from {}", manifest_dir.display()))?;
    let Some(command) = command else {
        return wizard(
            &manifest,
            dry_run,
            yes,
            &dotfiles_dir,
            &profiles_dir,
            &apps_dir,
        );
    };
    match command {
        Command::Config => config::config(&dotfiles_dir, dry_run),
        Command::Hardware => config::hardware_command(),
        Command::ListApps => apps::print(&apps_dir),
        Command::InstallApp { id } => apps::install(&apps_dir, &id, dry_run, &SystemRunner),
        Command::Build {
            component,
            tool,
            commit,
            deploy,
        } => {
            if let Some(tool) = tool {
                build_tool(&manifest, &tool, commit.as_deref(), dry_run, deploy)
            } else {
                let component = component.context("build requires a component or --tool")?;
                build_component(&manifest, &component, commit.as_deref(), dry_run)
            }
        }
        Command::ListComponents => list_components(&manifest),
        Command::Boot { disk, efi_dir } => {
            let os = distro::read_os_release()?;
            let distro_id = distro::detect_distro(&os);
            let distro = require_distro(&manifest, &distro_id)?;
            crate::boot::boot(distro, disk.as_deref(), &efi_dir, dry_run, yes)
        }
        Command::Doctor => {
            let os = distro::read_os_release()?;
            let distro_id = distro::detect_distro(&os);
            let distro = manifest.distros.get(&distro_id);
            doctor(&manifest, &distro_id, distro, &os)
        }
        Command::Install {
            components,
            greeter,
            latest,
            profiles,
        } => {
            let os = distro::read_os_release()?;
            let distro_id = distro::detect_distro(&os);
            let distro = require_distro(&manifest, &distro_id)?;
            install_plan(
                &manifest,
                distro,
                &os,
                dry_run,
                &dotfiles_dir,
                &profiles_dir,
                InstallOptions {
                    requested: &components,
                    profiles: &profiles,
                    no_greeter: !greeter,
                    latest,
                    yes,
                },
            )
        }
        Command::Update { latest, profiles } => {
            let os = distro::read_os_release()?;
            let distro_id = distro::detect_distro(&os);
            let distro = require_distro(&manifest, &distro_id)?;
            update_plan(&manifest, distro, &os, dry_run, latest, true)?;
            if dry_run {
                return Ok(());
            }
            install_plan(
                &manifest,
                distro,
                &os,
                false,
                &dotfiles_dir,
                &profiles_dir,
                InstallOptions {
                    requested: &[],
                    profiles: &profiles,
                    no_greeter: true,
                    latest: false,
                    yes,
                },
            )
        }
        Command::Repair => {
            let os = distro::read_os_release()?;
            let distro_id = distro::detect_distro(&os);
            let distro = require_distro(&manifest, &distro_id)?;
            install_plan(
                &manifest,
                distro,
                &os,
                dry_run,
                &dotfiles_dir,
                &profiles_dir,
                InstallOptions {
                    requested: &[],
                    profiles: &[],
                    no_greeter: true,
                    latest: false,
                    yes: true,
                },
            )
        }
        Command::Uninstall { purge } => {
            let os = distro::read_os_release()?;
            let distro_id = distro::detect_distro(&os);
            let distro = require_distro(&manifest, &distro_id)?;
            uninstall_plan(distro, dry_run, purge)
        }
    }
}

fn require_distro<'a>(manifest: &'a Manifest, distro_id: &str) -> Result<&'a DistroManifest> {
    manifest
        .distros
        .get(distro_id)
        .with_context(|| format!("distro '{distro_id}' is not in the manifest"))
}

fn list_components(manifest: &Manifest) -> Result<()> {
    println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    println!(
        "{} components from {}",
        manifest.components.len(),
        manifest.root.display()
    );
    for id in &manifest.ecosystem.components {
        if let Some(component) = manifest.components.get(id) {
            println!(
                "{:<28} {:<24} {} ({})",
                component.id, component.name, component.repo, component.build_system
            );
        }
    }
    Ok(())
}
