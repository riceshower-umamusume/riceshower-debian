# Dependency audit — upstream Noctalia tại commit pin

> Ngày audit: 2026-11-25
> Nguồn: checkout local tại đúng commit trong `manifest/lock/ecosystem.lock.toml`
> (`meson.build`, `lib/meson.build`, `meson_options.txt`).
> Mục đích: làm evidence cho Giai đoạn 0 của [roadmap](roadmap.md), không phải
> xác nhận package/version của từng distro.

## 1. Phạm vi và commit đã đọc

| Component | Commit | File đã đọc |
| --- | --- | --- |
| `umbriel` | `a909081a934a80bb7685080884ffaaa30c857e4c` | `meson.build`, `meson_options.txt` |
| `noctalia` | `2dfc0b5b6343165dc77dcd378717b4d372f791a5` | `meson.build`, `meson_options.txt` |
| `xdg-desktop-portal-umbriel` | `cb5829efccfcf4816c20c0ca0c95b89867ffc908` | `meson.build`, `lib/meson.build`, `meson_options.txt` |
| `luciole` | `ea29c1c65eeb8c99495ebfe4c55ea6eb0a61612b` | `meson.build`, `lib/meson.build`, `meson_options.txt` |
| `noctalia-greeter` | `ec778fcba85ea07e94213939d11baabcd009817d` | `meson.build`, `meson_options.txt` |

Lượt audit này phát hiện hai component dùng thư viện UI chung nhưng manifest chưa
khai báo đủ dependency trực tiếp. Đã bổ sung vào `build_dependencies`:

- `xdg-desktop-portal-umbriel`: `pango`, `freetype`, `fontconfig`, `harfbuzz`,
  `epoxy`, `xkbcommon`.
- `luciole`: `cairo`, `pango`, `freetype`, `fontconfig`, `harfbuzz`, `epoxy`.
- Thêm logical key `harfbuzz` vào package map của 7 distro imperative
  (`libharfbuzz-dev`, `harfbuzz-devel`, `harfbuzz`).

Không thêm dependency chỉ vì nó xuất hiện gián tiếp qua `pango`/`cairo`; các
module pkg-config đi kèm package đã có sẽ được preflight kiểm tra ở Giai đoạn 1.

## 2. Required dependency và version floor

Cột **logical key** là key dùng trong manifest; **pkg-config** là module upstream
thực sự gọi. Các dòng không ghi version là upstream không đặt floor.

### Umbriel

| Logical key | pkg-config | Ràng buộc | Ghi chú |
| --- | --- | --- | --- |
| `wayland-client` | `wayland-client` | — | |
| `wayland-server` | `wayland-server` | `>=1.24` | |
| `wayland-protocols` | `wayland-protocols` | `>=1.47` | |
| `wlroots` | `wlroots-0.20` | `>=0.20.1`, `<0.21.0` | ABI 0.20 |
| `xkbcommon` | `xkbcommon` | `>=1.10.0` | |
| `libinput` | `libinput` | `>=1.29` | |
| `libei` | `libeis-1.0` | `>=1.3` | pkg-config khác logical key |
| `pixman` | `pixman-1` | `>=0.43.0` | |
| `libdrm` | `libdrm` | `>=2.4.129` | |
| `libdisplay-info` | `libdisplay-info` | `>=0.3.0` | |
| `cairo` | `cairo` | — | |
| `pango` | `pango`, `pangocairo` | — | |
| `tomlplusplus` | `tomlplusplus` | — | |
| `nlohmann-json` | `nlohmann_json` | — | |
| `xcb`, `xcb-icccm`, `xcb-ewmh`, `xcb-shape`, `xcb-randr`, `xcb-render` | cùng tên | — | |

Optional/feature: `jemalloc` (glibc, auto), `localed` → `libsystemd >=243`,
`libudev` (auto), `lcms2` (auto), `tracy` (disabled mặc định), `tests`/`harness`
(feature; build option hiện tại tắt tests).

### Noctalia

