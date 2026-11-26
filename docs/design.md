# Thiết kế riceshower-debian

Tài liệu này ghi các quyết định đã chốt cho repo mới. Repo cũ và toàn bộ lịch sử cũ
đã bị xoá; đây là lịch sử mới, không import commit cũ.

## 1. Phạm vi

- Chỉ Debian: stable 13/trixie, testing 14/forky, sid.
- Đúng 5 component hiện tại: Umbriel, Noctalia Shell, xdg-desktop-portal-umbriel,
  Luciole, Noctalia Greeter.
- Component tương lai thêm bằng manifest/hook; không thêm backend distro khác.
- Không hỗ trợ Ubuntu, Mint, Fedora, Arch, CachyOS, NixOS, CaramOS.

## 2. Nguồn và tính tái lập

- Component luôn build từ source upstream, không dùng package Noctalia native.
- Quyết định hiện tại: resolve upstream `main` khi cài; **không last-good, không
  rollback**. Đây là rủi ro đã được người dùng chấp nhận; không tự đổi sang lockfile.
- Lockfile hiện chỉ phục vụ lệnh `build`/snapshot, không phải nguồn cài mặc định.

## 3. Prefix và tác động hệ thống

- Component cài vào `/usr/local`.
- Stable thiếu floor của wlroots 0.20, wayland-protocols 1.49, libinput 1.29,
  libdisplay-info 0.3, xkbcommon 1.10, libei 1.3, sdbus-c++ 2.x, WirePlumber 0.5.
  Các lib này build từ source vào `/opt/noctalia/deps`; không ghi đè `/usr`.
- Testing/sid dùng library hệ thống Debian.
- Không thay kernel, GPU driver/firmware, bootloader, Mesa, glibc, systemd, dbus,
  PipeWire core.
- Apt chỉ dùng repo Debian chính thức; không repo bên thứ ba, không `--nogpgcheck`.

## 4. UX

- Mặc định là wizard; `--yes` chạy hoàn toàn tự động với default đã ghi trong README.
- Sudo một lần qua kitty popup nếu có; fallback terminal.
- Greeter là opt-in `--greeter`; mặc định không đổi display manager, không bật
  greetd/Noctalia Greeter, không disable getty.
- Không tự migrate/xoá bản Noctalia cũ ở `/usr` trên host hiện tại. Test v1 trên VM
  sạch Debian stable/testing/sid.

## 5. Trạng thái engine

Đã port từ repo cũ: manifest, preflight APT/pkg-config/compiler, source cache theo SHA,
Meson build/staging/inventory, tests. Chưa có: deploy transaction, private dependency
builder cho stable, wizard, greeter setup, update/repair/uninstall, CI matrix và release.

## 6. Dotfiles, hardware và app catalog

- `dotfiles/manifest.toml` quản lý config cá nhân: Umbriel (keybinds + display include),
  Noctalia config/palette Catppuccin Mocha Sapphire, fcitx5-lotus, shell dotfiles.
- `config` copy file với backup/state; không ghi đè im lặng; dry-run in kế hoạch.
- `hardware` đọc GPU từ `/sys/class/drm`, chạy `umbriel outputs`, chọn mode cao nhất
  theo resolution rồi refresh; hiện tại DP-1 1920x1080@144.
- `config` sinh `~/.config/umbriel/display.toml`, Umbriel include file này.
- `apps/*.toml` là catalog app; hiện có fcitx5-lotus và Brave Origin Nightly.
  Chưa có engine cài app; đây là bước tiếp theo.
- Không đưa DNS/private value hoặc token vào repo public; module DNS/private sẽ có
  local override riêng.
