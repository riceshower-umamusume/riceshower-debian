## Verified installs

The installer has been run end-to-end in clean Debian 13 (stable), Debian 14
(testing) and Debian sid QEMU VMs: all five components built from source and
deployed to `/usr/local`, dotfiles (including the DP-1 1920x1080@144 display
profile and DNS drop-in) applied, and the binaries verified against system or
private `/opt/noctalia/deps` libraries.

### Lifecycle

- App catalog verified in the testing VM: `fcitx5-lotus` 7.0.2 and
  `brave-browser-nightly` 1.102.46 installed from their upstream apt repos.
- `install` builds and deploys; `--greeter` opts into Noctalia Greeter.
- `update [--latest]` refreshes the lockfile and redeploys.
- `repair` reruns the pinned install plan.
- `uninstall [--purge]` removes state-recorded files and optionally packages.
- `list-apps` / `install-app <id>` use the app catalog with apt source/key support.

# riceshower-debian

Noctalia full-ecosystem installer for **Debian stable, testing and sid**, built from source.

> **Status: Noctalia install plus base/terminal/gaming profiles.** The engine builds
> the five Noctalia components from source, prepares Debian apt (contrib/non-free,
> i386 for Steam, CPU microcode), installs the base/terminal/gaming package profiles,
> builds Starship from source with cargo, and applies the dotfiles. The default
> `install` runs all profiles; select a subset with `--profile base|terminal|gaming`.
> Full gaming/terminal install verified on a clean Debian sid VM: profile packages,
> i386/contrib/non-free, Starship 1.22.1 built from source, Steam/MangoHud/GameMode/
> Gamescope and the zsh/starship/MangoHud dotfiles. Stable and testing profile
> dry-runs pass.

## Scope

- **Only Debian**: 13/trixie (stable), 14/forky (testing), sid. No Ubuntu, Mint, Fedora,
  Arch, CachyOS, NixOS or CaramOS.
- **Full current ecosystem**: Umbriel, Noctalia Shell, xdg-desktop-portal-umbriel,
  Luciole and Noctalia Greeter. Future Noctalia components are added through the
  manifest/hook system, not new distro code.
- **Build from source**: the five components come from upstream sources, not native
  Noctalia packages. Current decision: resolve upstream `main` on install, with no
  last-good rollback (accepted risk).
- **Small footprint**: components install to `/usr/local`; stable-only missing libraries
  build into `/opt/noctalia/deps`; system libraries, `/usr`, kernel, GPU drivers and
  bootloader are not replaced.
- **No DM switching**: the greeter is installed as a component, but greetd/Noctalia
  Greeter configuration requires an explicit `--greeter`. The default is no display
  manager change.
- **Wizard + `--yes`**: interactive wizard by default; `--yes` uses the documented
  defaults for a fully automatic run. Sudo is requested once through a kitty popup when
  available.

## Decisions already fixed

| Item | Decision |
| --- | --- |
| Repo | `riceshower-umamusume/riceshower-debian`, fresh history |
| Suites | Debian stable 13/trixie, testing 14/forky, sid |
| Components | Umbriel, Noctalia, portal-umbriel, Luciole, Greeter |
| Prefix | components `/usr/local`; stable private deps `/opt/noctalia/deps` |
| Source | upstream `main`, no lock/rollback (explicitly accepted) |
| Greeter | opt-in, no automatic DM switch |
| History | no old repo history, no legacy archive |

## Quick start (current scaffold)

```bash
git clone https://github.com/riceshower-umamusume/riceshower-debian.git
cd riceshower-debian

./install.sh            # wizard: distro, GPU, DNS, dotfiles, apps
./install.sh --dry-run  # wizard plan only
./install.sh doctor
./install.sh hardware
./install.sh list-components
./install.sh list-apps
./install.sh install-app fcitx5-lotus --dry-run
./install.sh install --dry-run
./install.sh install --dry-run --profile gaming
./install.sh install --dry-run --profile terminal
./install.sh install --dry-run --profile base
./install.sh config --dry-run
./install.sh config
./install.sh build umbriel

cargo test
cargo clippy --all-targets -- -D warnings
```

`install` without `--dry-run` still stops before deployment. The build command fetches
the locked source and stages it under `~/.cache/noctalia-setup` without changing the host.

## Upgrade stable/testing to Sid

Sid and unstable are the same Debian branch: `unstable` is the suite, `sid` is
the codename. The script below follows Method 2 of the AtulHost guide, adapted
for Debian 13 deb822 sources and the TheLinuxVault warnings (no `-security`
suite, backup sources, verify the result):

```bash
sudo ./scripts/upgrade-to-sid.sh --dry-run
sudo ./scripts/upgrade-to-sid.sh --yes
sudo reboot
```

It backs up Debian source files to `/var/backups/upgrade-to-sid-<timestamp>`,
disables old stable/testing Debian archive sources, writes
`/etc/apt/sources.list.d/sid.sources` with
`Suites: sid`, runs `apt-get update` and `apt-get full-upgrade`, then
verifies `VERSION_CODENAME=sid` and `/etc/debian_version`. Add `--reboot` to
reboot automatically. This is one-way; back up `/home` first.

## Boot repair (BIOS and UEFI)

The installer never writes the bootloader. On a fresh mini.iso install where the
firmware boot menu has no Debian entry, inspect and then repair it explicitly:

```bash
./install.sh boot --dry-run
./install.sh boot --yes    # uses sudo; reboot afterwards
```