| Logical key | pkg-config | Ràng buộc | Ghi chú |
| --- | --- | --- | --- |
| `sdbus-cpp` | `sdbus-c++` | — | |
| `wayland-client`, `wayland-server`, `wayland-protocols` | cùng tên | — | |
| `cairo` | `cairo` | — | |
| `pango` | `pango`, `pangocairo` | `pango >=1.50` | |
| `xkbcommon` | `xkbcommon` | — | |
| `glib` | `glib-2.0`, `gobject-2.0`, `gio-2.0` | — | một package dev cung cấp cả ba |
| `libsecret` | `libsecret-1` | `>=0.20` | |
| `libsodium` | `libsodium` | `>=1.0.18` | |
| `polkit-agent` | `polkit-agent-1` | — | |
| `polkit-gobject` | `polkit-gobject-1` | — | |
| `pipewire-dev` | `libpipewire-0.3` | — | |
| `wireplumber-dev` | `wireplumber-0.5` | — | tên module đã mã hóa ABI 0.5 |
| `curl` | `libcurl` | — | |
| `qalculate` | `libqalculate` | — | |
| `libxml` | `libxml-2.0` | — | |
| `md4c` | `md4c` | — | |
| `nlohmann-json` | `nlohmann_json` | — | |
| `tomlplusplus` | `tomlplusplus` | — | |
| `libical` | `libical` | — | |
| `webp` | `libwebp` | — | required |
| `sndfile` | `sndfile` | — | required |

Optional/feature: `jemalloc` (đang tắt bằng `-Djemalloc=disabled`), `tests`
(đang tắt), `native_optimizations` (mặc định false), `system_libraries` (mặc
định false, dùng bản vendored).

### xdg-desktop-portal-umbriel

| Logical key | pkg-config | Ràng buộc | Ghi chú |
| --- | --- | --- | --- |
| `sdbus-cpp` | `sdbus-c++` | `>=2.0` | |
| `pipewire-dev` | `libpipewire-0.3` | — | |
| `wayland-client` | `wayland-client`, `wayland-egl` | — | `libwayland-dev`/`wayland` cung cấp cả hai |
| `wayland-server` | `wayland-server` | — | |
| `wayland-protocols` | `wayland-protocols` | `>=1.39` | |
| `libdrm` | `libdrm` | — | |
| `gbm` | `gbm` | — | |
| `cairo` | `cairo`, `cairo-ft` | — | |
| `pango` | `pango >=1.50`, `pangocairo`, `pangoft2` | `pango >=1.50` | phần `lib/` (share picker) |
| `freetype` | `freetype2` | — | phần `lib/` |
| `fontconfig` | `fontconfig` | — | phần `lib/` |
| `harfbuzz` | `harfbuzz` | — | phần `lib/` |
| `epoxy` | `epoxy` | — | fallback khi không dùng EGL/GLES |
| `xkbcommon` | `xkbcommon` | — | phần `lib/` |
| `glib` | `glib-2.0` | — | |
| `tomlplusplus`, `nlohmann-json` | cùng tên | — | |

Test-only: `libei-1.0 >=1.3` (chỉ dựng khi tests bật). `picker` mặc định true nên
các dependency của `lib/` là bắt buộc. `tests` hiện tắt bằng `-Dtests=false`;
`egl`/`glesv2` optional và có fallback `epoxy`.

### Luciole

| Logical key | pkg-config | Ràng buộc | Ghi chú |
| --- | --- | --- | --- |
| `wayland-protocols` | `wayland-protocols` | `>=1.49` | |
| `glib` | `glib-2.0`, `gio-2.0`, `gio-unix-2.0` | `gio >=2.76` | |
| `curl` | `libcurl` | `>=7.85` | |
| `libxml` | `libxml-2.0` | `>=2.9` | |
| `gdk-pixbuf` | `gdk-pixbuf-2.0` | `>=2.42` | |
| `tomlplusplus` | `tomlplusplus` | `>=3.4` | |
| `xkbcommon` | `xkbcommon` | — | |
| `libarchive` | `libarchive` | — | |
| `liblzma` | `liblzma` | — | |
| `libacl` | `libacl` | — | |
| `libpng` | `libpng` | — | |
| `librsvg` | `librsvg-2.0` | — | |
| `nlohmann-json` | `nlohmann_json` | — | |
| `wayland-client` | `wayland-client`, `wayland-egl` | — | |
| `wayland-cursor` | `wayland-cursor` | — | |
| `cairo` | `cairo`, `cairo-ft` | — | phần `lib/` |
| `pango` | `pango >=1.50`, `pangocairo`, `pangoft2` | `pango >=1.50` | phần `lib/` |
| `freetype` | `freetype2` | — | phần `lib/` |
| `fontconfig` | `fontconfig` | — | phần `lib/` |
| `harfbuzz` | `harfbuzz` | — | phần `lib/` |
| `epoxy` | `epoxy` | — | fallback EGL/GLES |

