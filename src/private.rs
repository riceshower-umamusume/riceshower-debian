use crate::build;
use crate::deploy;
use crate::manifest;
use crate::manifest::Manifest;
use crate::process::{SudoRunner, SystemRunner};
use crate::source;
use crate::state;
use anyhow::Result;
use std::env;
use std::path::Path;

pub const PREFIX: &str = "/opt/noctalia/deps";

pub fn environment() -> Vec<(String, String)> {
    let lib = format!("{PREFIX}/lib");
    let lib_arch = format!("{PREFIX}/lib/x86_64-linux-gnu");
    let pkg_config = [
        format!("{lib}/pkgconfig"),
        format!("{lib_arch}/pkgconfig"),
        format!("{PREFIX}/share/pkgconfig"),
    ]
    .join(":");
    let library_path = format!("{lib}:{lib_arch}");
    let path = match env::var("PATH") {
        Ok(current) if !current.is_empty() => format!("{PREFIX}/bin:{current}"),
        _ => format!("{PREFIX}/bin"),
    };
    vec![
        ("PKG_CONFIG_PATH".to_string(), pkg_config),
        ("LD_LIBRARY_PATH".to_string(), library_path),
        ("PATH".to_string(), path),
        ("CMAKE_PREFIX_PATH".to_string(), PREFIX.to_string()),
    ]
}

pub fn build_all(manifest: &Manifest, cache: &Path, sudo: &SudoRunner<SystemRunner>) -> Result<()> {
    let components = manifest::load_private(&manifest.root)?;
    if components.is_empty() {
        return Ok(());
    }
    let ids: Vec<String> = components.keys().cloned().collect();
    let order = build::build_order_components(&components, &ids)?;
    let env = environment();
    for id in order {
        let component = &components[&id];
        let commit = component.git_ref.clone();
        println!("building private dependency {id} @ {commit}...");
        let source_dir = source::acquire_component(component, &commit, &SystemRunner)?;
        let outcome = build::build_component(
            component,
            &commit,
            &source_dir,
            cache,
            PREFIX,
            &env,
            &SystemRunner,
        )?;
        let files = vec![deploy::DeployComponent {
            id: outcome.component,
            commit: outcome.commit,
            staging_dir: outcome.staging_dir,
            inventory: outcome.inventory,
        }];
        println!("deploying private dependency {id} to {PREFIX}...");
        let writer = deploy::SudoWriter { runner: sudo };
        deploy::deploy_components(
            Path::new("/"),
            &files,
            &[],
            "private",
            &state::state_path(),
            &writer,
            false,
        )?;
    }
    Ok(())
}
