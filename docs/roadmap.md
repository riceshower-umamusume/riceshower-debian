# Roadmap

Mỗi giai đoạn chỉ được coi là xong khi tiêu chí nghiệm thu đã chạy trên Debian thật.
Repo này mới bắt đầu lại lịch sử; không mang roadmap đa distro cũ sang.

## Giai đoạn 0 — Manifest Debian-only

- [x] 5 component manifest, `build_requirements`, `cxx_standard`.
- [x] `debian-stable`, `debian-testing`, `debian-sid`.
- [x] Stable private dependency list; testing/sid system libraries.
- [x] Prefix `/usr/local`; component build artifacts.
- [x] Xác minh tên package/version thật trên VM Debian 13/testing/sid.

## Giai đoạn 1 — Preflight

- [x] APT/dpkg, pkg-config, compiler standard.
- [x] Báo cáo installed/installable/missing/version/private; install thật dừng khi blocking.
- [x] Preflight chạy trong VM Debian 13/testing/sid.

## Giai đoạn 2 — Source

- [x] Cache XDG theo component/SHA, verify HEAD, allowlist GitHub, timeout, chống cache bẩn.
- [x] Từ chối submodule/git-lfs.
- [ ] Fetch upstream `main` mặc định khi cài (hiện build command dùng lock).

## Giai đoạn 3 — Build/staging

- [x] Meson setup/compile/test/install `--destdir`; staging inventory SHA256.
- [x] Prefix `/usr/local`; chặn path/symlink escape.
- [x] 5/5 component build/staging đã chạy thật trên host Debian sid cũ.
- [x] Build 5/5 trong VM Debian 13/testing/sid (5/5 deploy `/usr/local`).

## Dotfiles/hardware đã có bản đầu tiên: config Umbriel/Noctalia/fcitx5/shell, backup/state,
display 1920x1080@144, app catalog, `install-app` apt và GPU package profiles.
DNS, display, wizard full, uninstall/update/repair và engine install/deploy đã có.

Giai đoạn 4 — Stable private dependency builder

- [x] Pin source/SHA và recipe cho wlroots, wayland stack, libinput, xkbcommon,
  libdisplay-info, libei, sdbus-c++, WirePlumber.
- [x] Build vào `/opt/noctalia/deps`; pkg-config, RPATH runtime.
- [x] Không nhiễm môi trường ứng dụng khác.

## Giai đoạn 5 — Deploy transaction

- [x] Deploy writer cho `/usr/local`; inventory; file/symlink; backup/state/rollback.
- [x] Package manager apt idempotent; không tự thêm repo.
- [x] Unit test temp root; dry-run plan trong `install --dry-run`.
- [x] Chạy thật trên VM sạch /usr/local; binary chạy với private libs.
- [ ] Kiểm tra login/logout và greeter thật (VM không có GPU/seat).

## Giai đoạn 6 — Wizard, `--yes`, greeter

- [x] Wizard mặc định (không subcommand): distro/GPU/output/DNS/dotfiles/apps.
- [x] `--dry-run` in kế hoạch; `--yes` áp dụng dotfiles/display.
- [x] Wizard `--yes` chạy luôn full install.
- [x] Greeter opt-in `--greeter`; mặc định bỏ qua; không tự đổi DM.
- [ ] PAM/getty rollback-safe trên máy thật.

## Giai đoạn 7 — Update/repair/uninstall

- [ ] Update theo main, không rollback (hiện update lockfile + redeploy).
- [x] Repair/uninstall chỉ file do installer sở hữu; giữ config user.

## Giai đoạn 8 — Validation và CI

- [x] CI matrix container Debian 13/testing/sid.
- [x] VM sạch: clone, wizard/`--yes`, 5/5 (sid/testing/stable).
- [ ] Login/logout và greeter thật.
- [ ] Release x86_64 với SHA256/attestation.

Các mục còn lại: app build-from-source, login/greeter thật, release có attestation, GUI.

## Giai đoạn 9 — Gaming, GPU và terminal

- [x] Profiles `base`, `terminal`, `gaming`, `all`; `--profile` trong install/wizard/update/repair.
- [x] Tự bật `contrib`/`non-free`, i386 multiarch, microcode theo CPU, apt update.
- [x] Steam/gamescope/MangoHud/GameMode/Lutris/Wine/i386 Mesa-Vulkan.
- [x] zsh + starship + autosuggestions + syntax-highlighting + fzf/eza/bat/fd/rg/zoxide.
- [x] Starship build từ source bằng cargo vào `/usr/local`; dotfiles zsh/starship/MangoHud.
- [x] Verify full gaming/terminal install trên VM sid; dry-run stable/testing.
- [ ] Verify full gaming/terminal install trên VM stable/testing.
- [ ] Source recipe cho gamescope/MangoHud/GameMode.
- [ ] Steam first-run, 32-bit Vulkan và GPU vendor trên máy thật.
