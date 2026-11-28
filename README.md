# riceshower-debian

Bộ cài đặt và quản lý hệ sinh thái **Noctalia trên Debian**, bao gồm Umbriel Wayland compositor, Noctalia Shell, portal, Luciole và Noctalia Greeter. Các component Noctalia được build từ source upstream; package hệ thống và dependency được quản lý bằng APT.

> **Phạm vi hỗ trợ:** Debian 13 (stable / trixie), Debian 14 (testing / forky) và Debian Sid (unstable). Không hỗ trợ Ubuntu, Linux Mint, Fedora, Arch hoặc các distro khác.

## Mục lục

- [Tổng quan](#tổng-quan)
- [Yêu cầu trước khi cài](#yêu-cầu-trước-khi-cài)
- [Cài đặt nhanh](#cài-đặt-nhanh)
- [Repo cài những gì](#repo-cài-những-gì)
- [Cấu hình hệ thống được thay đổi](#cấu-hình-hệ-thống-được-thay-đổi)
- [GPU và màn hình](#gpu-và-màn-hình)
- [Greeter, greetd và đăng nhập](#greeter-greetd-và-đăng-nhập)
- [Quản lý ứng dụng tùy chọn](#quản-lý-ứng-dụng-tùy-chọn)
- [Các lệnh](#các-lệnh)
- [Cập nhật, sửa chữa và gỡ cài đặt](#cập-nhật-sửa-chữa-và-gỡ-cài-đặt)
- [Nâng cấp Debian lên Sid](#nâng-cấp-debian-lên-sid)
- [Vị trí cài đặt và dữ liệu](#vị-trí-cài-đặt-và-dữ-liệu)
- [Kiến trúc repository](#kiến-trúc-repository)
- [Giới hạn và lưu ý an toàn](#giới-hạn-và-lưu-ý-an-toàn)

## Tổng quan

Repository cung cấp một installer viết bằng Rust, được khởi chạy qua `install.sh` (Bash). Installer thực hiện các bước chính:

1. Nhận diện Debian suite và kiểm tra phiên bản được hỗ trợ.
2. Phân tích manifest để tạo kế hoạch component, package và profile.
3. Kiểm tra compiler, build tools, package, `pkg-config` và version dependency.
4. Chuẩn bị APT, architecture và CPU microcode theo profile.
5. Cài package cần thiết bằng APT.
6. Build các dependency riêng trên Debian stable khi thư viện hệ thống chưa đạt version yêu cầu.
7. Tải source theo commit SHA, build và staging component.
8. Deploy component vào `/usr/local`, lưu inventory và trạng thái.
9. Áp dụng dotfiles, cấu hình màn hình được phát hiện, DNS drop-in và Zsh.

Manifest là nguồn dữ liệu chính cho component, distro, package và profile. Các đường dẫn manifest được liệt kê ở phần [Kiến trúc repository](#kiến-trúc-repository).

## Yêu cầu trước khi cài

- Một bản Debian được hỗ trợ ở trên.
- Tài khoản người dùng thông thường có quyền `sudo`. **Không chạy installer trực tiếp bằng root.**
- Git.
- Rust toolchain có Cargo (MSRV khai báo trong `Cargo.toml`: Rust 1.80). `install.sh` cần Cargo để build/chạy CLI nếu chưa có binary release tại `target/release/noctalia-setup`; installer không tự bootstrap Rust.
- Kết nối Internet để lấy package và source upstream.
- Dung lượng đĩa đủ cho compiler, source cache, build tree và staging.
- APT sources Debian đang hoạt động.

Installer dùng quyền sudo cho các thao tác hệ thống. Khi gọi qua `install.sh`, chương trình từ chối chạy bằng root; nếu được gọi qua sudo từ một user, nó chuyển lại về user ban đầu.

## Cài đặt nhanh

Clone repository và chạy bằng user thông thường:

```bash
git clone https://github.com/riceshower-umamusume/riceshower-debian.git
cd riceshower-debian

# Kiểm tra hệ thống và xem thông tin wizard (chỉ đọc)
./install.sh

# Xem trước toàn bộ kế hoạch, không thay đổi hệ thống
./install.sh install --dry-run

# Cài tất cả component mặc định và cả ba profile
./install.sh install --yes

# Tùy chọn: cài thêm binary Noctalia Greeter
./install.sh install --yes --greeter
```

Lệnh `./install.sh` không có subcommand là màn hình tổng quan/định hướng, không phải wizard tương tác nhiều bước. Nó hiển thị distro, GPU, màn hình, DNS và app catalog; sau đó hướng dẫn chạy `install --dry-run` và `install --yes`.

`install --yes` thực hiện cài đặt thật. Không có `--yes`, lệnh install thật sẽ dừng; hãy dùng `--dry-run` để xem kế hoạch trước.

## Repo cài những gì

### 1. Năm component Noctalia

| Component | Vai trò | Build system |
| --- | --- | --- |
| [Umbriel](https://github.com/noctalia-dev/umbriel) | Wayland compositor, quản lý output và cửa sổ | Meson / Ninja |
| [Noctalia Shell](https://github.com/noctalia-dev/noctalia) | Desktop shell và giao diện Noctalia | Meson / Ninja |
| [xdg-desktop-portal-umbriel](https://github.com/noctalia-dev/xdg-desktop-portal-umbriel) | XDG Desktop Portal tích hợp Umbriel | Meson / Ninja |
| [Luciole](https://github.com/noctalia-dev/luciole) | File manager | Meson / Ninja |
| [Noctalia Greeter](https://github.com/noctalia-dev/noctalia-greeter) | Greeter dành cho greetd | Meson / Ninja |

Các component được build từ source upstream. Commit cài đặt mặc định được lấy từ `manifest/lock/ecosystem.lock.toml`, không tự động trỏ theo HEAD mới nhất của `main`. Dùng `update --latest` để resolve commit mới và cập nhật lockfile trước khi build/deploy lại.

Các build dependency gồm toolchain C/C++ (C++20/C++23 tùy component), Meson, Ninja, `just`, `pkg-config`, Wayland scanner và các thư viện phát triển liên quan đến Wayland, wlroots, libinput, xkbcommon, DRM, XCB, Cairo, Pango, GLib, PipeWire, Polkit, font/rendering, JSON và TOML. Danh sách version floor và package mapping chính xác nằm trong:

- `manifest/components/*.toml`
- `manifest/distros/debian-*.toml`

### 2. Các profile mặc định

Nếu không chỉ định `--profile`, installer dùng profile `all`, bao gồm `base`, `terminal` và `gaming`.

#### Base

| Package / nhóm | Mục đích |
| --- | --- |
| `firmware-linux`, `firmware-misc-nonfree` | Firmware phần cứng |
| CPU microcode | Microcode theo hãng CPU được phát hiện |
| `earlyoom` | Xử lý tình trạng thiếu bộ nhớ |
| `zram-tools` | RAM nén |
| `timeshift` | Snapshot hệ thống |
| `build-essential` | Toolchain build cơ bản |
| `curl`, `wget`, `git`, `ca-certificates`, `gnupg` | Công cụ tải và xác minh nguồn |
| `btop`, `fastfetch` | Theo dõi và hiển thị thông tin hệ thống |

Profile này bật APT components `contrib` và `non-free`.

#### Terminal

- Zsh, Zsh autosuggestions và syntax highlighting.
- Starship prompt (có APT package fallback; installer có recipe build Starship từ source bằng Cargo).
- fzf, eza, bat, fd-find, ripgrep và zoxide.
- Cargo, rustc và CMake phục vụ build.

Sau khi áp dụng cấu hình, installer đặt Zsh làm login shell nếu `/usr/bin/zsh` tồn tại. Cần đăng xuất/đăng nhập lại hoặc chạy `exec zsh`.

Phím tắt được cấu hình trong Zsh:

| Phím | Tác vụ |
| --- | --- |
| Ctrl-R | Tìm lịch sử bằng fzf |
| Ctrl-T | Chọn file bằng fzf |
| Alt-C | Chọn thư mục bằng fzf |
| Right / End | Chấp nhận gợi ý autosuggestion |
| Tab | Completion |
| `z <dir>` / `zi` | Điều hướng với zoxide |

#### Gaming

Các package và thư viện chính:

- Steam: `steam-installer`, `steam-devices`.
- Lutris, Wine và Winetricks.
- Gamescope, MangoHud và GameMode.
- Vulkan tools, Mesa Vulkan, Vulkan loader và Mesa OpenGL.
- Các gói Mesa/Vulkan i386 để hỗ trợ game 32-bit.
- Bộ thư viện phát triển cần thiết để build Gamescope, MangoHud, GameMode và các component desktop.

Profile bật APT `contrib`/ `non-free`, thêm kiến trúc `i386`, sau đó chạy `apt-get update`. Các source tool Gamescope, MangoHud và GameMode có recipe build từ source; package APT được giữ làm phương án dự phòng. Danh sách đầy đủ, bao gồm các gói `-dev`, nằm trong `profiles/gaming.toml`.

### 3. Source tools

Ngoài năm component Noctalia, profile có thể build các tool từ source:

| Tool | Nguồn / cách build |
| --- | --- |
| Starship | Rust / Cargo, commit pin trong `manifest/tools/starship.toml` |
| Gamescope | Source recipe trong app/tool manifest |
| MangoHud | Source recipe trong app/tool manifest |
| GameMode | Source recipe trong app/tool manifest |

Build tool trong quá trình cài profile có thể thất bại mà không làm mất package APT fallback; installer in cảnh báo và tiếp tục.

### 4. Dependency riêng cho Debian stable

Debian stable có thể thiếu một số version thư viện cần thiết. Installer build dependency riêng vào:

```text
/opt/noctalia/deps
```

Nhóm này gồm Wayland/wlroots stack, Wayland protocols, libinput, xkbcommon, pixman, libdrm, libdisplay-info, libei, sdbus-c++ và WirePlumber theo pin trong `manifest/private/*.toml`.

Các dependency riêng không được cài đè vào `/usr`. Debian testing và Sid dùng package/thư viện hệ thống theo distro manifest.

### 5. App catalog (cài riêng, không nằm trong profile mặc định)

| ID | Ứng dụng | Nguồn |
| --- | --- | --- |
| `brave-origin-nightly` | Brave Origin Nightly | APT repository Nightly của Brave |
| `fcitx5-lotus` | Fcitx5 Lotus | APT repository của Fcitx5 Lotus |

Hai ứng dụng này chỉ được cài khi gọi `install-app`. Cài chúng sẽ thêm APT source và keyring tương ứng; đây là repository bên thứ ba, khác với APT source Debian được dùng cho cài đặt mặc định.

App catalog engine hiện hỗ trợ các backend APT, Git/source build, Cargo, Flatpak per-user và AppImage có kiểm tra SHA-256. Catalog hiện tại chỉ khai báo hai ứng dụng APT ở bảng trên.

## Cấu hình hệ thống được thay đổi

### APT và kiến trúc

Tùy profile, installer có thể:

- Bật `contrib` và `non-free` trong Debian archive entries đang hoạt động.
- Thêm kiến trúc `i386` cho gaming.
- Chạy `apt-get update`.
- Cài CPU microcode phù hợp với CPU được phát hiện.
- Cài package GPU được khuyến nghị theo GPU phát hiện được.

Installer sửa các APT source Debian đang có; hãy kiểm tra cấu hình APT của máy trước khi chạy nếu bạn có mirror hoặc source tùy chỉnh.

### Dotfiles và shell

Các file trong `dotfiles/files/` được copy vào HOME theo `dotfiles/manifest.toml`, gồm:

- `~/.config/umbriel/`
- `~/.config/noctalia/`
- `~/.config/starship.toml`
- `~/.config/fcitx5/`
- `~/.config/environment.d/`
- `~/.zshrc`, `~/.bashrc`, `~/.profile`
- `~/.gitconfig`, `~/.npmrc`

Khi target đã tồn tại và nội dung khác, installer tạo backup trước khi ghi file. Trạng thái dotfiles được lưu riêng. Các file giống hệt nội dung nguồn được bỏ qua.

### DNS

Lệnh `config` và bước cuối của `install` đọc nameserver hiện tại từ `/run/systemd/resolve/resolv.conf` hoặc fallback `/etc/resolv.conf`, bỏ qua stub `127.0.0.53`, rồi ghi drop-in:

```text
/etc/systemd/resolved.conf.d/90-riceshower-debian.conf
```

Drop-in chứa danh sách DNS được phát hiện tại thời điểm chạy. Hãy kiểm tra DNS/resolved trên máy nếu bạn đang dùng NetworkManager, VPN hoặc DNS quản lý riêng.

### Noctalia privacy settings

Dotfile Noctalia hiện đặt:

- `external_ip_enabled = true`
- `telemetry_enabled = true`

Đây là lựa chọn cấu hình được repo cung cấp. Nếu không muốn các tính năng này, hãy chỉnh `dotfiles/files/noctalia/config.toml` trước khi chạy `config` hoặc cài đặt.

## GPU và màn hình

Installer nhận diện vendor GPU từ sysfs PCI vendor ID:

| Vendor | Package được đề xuất |
| --- | --- |
| AMD | `firmware-amd-graphics`, `mesa-vulkan-drivers`, `vulkan-tools` |
| Intel | `firmware-intel-graphics`, `intel-media-va-driver`, `mesa-vulkan-drivers`, `vulkan-tools` |
| NVIDIA | `nvidia-driver`, `nvidia-vulkan-icd` |
| Không nhận diện | Không thêm package GPU đề xuất |

Trong lệnh install, các package GPU đề xuất được thêm vào kế hoạch cài đặt theo kết quả phát hiện phần cứng, kể cả khi bạn chọn riêng một profile. Trên máy nhiều GPU, logic hiện tại lấy vendor đầu tiên được tìm thấy trong `/sys/class/drm`; hãy kiểm tra bằng `./install.sh hardware` và xem kế hoạch `--dry-run` trước khi cài.

Nếu Umbriel đang chạy, lệnh hardware đọc output/mode từ `umbriel outputs`. Khi áp dụng cấu hình, repo chọn mode có độ phân giải pixel cao nhất, sau đó ưu tiên refresh rate cao nhất, rồi tạo `~/.config/umbriel/display.toml`. Cấu hình màn hình mặc định trong repo có DP-1 1920x1080@144; cấu hình sinh ra từ phần cứng đang chạy có thể thay thế nội dung đó.

## Greeter, greetd và đăng nhập

Noctalia Greeter là component tùy chọn:

```bash
./install.sh install --dry-run --greeter
./install.sh install --yes --greeter
```

**Lưu ý quan trọng:** theo luồng cài đặt hiện tại, `--greeter` chọn build/deploy component Greeter và dependency `greetd`; nó không tự viết `/etc/greetd/config.toml`, không tự bật/chuyển display manager sang greetd và không disable getty. Sau khi cài, việc cấu hình greetd/PAM/session và kiểm tra đường đăng nhập vẫn là bước quản trị riêng. Không coi việc cài binary Greeter là đã hoàn tất cấu hình login manager.

Mặc định installer bỏ qua Noctalia Greeter và không đổi display manager.

## Quản lý ứng dụng tùy chọn

```bash
# Liệt kê app catalog
./install.sh list-apps

# Xem trước cài Brave Origin Nightly
./install.sh install-app brave-origin-nightly --dry-run

# Cài Brave Origin Nightly
./install.sh install-app brave-origin-nightly

# Xem trước/cài Fcitx5 Lotus
./install.sh install-app fcitx5-lotus --dry-run
./install.sh install-app fcitx5-lotus
```

Kiến trúc được lấy bằng `dpkg --print-architecture`. App install thành công được ghi vào:

```text
~/.local/state/noctalia-setup/apps.toml
```

Các backend source/Cargo/AppImage có quy tắc pin commit hoặc kiểm tra SHA-256. Source build recipe là lệnh shell tin cậy được duy trì trong repository, không phải sandbox.

## Các lệnh

Tất cả lệnh đều có thể nhận `--dry-run` ở cấp global để xem kế hoạch mà không thực hiện thay đổi được hỗ trợ bởi lệnh đó.

| Lệnh | Chức năng |
| --- | --- |
| `./install.sh` | Hiển thị thông tin hệ thống, hardware và app catalog |
| `./install.sh doctor` | Kiểm tra distro, manifest, lockfile, state và preflight |
| `./install.sh hardware` | Hiển thị GPU, output và mode |
| `./install.sh list-components` | Liệt kê component |
| `./install.sh list-apps` | Liệt kê ứng dụng tùy chọn |
| `./install.sh install --dry-run` | Xem kế hoạch cài đặt |
| `./install.sh install --yes` | Cài component và profile |
| `./install.sh install --yes --profile base` | Chỉ cài profile base cùng component |
| `./install.sh install --yes --profile terminal` | Chỉ cài profile terminal cùng component |
| `./install.sh install --yes --profile gaming` | Chỉ cài profile gaming cùng component |
| `./install.sh install --yes --greeter` | Cài thêm Greeter |
| `./install.sh build umbriel` | Build và staging component, không deploy |
| `./install.sh build --tool starship` | Build và staging source tool |
| `./install.sh build --tool starship --deploy` | Build và deploy source tool |
| `./install.sh config` | Áp dụng dotfiles, display config, DNS và Zsh |
| `./install.sh install-app <id>` | Cài app catalog theo ID |
| `./install.sh update` | Dùng commit pin hiện tại và cài/deploy lại |
| `./install.sh update --latest --yes` | Resolve upstream ref mới, cập nhật lockfile rồi cài/deploy |
| `./install.sh repair` | Chạy lại install plan theo lockfile hiện tại |
| `./install.sh uninstall --dry-run` | Xem trước file/package được gỡ |
| `./install.sh uninstall` | Gỡ file component được ghi nhận trong state |
| `./install.sh uninstall --purge` | Gỡ file component và yêu cầu APT remove package đã ghi nhận |
| `./install.sh boot --dry-run` | Xem kế hoạch cài/sửa GRUB |
| `./install.sh boot --yes` | Cài/sửa GRUB trên disk được phát hiện hoặc chỉ định |

Ví dụ chọn nhiều profile:

```bash
./install.sh install --dry-run --profile base,terminal
./install.sh install --yes --profile base,terminal
```

Lưu ý: profile chỉ chọn nhóm package/source tools; component Noctalia vẫn được chọn theo install plan. Component Greeter vẫn opt-in qua `--greeter`.

## Cập nhật, sửa chữa và gỡ cài đặt

### Update

- `update`: sử dụng commit SHA đang có trong lockfile, sau đó chạy lại install/deploy.
- `update --latest`: resolve commit mới từ upstream ref, cập nhật lockfile nếu có thay đổi rồi cài/deploy theo lockfile mới.
- `update --latest --dry-run`: xem commit mới có thể dùng, không ghi lockfile và không cài đặt.

### Repair

`repair` chạy lại install plan theo lockfile hiện tại, không bật Greeter mặc định. Dùng `--dry-run` trước nếu muốn xem kế hoạch.

### Uninstall

Uninstaller dựa trên state tại `/var/lib/noctalia-setup/state.toml`. Nó xóa các file component đã được ghi nhận và drop-in DNS của repo. `--purge` yêu cầu APT remove các package được ghi trong state.

Uninstall hiện không phải công cụ khôi phục toàn bộ máy về trạng thái ban đầu: dotfiles trong HOME, backup, app catalog installs/state, source/build cache và thay đổi login shell không được tự động hoàn nguyên. Kiểm tra kế hoạch và backup trước khi gỡ.

## Nâng cấp Debian lên Sid

Có script riêng, không chạy trong quá trình cài Noctalia:

```bash
# Chỉ xem thay đổi dự kiến
sudo ./scripts/upgrade-to-sid.sh --dry-run

# Xác nhận chuyển sang Sid
sudo ./scripts/upgrade-to-sid.sh --yes

# Chỉ dùng khi bạn chủ động muốn reboot sau nâng cấp
sudo ./scripts/upgrade-to-sid.sh --yes --reboot
```

Script backup Debian source files vào `/var/backups/upgrade-to-sid-<timestamp>`, thay Debian archive sources bằng Sid, chạy `apt-get update` và `apt-get full-upgrade`, sau đó kiểm tra suite. Đây là thay đổi lớn, có thể làm hệ thống không khởi động được; backup dữ liệu và đọc script trước khi chạy. Không dùng như bước cài desktop thông thường.

## Vị trí cài đặt và dữ liệu

| Đường dẫn | Nội dung |
| --- | --- |
| `/usr/local` | Binary, library và data của component/source tool được deploy |
| `/opt/noctalia/deps` | Dependency build riêng cho Debian stable |
| `/var/lib/noctalia-setup/state.toml` | State component/package và file inventory |
| `/var/lib/noctalia-setup/backups/` | Backup file cũ khi deploy component |
| `~/.cache/noctalia-setup/` | Source cache, build tree, staging và inventory |
| `~/.local/state/riceshower-debian/dotfiles.toml` | State áp dụng dotfiles |
| `~/.local/state/riceshower-debian/backups/` | Backup dotfiles trước khi thay |
| `~/.local/state/noctalia-setup/apps.toml` | State app catalog |
| `/etc/systemd/resolved.conf.d/90-riceshower-debian.conf` | DNS drop-in do lệnh config tạo |

## Kiến trúc repository

```text
.
├── install.sh
├── Cargo.toml
├── src/                       # Rust CLI, planner, preflight, build/deploy
├── manifest/
│   ├── ecosystem.toml         # Danh sách component/distro
│   ├── components/            # 5 component Noctalia
│   ├── distros/               # Debian stable/testing/sid và package map
│   ├── lock/                  # Commit SHA dùng cho cài đặt mặc định
│   ├── private/               # Dependency build riêng cho stable
│   └── tools/                 # Source tools như Starship
├── profiles/
│   ├── all.toml
│   ├── base.toml
│   ├── terminal.toml
│   └── gaming.toml
├── apps/
│   ├── *.toml                 # App catalog
│   └── keys/                  # Keyring nguồn APT ứng dụng
├── dotfiles/
│   ├── manifest.toml
│   └── files/                 # Umbriel, Noctalia, shell, Fcitx5...
├── scripts/
│   └── upgrade-to-sid.sh
└── docs/
    ├── design.md
    └── roadmap.md
```

Các file manifest/profile là nguồn chuẩn cho danh sách package và dependency. Khi danh sách thay đổi, hãy cập nhật manifest tương ứng thay vì chỉ sửa README.

## Giới hạn và lưu ý an toàn

- Chỉ hỗ trợ các suite Debian được khai báo trong manifest; không suy rộng sang distro khác.
- Build source upstream có rủi ro tương thích theo thời gian. Cài đặt mặc định dùng SHA pin; `--latest` chủ động cập nhật pin.
- Cài đặt đầy đủ cần mạng, thời gian build và dung lượng đáng kể.
- GPU detection hiện chọn vendor đầu tiên tìm thấy; máy hybrid/multi-GPU cần xem trước package plan.
- `--greeter` hiện không tự cấu hình greetd/PAM hoặc chuyển display manager.
- `config` có thể thay dotfiles, login shell và DNS drop-in; đọc các file trong `dotfiles/files/` trước khi áp dụng.
- Noctalia config hiện bật telemetry và external IP feature theo file cấu hình mặc định của repo.
- App catalog có thể thêm third-party APT source và keyring; chỉ cài ứng dụng mà bạn tin cậy.
- `boot --yes` thay đổi GRUB; `upgrade-to-sid.sh` thay đổi Debian suite. Hai thao tác này không thuộc cài desktop thông thường.
- Giữ backup và xem `--dry-run` trước các thao tác ghi hệ thống.

## Tài liệu liên quan

- [Thiết kế repository](docs/design.md)
- [Roadmap](docs/roadmap.md)
- [Component manifests](manifest/components/)
- [Debian distro manifests](manifest/distros/)
- [Install profiles](profiles/)
- [App catalog](apps/)
