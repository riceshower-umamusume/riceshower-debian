# Contributing

This repository is Debian-only. Keep changes inside the five-component Noctalia scope
and the three supported suites.

## Adding or changing a component

1. Add `manifest/components/<id>.toml` with source URL, build system, build tools,
   build dependencies, `build_requirements`, artifacts and runtime dependencies.
2. Add the component to `manifest/ecosystem.toml` only if it is part of the current
   Noctalia ecosystem. Future components must work through the same manifest/hook model.
3. Add any special behavior as `hooks/<component>/debian.sh`; do not add per-component
   Rust branches.
4. Verify package names and pkg-config modules on Debian stable/testing/sid. The stable
   suite may use private dependencies under `/opt/noctalia/deps`, but testing/sid use
   Debian system libraries.
5. Run the manifest tests and `./install.sh install --dry-run` on a Debian host or
   container.

## Quality gates

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release
shellcheck install.sh
```

Do not claim install/deploy works until it has been tested on clean Debian stable,
testing and sid VMs. The repository currently stops before deployment.
