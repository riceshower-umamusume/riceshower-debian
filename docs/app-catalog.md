# App catalog and installation backends

The `install-app <id>` command reads one TOML file from `apps/`. Supported
`kind` values are `apt`, `source`, `cargo`, `flatpak`, and `appimage`.
The installer detects Debian's native architecture with
`dpkg --print-architecture` before selecting architecture-specific packages.

## APT

```toml
id = "example-apt"
name = "Example APT app"
kind = "apt"
repo = "https://packages.example.org/debian"
packages = ["example"]
[packages_by_arch]
amd64 = ["example"]
arm64 = ["example"]
apt_source = "deb [arch=amd64 signed-by=/etc/apt/keyrings/example-apt.gpg] https://packages.example.org/debian stable main"
apt_key_file = "keys/example-apt.gpg"
```

Use `packages_by_arch` when package names differ by architecture; otherwise
`packages` is used. The installer replaces an `arch=amd64` qualifier in
`apt_source` with the detected architecture. Key files must be inside the app
catalog directory and are installed into `/etc/apt/keyrings`.

## Git source build

```toml
id = "example-source"
name = "Example source app"
kind = "source"
source_url = "https://github.com/example/project.git"
git_commit = "0123456789abcdef0123456789abcdef01234567"
build_command = ["cd {source} && meson setup build && meson compile -C build"]
install_command = ["cd {source} && meson install -C build --destdir {source}/destdir"]
```

Source apps require a full 40-character commit SHA. The checkout is detached
and verified against that SHA before recipes execute. Recipe entries are
trusted shell commands maintained by this repository; `{source}` is replaced
with the absolute checkout path. Recipes are not sandboxed. Use explicit,
reviewed install commands and avoid destructive commands.

## Cargo

```toml
id = "example-cargo"
name = "Example Rust app"
kind = "cargo"
source_url = "https://github.com/example/project.git"
git_commit = "0123456789abcdef0123456789abcdef01234567"
cargo_package = "example"
cargo_bin = "example"
```

Cargo apps use `cargo install --git --rev ... --locked` and install into
`~/.local/opt/noctalia-apps/<id>`. Provide a full commit SHA for immutable
reproducibility; a branch name or movable tag is not a reproducible pin.

## Flatpak

```toml
id = "org.example.App"
name = "Example Flatpak"
kind = "flatpak"
flatpak_ref = "org.example.App//stable"
flatpak_remote = "flathub"
```

Flatpaks are installed per-user. The Flathub remote is added if it is missing.

## AppImage

```toml
id = "example-appimage"
name = "Example AppImage"
kind = "appimage"
appimage_url = "https://downloads.example.org/example.AppImage"
appimage_sha256 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
```

AppImages require a SHA-256 digest. Downloads are verified before being made
executable and stored in `~/.local/opt/noctalia-apps/`.

## Dry-run and installed app state

`./install.sh install-app <id> --dry-run` prints the selected backend plan and
does not install or write state. Successful installs are recorded in
`~/.local/state/noctalia-setup/apps.toml`, including backend, reference/version,
detected architecture, and install timestamp. This state is separate from the
system component state.

The app catalog currently contains the existing Brave Origin Nightly and
Fcitx5 Lotus APT entries. Additional backend-specific apps are added by
committing reviewed TOML manifests. The installer does not infer arbitrary
build commands or download unverified AppImages.
