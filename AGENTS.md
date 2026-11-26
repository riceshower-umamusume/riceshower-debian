# AGENTS.md

Canonical instructions for coding agents working in `riceshower-debian`.

## Default mode

Ponytail full unless the user asks otherwise: smallest complete change, no speculative
features, no wrappers for single-use code, one small test for non-trivial logic.

This repository is indexed with CodeGraph (`.codegraph/`, local-only). Use CodeGraph
before broad file reading; run `codegraph sync` after structural changes. Never commit
`.codegraph/`. The CodeGraph project path is `/home/riceshower/riceshower-debian`.

## Non-negotiable project rules

- **Debian only.** Supported suites are stable 13/trixie, testing 14/forky and sid.
  Do not add Ubuntu, Mint, Fedora, Arch, CachyOS, NixOS or CaramOS branches.
- **Manifest is the source of truth.** Components: `manifest/components/<id>.toml`;
  distros: `manifest/distros/<id>.toml`; index: `manifest/ecosystem.toml`.
  Add a future component through manifest/hook, not a distro-specific branch.
- **Five current components only**: `umbriel`, `noctalia`, `xdg-desktop-portal-umbriel`,
  `luciole`, `noctalia-greeter`.
- **Build from source.** Components are built from upstream sources, never from native
  Noctalia packages. Current source policy is upstream `main` with no last-good rollback;
  do not silently reintroduce a lockfile-only install without the user's decision.
- **Prefix**: components `/usr/local`; stable-only private dependencies
  `/opt/noctalia/deps`. Never overwrite `/usr` system libraries.
- **Do not auto-switch the display manager.** Greeter configuration is opt-in through
  `--greeter`; never grant the `input` group; handle getty/PAM explicitly.
- **No third-party repositories**, no `--nogpgcheck`, no kernel/GPU/bootloader changes.
- **Wizard + `--yes`.** The default flow is interactive; `--yes` must use documented
  defaults and still show the sudo action. Do not make silent destructive choices.
- **`/usr` is off-limits for this repo.** The previous installation at `/usr` is not
  migrated or removed automatically.
- **Dotfiles layer**: `dotfiles/manifest.toml` is the source of truth; apply copies with
  backup and state, never silently overwrites. Display settings are generated from
  `hardware` (GPU/output/best mode). Do not commit secrets; app catalog is `apps/*.toml`.
- Run before finishing: `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test --all`,
  `cargo build --release`, `shellcheck install.sh`.

Docs are Vietnamese; code identifiers and comments are English.
