# Code audit — Debian-only installer

Audit date: 2026-11-28
Scope: every Rust source file, manifests, dotfiles, apps, install.sh, CI.
Verification: `cargo test --all` (66), `cargo clippy --all-targets -- -D warnings`,
`cargo build --release`, CI Debian 13/testing/sid + Rust + Shell, and a full
install on a fresh Debian sid VM.

## File review

| File | Lines | Role | Audit note |
| --- | ---: | --- | --- |
| `src/main.rs` | 17 | CLI entry | SIGPIPE restored; delegates to `commands::run`. |
| `src/lib.rs` | 19 | crate modules | All modules registered; no dead module. |
| `src/cli.rs` | 76 | clap CLI | Debian lifecycle only; stubs removed. |
| `src/commands.rs` | 147 | dispatch | Install/update/repair/uninstall/wizard wired. |
| `src/commands/install.rs` | 165 | install plan | Package order, private layer, GPU packages, dotfiles. |
| `src/commands/wizard.rs` | 89 | wizard | `--yes` runs one full install; no double config. |
| `src/commands/config.rs` | 51 | dotfiles/DNS | Uses `xdg::home`; DNS through sudo. |
| `src/commands/update.rs` | 135 | update | Lock refresh + redeploy; no-latest redeploys. |
| `src/commands/uninstall.rs` | 75 | uninstall | Files only, no directories; removes DNS/state. |
| `src/commands/doctor.rs` | 95 | diagnostics | Matching version computed once. |
| `src/commands/build.rs` | 50 | build one | Reuses the build engine. |
| `src/commands/version.rs` | 213 | suite matching | Debian suite/tier checks. |
| `src/plan.rs` | 236 | plan | Component/runtime closure; private deps excluded from apt. |
| `src/preflight.rs` | 751 | preflight | APT-only; private status is non-blocking. |
| `src/build.rs` | 734 | build/staging | Meson + CMake; prefix validation; SHA256 inventory. |
| `src/source.rs` | 577 | source cache | SHA-keyed XDG cache; repo allowlist; tracked-only verify. |
| `src/deploy.rs` | 463 | deploy | Transaction, backups, rollback, state, sudo tee. |
| `src/private.rs` | 83 | private layer | `/opt/noctalia/deps`, RPATH, pkg-config/lib64. |
| `src/state.rs` | 93 | state | `/var/lib/noctalia-setup/state.toml`. |
| `src/dotfiles.rs` | 287 | dotfiles | Backup, mode preservation, duplicate-id check. |
| `src/dns.rs` | 74 | DNS | Upstream reader; resolved drop-in. |
| `src/apps.rs` | 202 | app catalog | apt source/key-file only; id/key validation. |
| `src/hardware.rs` | 232 | hardware | GPU vendor, Umbriel outputs, display TOML. |
| `src/process.rs` | 242 | runner | Streaming/env/process-group; sudo wrapper. |
| `src/version.rs` | 68 | versions | Strict constraints, lenient package suffixes. |
| `src/distro.rs` | 150 | distro detect | Debian-only; apt suite decides test vs sid. |
| `src/atomic_file.rs` | 98 | atomic write | Temp in same directory; create_new. |
| `src/xdg.rs` | 23 | XDG paths | Shared home/state/cache resolution. |
| `install.sh` | 34 | entrypoint | Refuses root; re-execs via SUDO_USER. |
| `manifest/**` | — | manifests | Debian-only; libjxl added for current Noctalia. |
| `dotfiles/**` | — | configs | Display include uses `~`; no absolute home. |
| `apps/**` | — | app catalog | Public keys pinned in repo. |
| `.github/workflows/**` | — | CI | Debian 13/testing/sid matrix; release attestation. |

## Prefix map

| Purpose | Path | Status |
| --- | --- | --- |
| Components | `/usr/local` | correct |
| Stable private deps | `/opt/noctalia/deps` (lib64 RPATH) | correct |
| Installer state | `/var/lib/noctalia-setup` | parent now created before tee |
| User dotfiles | `$HOME/.config`, `$HOME/.local/state` | user-owned |
| Display include | `~/.config/umbriel/display.toml` | no hardcoded home |
| DNS drop-in | `/etc/systemd/resolved.conf.d/90-riceshower-debian.conf` | root system file |
| Source/build cache | `$XDG_CACHE_HOME/noctalia-setup`, else `~/.cache/noctalia-setup` | shared helper |
| App sources/keys | `/etc/apt/sources.list.d`, `/etc/apt/keyrings` | root system files |

## Bugs found and fixed during this audit

1. `dotfiles/files/umbriel/config.toml:19` — absolute `/home/riceshower` include;
   fixed to `~` (99aff4c).
2. `src/commands/wizard.rs` — `--yes` applied dotfiles/DNS twice; fixed (a922050).
3. `src/commands/install.rs` — hardcoded “5 components” and stale private text;
   fixed (a922050).
4. `manifest/components/noctalia.toml` — missing `libjxl`, so a fresh sid without
   the greeter failed Meson setup; fixed (d7cf47c).
5. `src/apps.rs` — unused shell `apt_key` branch and unvalidated ids/key paths;
   fixed (7749074).
6. `src/deploy.rs` — state parent not created before `tee`; fresh sid deploy
   failed and rolled back; fixed (19096e5).
7. `src/commands/update.rs` — `update` without `--latest` bailed instead of
   redeploying; fixed.
8. `src/commands/doctor.rs` — suite matched twice; fixed.
9. `src/apps.rs` / `src/dotfiles.rs` — duplicate ids accepted; now rejected.
10. `src/source.rs` / `src/dotfiles.rs` — duplicated XDG resolution; moved to
    `src/xdg.rs`.

## Verification

- Fresh Debian sid VM: `install complete`; 4 components (greeter opt-in),
  `umbriel 0.1.0`, `noctalia v5.6.0`, portal/Luciole deployed.
- `~/.config/umbriel/display.toml` = DP-1 1920x1080@144; include `~`.
- DNS = upstream nameservers; all home config `test:test` (not root).
- `cargo test --all`: 66 passed; clippy exit 0; release build exit 0.
- CI: Debian 13/testing/sid, Rust and Shell jobs green.

## Remaining risks

- Login/greeter was not exercised on a real seat/GPU; the greeter remains opt-in.
- App source builds are future work; apps currently use apt packages.
- A direct `target/release/noctalia-setup config` run as root bypasses the
  `install.sh` root guard.
