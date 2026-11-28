# Verification log

Date: 2026-11-28. This file records live commands and outputs, not design
intent. The VM is disposable; the host commands are read-only.

## Host GPU and display (real hardware)

```
$ lspci -nn | grep VGA
03:00.0 VGA compatible controller [0300]: Advanced Micro Devices, Inc. [AMD/ATI]
Polaris 20 XL/XTR [Radeon RX 580 2048SP / RX 590 GME] [1002:6fdf] (rev ef)

$ ./target/release/noctalia-setup hardware
gpu: amd
recommended packages: firmware-amd-graphics, mesa-vulkan-drivers, vulkan-tools
  DP-1: 1920x1080@144
suggested display config:
[output.DP-1]
mode = "1920x1080@144"
position = [0, 0]
scale = 1.0
```

Vulkan was checked without installing anything on the host: `vulkan-tools` and
`libvulkan1` debs were downloaded to the session scratchpad, extracted, and run
with `LD_LIBRARY_PATH`:

```
GPU0:
  apiVersion    = 1.4.354
  driverVersion = 26.2.7
  deviceType    = PHYSICAL_DEVICE_TYPE_DISCRETE_GPU
  deviceName    = AMD Radeon RX 580 2048SP (RADV POLARIS10)
  driverName    = radv
  driverInfo    = Mesa 26.2.7-1
GPU1:
  deviceType    = PHYSICAL_DEVICE_TYPE_CPU
  deviceName    = llvmpipe (LLVM 22.1.8, 256 bits)
```

So `gpu=unknown` earlier was the GPU-less QEMU VM, not the detector. The host
detects AMD, picks the AMD firmware/Mesa/Vulkan packages and the 144Hz mode.

## Debian sid VM: full profile install

Clean Debian sid QEMU VM, 8 vCPU/12 GB, no GPU:

```
./install.sh install --yes
profiles: Debian base, Full workstation, Steam and gaming, Zsh + Starship terminal
profile packages (41): ... steam-installer steam-devices mangohud gamemode gamescope ...
source tools: starship
system: enabled apt components contrib, non-free
system: added architecture i386
install complete
```

Verified afterward:

```
/usr/local/bin/starship            -> starship 1.22.1 (source build)
/usr/local/bin/noctalia            -> noctalia v5.6.0 (2dfc0b5b6343)
/usr/local/bin/umbriel             -> umbriel 0.1.0 (a909081a934a)
/usr/local/bin/luciole             -> luciole 0.1.0 (ea29c1c65eeb)
steam-installer 1:1.0.0.87~ds-3    steampath /usr/games/steam
mangohud 0.8.4+dfsg-1+b2           gamemode 1.8.2-2+b2
gamescope 3.16.24+ds-2             vulkan-tools installed
libgl1-mesa-dri:i386 / libglx-mesa0:i386 / libvulkan1:i386 installed
~/.config/starship.toml, ~/.zshrc, ~/.config/environment.d/90-gaming.conf applied
```

Stable/testing dry-runs both print `blocking: no`.

## Source builds for gaming tools

Pinned recipes (in `manifest/tools/`):

| Tool | Version | Commit | Build/tests in sid VM |
| --- | --- | --- | --- |
| Gamescope | 3.17.0 | `e6674a98118ca7b098b98a3874b8f195ae1bf750` | 75 tests pass, install to staging OK |
| MangoHud | v0.8.4 | `992103e4fb744897826de04ea00a2f71e7018214` | 1 test pass, install to staging OK |
| GameMode | 1.8.2 | `c54d6d4243b0dd0afcb49f2c9836d432da171a2b` | 1 test pass, install to staging OK |

Gamescope needs submodules (`wlroots`, `libliftoff`, `vkroots`,
`libdisplay-info`, `openvr`, `reshade`, `SPIRV-Headers`). The source cache only
allows them when the tool manifest sets `allow_submodules = true`; otherwise the
existing submodule policy still fails the fetch.

The engine uses the same source/build path for these tools and keeps the apt
package as a fallback if a source build fails.

## Terminal

```
$ zsh -ic 'print -r -- $PROMPT; alias ls'
... starship prompt command ...
ls='eza --icons --group-directories-first'
```

The `zle` warning in a non-TTY `zsh -ic` is expected; interactive zsh loads
zsh-autosuggestions and zsh-syntax-highlighting normally.

## Automated checks

```
cargo fmt --all --check                  OK
cargo clippy --all-targets -- -D warnings OK
cargo test --all                         70 passed
```