Optional/feature: `tests` (đang tắt), `native_optimizations` (false),
`benchmarks` (false), `build_revision` (string), `egl`/`glesv2` optional.

### Noctalia Greeter

| Logical key | pkg-config | Ràng buộc | Ghi chú |
| --- | --- | --- | --- |
| `wayland-client` | `wayland-client` | — | |
| `wayland-server` | `wayland-server` | — | |
| `wayland-protocols` | `wayland-protocols` | — | |
| `xkbcommon` | `xkbcommon` | — | |
| `freetype` | `freetype2` | — | |
| `fontconfig` | `fontconfig` | — | |
| `cairo` | `cairo`, `cairo-ft` | — | |
| `pango` | `pango`, `pangocairo`, `pangoft2` | — | |
| `librsvg` | `librsvg-2.0` | — | |
| `libxml` | `libxml-2.0` | — | |
| `glib` | `glib-2.0`, `gobject-2.0`, `gio-2.0` | — | |
| `tomlplusplus` | `tomlplusplus` | — | |
| `nlohmann-json` | `nlohmann_json` | — | |
| `wlroots` | `wlroots-0.20` | — | ABI 0.20 |
| `libinput` | `libinput` | — | |
| `webp` | `libwebp` | — | required |
| `libjxl` | `libjxl`, `libjxl_threads` | — | required |
| `epoxy` | `epoxy` | — | fallback EGL/GLES; `egl`/`glesv2`/`wayland-egl` optional |

Feature: `native_optimizations` mặc định false. Greeter không gọi trực tiếp
`harfbuzz`; nó đi qua pango.

## 3. Build option đã đối chiếu

| Component | Build option hiện tại | Loại option upstream | Kết quả |
| --- | --- | --- | --- |
| `umbriel` | `-Dtests=disabled` | feature | hợp lệ |
| `noctalia` | `-Dtests=disabled`, `-Djemalloc=disabled` | feature | hợp lệ |
| `xdg-desktop-portal-umbriel` | `-Dtests=false` | boolean | hợp lệ |
| `luciole` | `-Dtests=disabled` | feature | hợp lệ |
| `noctalia-greeter` | không truyền option | — | hợp lệ |

`just` vẫn nằm trong `build_tools` của các component vì upstream có justfile; engine
Meson chưa được implement nên chưa thể kết luận có thể bỏ. Đây là mục còn mở của
Giai đoạn 0 (rà lại build_tools), không phải dependency link.

## 4. Chưa xác minh

- Tên package trong `package_map` mới chỉ được đối chiếu theo family; **chưa**
  xác minh version trong repository chính thức của từng distro. Không dùng tài
  liệu này để khẳng định một distro đạt Tier 1.
- Các floor trên là ràng buộc upstream; việc distro có package đủ mới hay không
  thuộc Giai đoạn 1 (preflight version/ABI).
- Các dependency optional (`libsystemd`, `libudev`, `lcms2`, `jemalloc`, `tracy`,
  `egl`/`glesv2`) chưa được đưa vào package plan; build hiện tại để Meson tự
  disable hoặc dùng fallback. Khi engine build được implement phải quyết định
  rõ required/optional thay vì dựa vào auto-detection.
- `harfbuzz` vừa được thêm vào package map; cần xác minh package tồn tại trên
  đúng bản distro trước khi tuyên bố hỗ trợ.

## 5. Bước tiếp theo

1. Đã encode bảng trên vào `build_requirements` của manifest và validate khi load.
2. Đã có preflight APT, pkg-config và compiler; xem `src/preflight.rs` và
   Giai đoạn 1 trong roadmap.
3. Tiếp theo: source acquisition/build/staging và xác minh package/version trên
   container/VM của từng distro.