It detects BIOS vs UEFI, derives the root disk with `findmnt`/`lsblk` (override
with `--disk /dev/nvme0n1`), installs `grub-pc` or `grub-efi-amd64`, runs
`grub-install --recheck`, `update-grub`, and creates an EFI boot entry if
`efibootmgr` shows none. On UEFI, mount the ESP at `/boot/efi` first, or pass
`--efi-dir`. Safe on a running installed system; for a system that cannot boot,
chroot from a live USB:

```bash
mount /dev/nvme0n1p1 /mnt
mount --bind /dev /mnt/dev
mount --bind /proc /mnt/proc
mount --bind /sys /mnt/sys
chroot /mnt
# clone the repo inside the chroot, then:
./install.sh boot --yes --disk /dev/nvme0n1
```

## Terminal, zsh and hotkeys

`install`/`config` installs the terminal profile, applies the zsh dotfiles and,
if `/usr/bin/zsh` exists, changes the user's login shell with
`chsh -s /usr/bin/zsh` (dry-run prints it; restart the session or run
`exec zsh`). The VM verification ended with `SHELL=/usr/bin/zsh`.

Hotkeys in `~/.zshrc`:

- `Ctrl-R` — fzf history search
- `Ctrl-T` — fzf file picker
- `Alt-C` — fzf directory change
- `→` / `End` — accept zsh-autosuggestions
- `Tab` — completion; `z <dir>` / `zi` — zoxide; `ls`/`ll`/`la` — eza

The keymap is Emacs (`bindkey -e`). `zsh-syntax-highlighting` has no keybinding.

## Languages

- `install.sh` — **bash** (`#!/usr/bin/env bash`, `[[`, `set -Eeuo pipefail`).
  Run `./install.sh` or `bash install.sh`; do not run `zsh install.sh` (the
  script now prints an explicit error if invoked by zsh).
- `src/*.rs` — **Rust** CLI; `install.sh` builds/runs it with Cargo.
- `manifest/`, `profiles/`, `apps/` — **TOML**.
- `dotfiles/files/shell/zshrc` — **zsh**; `bashrc`/`profile` — bash/sh;
  Noctalia/Umbriel/fcitx5 configs — TOML/JSON.
- No Python or make-based installer; upstream components use Meson/Cargo.

## Step-by-step

```bash
git clone https://github.com/riceshower-umamusume/riceshower-debian.git
cd riceshower-debian
./install.sh doctor                 # read-only distro/state check
./install.sh hardware               # GPU/output/mode
./install.sh install --dry-run      # full plan
./install.sh install --dry-run --profile gaming
./install.sh install --yes          # default: all profiles
# optional: ./install.sh install --yes --greeter
exec zsh                            # or log out/in; shell is already changed
echo $SHELL; starship --version
./install.sh config                 # reapply dotfiles/DNS/shell
./install.sh config --dry-run
./install.sh install-app fcitx5-lotus --dry-run
./install.sh install-app fcitx5-lotus
./install.sh install-app brave-origin-nightly
./install.sh update / repair / uninstall
```

After install, log out/in for the session environment and choose the Umbriel /
Noctalia session; the greeter is opt-in and the repo never switches the display
manager automatically. Steam first-run is manual; MangoHud env is
`~/.config/environment.d/90-gaming.conf`, and the Steam launch example is
`gamescope -W 1920 -H 1080 -r 144 --mangoapp -- %command%`.

## Layout

```
src/                 Rust CLI, manifest, distro/version detection, preflight, source/build/state
manifest/components/ five component manifests with build requirements
manifest/distros/    debian-stable.toml, debian-testing.toml, debian-sid.toml
manifest/lock/       source snapshot used by the build command
dotfiles/            personal config manifest + Umbriel/Noctalia/fcitx5/shell files
apps/                app catalog (fcitx5-lotus, Brave Origin Nightly)
profiles/            base, terminal, gaming and all install profiles
manifest/tools/      source tools (starship via cargo)
docs/                design, roadmap, dependency and gaming research
install.sh           bootstrap wrapper
```

## Gaming and terminal profiles

- `base`: firmware, CPU microcode, earlyoom, zram, timeshift, build tools.
- `terminal`: zsh, starship (source, cargo), autosuggestions, syntax highlighting,
  fzf, eza, bat, fd, ripgrep, zoxide.
- `gaming`: Steam, Lutris, Wine, i386 Mesa/Vulkan, plus Gamescope, MangoHud and
  GameMode built from pinned source recipes (apt packages stay as fallback).
  `build --tool <id> [--deploy]` builds/deploys one source tool without a full
  install.
- `all` (default): base + terminal + gaming.

`./install.sh install --dry-run --profile gaming` previews one profile. A full
install enables Debian `contrib`/`non-free`, adds i386, installs the packages,
builds Starship to `/usr/local`, then applies the zsh/starship/MangoHud dotfiles.
Steam launch example: `gamescope -W 1920 -H 1080 -r 144 --mangoapp -- %command%`.

## Next work

1. Finish full gaming/terminal VM verification on sid/testing/stable.
2. Verify the pinned Gamescope/MangoHud/GameMode source recipes on stable/testing
   (sid source builds and tests already pass).
3. Verify Steam first-run, 32-bit Vulkan and the GPU vendor packages on real hardware.
4. Optional GUI front-end over the same profiles.

## License

MIT. See [LICENSE](LICENSE).
