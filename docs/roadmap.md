# Roadmap

Mỗi giai đoạn chỉ được coi là xong khi tiêu chí nghiệm thu đã chạy trên Debian thật.
Repo này mới bắt đầu lại lịch sử; không mang roadmap đa distro cũ sang.

## Giai đoạn 0 — Manifest Debian-only

- [x] 5 component manifest, `build_requirements`, `cxx_standard`.
- [x] `debian-stable`, `debian-testing`, `debian-sid`.
- [x] Stable private dependency list; testing/sid system libraries.
- [x] Prefix `/usr/local`; component build artifacts.
- [ ] Xác minh tên package/version thật trên từng suite (container/VM).

## Giai đoạn 1 — Preflight

- [x] APT/dpkg, pkg-config, compiler standard.
- [x] Báo cáo installed/installable/missing/version/private; install thật dừng khi blocking.
- [ ] Chạy preflight trong container stable/testing/sid.

## Giai đoạn 2 — Source

- [x] Cache XDG theo component/SHA, verify HEAD, allowlist GitHub, timeout, chống cache bẩn.
- [x] Từ chối submodule/git-lfs.
- [ ] Fetch upstream `main` mặc định khi cài (hiện build command dùng lock).

## Giai đoạn 3 — Build/staging

- [x] Meson setup/compile/test/install `--destdir`; staging inventory SHA256.
- [x] Prefix `/usr/local`; chặn path/symlink escape.
- [x] 5/5 component build/staging đã chạy thật trên host Debian sid cũ.
- [ ] Build 5/5 trong container stable/testing/sid của repo mới.

## Giai đoạn 4 — Stable private dependency builder

- [ ] Pin source/SHA và recipe cho wlroots, wayland stack, libinput, xkbcommon,
  libdisplay-info, libei, sdbus-c++, WirePlumber.
- [ ] Build vào `/opt/noctalia/deps`; pkg-config/linker/runtime wrapper.
- [ ] Không nhiễm môi trường ứng dụng khác.

## Giai đoạn 5 — Deploy transaction

- [ ] Backup/state/rollback; chỉ ghi `/usr/local` và `/opt/noctalia`.
- [ ] Package manager idempotent; không tự thêm repo.
- [ ] Dry-run transaction plan.

## Giai đoạn 6 — Wizard, `--yes`, greeter

- [ ] Wizard mặc định; `--yes` dùng defaults.
- [ ] Greeter opt-in `--greeter`; không tự đổi DM; PAM/getty rollback-safe.

## Giai đoạn 7 — Update/repair/uninstall

- [ ] Update theo main, không rollback (quyết định hiện tại).
- [ ] Repair/uninstall chỉ file do installer sở hữu; giữ config user.

## Giai đoạn 8 — Validation và CI

- [ ] Container stable/testing/sid build + preflight.
- [ ] VM sạch: clone, wizard/`--yes`, 5/5, login/logout, greeter opt-in.
- [ ] Release x86_64 với SHA256/attestation.

Cho đến khi các mục trên xong, README phải ghi rõ đây là scaffold chưa cài được.
