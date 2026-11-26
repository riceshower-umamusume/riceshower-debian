# riceshower-debian

Noctalia full-ecosystem installer for **Debian stable, testing and sid**, built from source.

> **Status: engine ported, installer not finished.** The repo was created fresh after
> deleting the three previous Noctalia setup repositories. The manifest, dependency
> preflight, source cache and Meson build/staging engine are ported and tested. Deploy,
> the stable private-dependency builder, the wizard, greeter setup, update/repair and
> uninstall are still missing. This is not yet a "clone and it installs everything" repo.

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

./install.sh doctor
./install.sh list-components
./install.sh install --dry-run
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
docs/                design, roadmap, dependency audit
install.sh           bootstrap wrapper
```

## Next work

1. Stable private-dependency builder into `/opt/noctalia/deps` (wlroots, wayland stack,
   libinput, xkbcommon, libdisplay-info, libei, sdbus-c++, WirePlumber).
2. Deploy transaction to `/usr/local` with backup/state/rollback.
3. Wizard and `--yes` defaults.
4. Greeter/PAM/getty integration, opt-in and rollback-safe.
5. Update/repair/uninstall and clean Debian stable/testing/sid VM validation.

## License

MIT. See [LICENSE](LICENSE).
