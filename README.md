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
> Full gaming/terminal VM verification is in progress.

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
- `gaming`: Steam, gamescope, MangoHud, GameMode, Lutris, Wine, i386 Mesa/Vulkan.
- `all` (default): base + terminal + gaming.

`./install.sh install --dry-run --profile gaming` previews one profile. A full
install enables Debian `contrib`/`non-free`, adds i386, installs the packages,
builds Starship to `/usr/local`, then applies the zsh/starship/MangoHud dotfiles.
Steam launch example: `gamescope -W 1920 -H 1080 -r 144 --mangoapp -- %command%`.

## Next work

1. Finish full gaming/terminal VM verification on sid/testing/stable.
2. Add pinned source recipes for gamescope, MangoHud and GameMode.
3. Verify Steam first-run, 32-bit Vulkan and the GPU vendor packages on real hardware.
4. Optional GUI front-end over the same profiles.

## License

MIT. See [LICENSE](LICENSE).
