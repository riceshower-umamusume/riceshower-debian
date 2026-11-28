use crate::deploy;
use crate::manifest::DistroManifest;
use crate::process::{Runner, SudoRunner, SystemRunner};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

const GRUB_INSTALL: &str = "/usr/sbin/grub-install";
const UPDATE_GRUB: &str = "/usr/sbin/update-grub";

pub fn boot(
    distro: &DistroManifest,
    disk: Option<&Path>,
    efi_dir: &Path,
    dry_run: bool,
    yes: bool,
) -> Result<()> {
    if !yes && !dry_run {
        bail!("boot requires --yes (it writes the bootloader); use --dry-run first");
    }
    let runner = SystemRunner;
    let uefi = Path::new("/sys/firmware/efi").is_dir();
    let disk = match disk {
        Some(path) => path.to_path_buf(),
        None => detect_root_disk(&runner)?,
    };
    if !disk.is_absolute() {
        bail!("--disk must be an absolute block device, e.g. /dev/nvme0n1");
    }
    if uefi && !efi_dir.is_dir() {
        bail!(
            "UEFI detected but {} is not mounted; mount the ESP there (or pass --efi-dir)",
            efi_dir.display()
        );
    }
    let mut packages = vec!["os-prober".to_string()];
    if uefi {
        packages.extend(["grub-efi-amd64".to_string(), "efibootmgr".to_string()]);
    } else {
        packages.push("grub-pc".to_string());
    }
    println!("boot plan");
    println!("  firmware: {}", if uefi { "UEFI" } else { "BIOS" });
    println!("  root disk: {}", disk.display());
    println!("  packages: {}", packages.join(", "));
    let mut commands: Vec<Vec<String>> = Vec::new();
    if uefi {
        commands.push(vec![
            GRUB_INSTALL.into(),
            "--target=x86_64-efi".into(),
            "--efi-directory".into(),
            efi_dir.display().to_string(),
            "--bootloader-id=debian".into(),
            "--recheck".into(),
        ]);
    } else {
        commands.push(vec![
            GRUB_INSTALL.into(),
            "--target=i386-pc".into(),
            "--recheck".into(),
            disk.display().to_string(),
        ]);
    }
    commands.push(vec![UPDATE_GRUB.into()]);
    for command in &commands {
        println!("  {}", command.join(" "));
    }
    if dry_run {
        println!("dry-run: no bootloader changes made");
        return Ok(());
    }
    let sudo = SudoRunner::new(SystemRunner);
    if !uefi {
        let selections = format!(
            "grub-pc grub-pc/install_devices multiselect {}\n",
            disk.display()
        );
        let output = sudo.run_with_stdin("debconf-set-selections", &[], &selections)?;
        if !output.success {
            bail!("debconf-set-selections failed: {}", output.stderr.trim());
        }
    }
    deploy::install_packages(distro, &packages, &sudo)?;
    for command in &commands {
        let args: Vec<&str> = command.iter().skip(1).map(String::as_str).collect();
        let output = sudo.run(&command[0], &args)?;
        if !output.success {
            bail!(
                "{} failed: {}",
                command.join(" "),
                if output.stderr.trim().is_empty() {
                    output.stdout.trim()
                } else {
                    output.stderr.trim()
                }
            );
        }
    }
    if uefi {
        ensure_efi_entry(&runner, &sudo, efi_dir)?;
    }
    println!(
        "boot: GRUB installed on {}; reboot and select Debian",
        disk.display()
    );
    Ok(())
}

fn detect_root_disk(runner: &dyn Runner) -> Result<PathBuf> {
    let output = runner.run("findmnt", &["-n", "-o", "SOURCE", "/"])?;
    if !output.success {
        bail!("findmnt failed: {}", output.stderr.trim());
    }
    let source = output.stdout.trim();
    if !source.starts_with("/dev/") {
        bail!("root source is not a block device ({source}); pass --disk explicitly");
    }
    let output = runner.run("lsblk", &["-no", "PKNAME", source])?;
    let parent = output.stdout.trim();
    if !output.success || parent.is_empty() {
        bail!("cannot find the parent disk of {source}; pass --disk explicitly");
    }
    Ok(PathBuf::from(format!("/dev/{parent}")))
}

fn ensure_efi_entry(
    runner: &dyn Runner,
    sudo: &SudoRunner<SystemRunner>,
    efi_dir: &Path,
) -> Result<()> {
    let entries = sudo.run("efibootmgr", &["-v"])?;
    if entries.success && entries.stdout.to_lowercase().contains("debian") {
        println!("boot: EFI boot entry already present");
        return Ok(());
    }
    let source = runner.run(
        "findmnt",
        &["-n", "-o", "SOURCE", &efi_dir.display().to_string()],
    )?;
    let source = source.stdout.trim();
    let (disk, part) = partition_of(source)
        .with_context(|| format!("cannot derive disk/partition from ESP source {source}"))?;
    let output = sudo.run(
        "efibootmgr",
        &[
            "--create",
            "--disk",
            &disk,
            "--part",
            &part,
            "--label",
            "Debian",
            "--loader",
            "\\EFI\\debian\\grubx64.efi",
        ],
    )?;
    if !output.success {
        bail!("efibootmgr --create failed: {}", output.stderr.trim());
    }
    println!("boot: created EFI entry for \\EFI\\debian\\grubx64.efi");
    Ok(())
}

fn partition_of(source: &str) -> Option<(String, String)> {
    let name = source.strip_prefix("/dev/")?;
    let (disk, part) = if name.starts_with("nvme") || name.starts_with("mmcblk") {
        let index = name.rfind('p')?;
        (&name[..index], &name[index + 1..])
    } else {
        let index = name.rfind(|c: char| !c.is_ascii_digit())?;
        (&name[..=index], &name[index + 1..])
    };
    if disk.is_empty() || part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((format!("/dev/{disk}"), part.to_string()))
}

#[cfg(test)]
mod tests {
    use super::partition_of;

    #[test]
    fn nvme_partition() {
        assert_eq!(
            partition_of("/dev/nvme0n1p2"),
            Some(("/dev/nvme0n1".into(), "2".into()))
        );
    }

    #[test]
    fn sd_partition() {
        assert_eq!(
            partition_of("/dev/sda1"),
            Some(("/dev/sda".into(), "1".into()))
        );
    }
}
