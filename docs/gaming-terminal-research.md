# Gaming, GPU and terminal research (Debian sid)

Date: 2026-11-28. Scope: what a fresh Debian sid gaming + terminal machine
needs, which parts can be built from source, and what this repo should automate.

## Sources

- Debian package `steam-installer` (sid): <https://packages.debian.org/sid/steam-installer>
  and `steam` transitional package. Steam is in `contrib`/`non-free`, needs
  i386 multiarch and 32-bit GL/Vulkan libraries.
- Debian package pages: `starship` is in Debian 13+ main;
  <https://sources.debian.org/src/starship/1.22.1-1/>.
- Starship guide: <https://starship.rs/guide/> — `apt install starship` on
  Debian 13+, `cargo install starship --locked` from source, then
  `eval "$(starship init zsh)"`.
- ArchWiki Gamescope: <https://wiki.archlinux.org/title/Gamescope> — Steam
  launch option `gamescope -W 1920 -H 1080 -r 144 -- %command%`, `--mangoapp`
  for MangoHud, `--adaptive-sync`, `--hdr-enabled`.
- Hacker News:
  <https://news.ycombinator.com/item?id=44364874> (Starship),
  <https://news.ycombinator.com/item?id=46563853> (zsh+starship+plugins),
  <https://news.ycombinator.com/item?id=47611694> (Debian lag vs gaming distros).
- Reddit: r/linux_gaming "How's your gaming experience on Debian Stable?",
  r/debian post-install threads (sources, sudo, firmware, firewall).
- Vietnamese: <https://vi.windowsnoticias.com/Ch%C6%A1i-game-tr%C3%AAn-Linux%3A-Hi%E1%BB%87u-n%C4%83ng-th%E1%BB%B1c--kh%E1%BA%A3-n%C4%83ng-t%C6%B0%C6%A1ng-th%C3%ADch-v%C3%A0-Proton/>
  (Proton/Steam), <https://congdonglinux.com/cai-steam-tren-ubuntu-24-04/>
  (Steam, i386, Proton), Steamworks Vietnamese Proton docs.
- Debian Steam install guides: GoLinuxCloud, LinuxCapable, LinuxConfig
  (enable `contrib non-free`, `dpkg --add-architecture i386`, install
  `steam-installer`).
- Debian post-install: FossLinux 2026 guide, mutschler.dev Trixie, Big Iron
  microcode guide (firmware + microcode from `non-free-firmware`).

## Decisions for this repo

### System/base
- Enable `contrib` and `non-free` in the Debian deb822 sources (keep
  `non-free-firmware`), then `apt-get update`.
- Install firmware and the CPU microcode matching the host
  (`intel-microcode` on GenuineIntel, `amd64-microcode` on AuthenticAMD).
- Install `earlyoom`, `zram-tools`, `timeshift`, `build-essential`, `curl`,
  `wget`, `git`, `ca-certificates`, `gnupg`, `btop`, `fastfetch`.
- Do not run `apt full-upgrade` automatically; it is destructive on a live
  system. Document it as the first manual step.

### Terminal
- zsh + starship + zsh-autosuggestions + zsh-syntax-highlighting +
  zsh-completions, fzf, eza, bat, fd-find, ripgrep, zoxide, btop, fastfetch.
- Starship is built from source with cargo (the repo's source-first policy);
  the Debian `starship` package is the fallback if cargo is unavailable.
- Dotfiles: `~/.zshrc`, `~/.config/starship.toml`, `~/.bashrc` starship line.

### Gaming/GPU
- i386 multiarch and 32-bit Mesa/Vulkan for Steam.
- `steam-installer`, `steam-devices`, `gamescope`, `mangohud`, `gamemode`,
  `lutris`, `wine`, `winetricks`, `vulkan-tools`.
- GPU packages come from hardware detection (`firmware-amd-graphics`,
  `mesa-vulkan-drivers`, `vulkan-tools`; Intel equivalent; NVIDIA driver).
- Set `MANGOHUD=1` and document Steam launch options
  (`gamescope -W 1920 -H 1080 -r 144 --mangoapp -- %command%`).
- Steam itself is binary-only; Gamescope, MangoHud and GameMode now have pinned
  source recipes too. Gamescope needs explicit submodules (`wlroots`, `libliftoff`,
  `vkroots`, `libdisplay-info`, `openvr`, `reshade`, `SPIRV-Headers`), so the
  source cache allows submodules only when the tool manifest opts in. If a
  source tool fails, the apt package stays in place and the install continues.

### Source-first
- Noctalia ecosystem: already built from source.
- starship: source recipe (cargo).
- Steam: not practical from source; use `steam-installer`.
- Gamescope 3.17.0, MangoHud v0.8.4, GameMode 1.8.2: pinned source builds into
  `/usr/local`, with their Debian packages kept as the fallback/runtime layer.
- Build deps for these three live in the `gaming` profile; apt remains the
  fallback on stable if a source recipe cannot build.

## Debian packages used

Base: `firmware-linux`, `firmware-misc-nonfree`, `earlyoom`, `zram-tools`,
`timeshift`, `build-essential`, `curl`, `wget`, `git`, `ca-certificates`,
`gnupg`, `btop`, `fastfetch`, plus CPU microcode.

Terminal: `zsh`, `starship`, `zsh-autosuggestions`, `zsh-syntax-highlighting`,
`zsh-completions`, `fzf`, `eza`, `bat`, `fd-find`, `ripgrep`, `zoxide`,
`btop`, `fastfetch`.

Gaming: `steam-installer`, `steam-devices`, `gamescope`, `mangohud`,
`gamemode`, `lutris`, `wine`, `winetricks`, `vulkan-tools`,
`mesa-vulkan-drivers`, `libvulkan1`, `libgl1-mesa-dri`, `libglx-mesa0`,
`libgl1-mesa-dri:i386`, `libglx-mesa0:i386`.

## Community findings (what was filtered in)

- Hacker News Starship threads: the common setup is a minimal zsh plus Starship,
  zsh-autosuggestions and zsh-syntax-highlighting, not a heavy framework.
  <https://news.ycombinator.com/item?id=44364874>,
  <https://news.ycombinator.com/item?id=46563853>
- Hacker News Linux-gaming threads: Debian stable works for Steam/Proton but is
  behind on Mesa/kernel features; Debian sid is the right place for the newest
  GPU stack. <https://news.ycombinator.com/item?id=47611694>,
  <https://news.ycombinator.com/item?id=42633578>
- Reddit r/linux_gaming: Debian Steam needs `contrib`/`non-free`, i386 and the
  32-bit GL/Vulkan libraries; testing/sid reduce driver-age problems.
  <https://www.reddit.com/r/linux_gaming/comments/nzjssd/hows_your_gaming_experience_on_debian_stable/>,
  <https://www.reddit.com/r/linux_gaming/comments/14zsfxf/do_steam_games_work_well_on_debian_12/>
- Reddit r/debian post-install threads: enable the right sources, install
  firmware/microcode, a firewall/timeshift, and avoid blindly dist-upgrading a
  working desktop. <https://www.reddit.com/r/debian/comments/18ixlej/what_do_you_do_post_debian_install/>
- Vietnamese sources: Proton/Steam on Linux and the i386/Steam installer steps.
  <https://vi.windowsnoticias.com/Ch%C6%A1i-game-tr%C3%AAn-Linux%3A-Hi%E1%BB%87u-n%C4%83ng-th%E1%BB%B1c--kh%E1%BA%A3-n%C4%83ng-t%C6%B0%C6%A1ng-th%C3%ADch-v%C3%A0-Proton/>,
  <https://congdonglinux.com/cai-steam-tren-ubuntu-24-04/>,
  <https://partner.steamgames.com/doc/steamhardware/proton?language=vietnamese>
- ArchWiki Gamescope: Steam launch flags, `--mangoapp` (not MangoHud directly),
  `--adaptive-sync`, `--hdr-enabled`, `setcap CAP_SYS_NICE` caveat.
  <https://wiki.archlinux.org/title/Gamescope>
- Filtered out: Flatpak Steam as default (repo policy is apt/system integration),
  automatic `apt full-upgrade` on a live machine, third-party GPU PPAs, and
  heavyweight zsh frameworks. These are documented as manual choices instead.

## Verification expectations

- `install --dry-run` lists the profile packages and source prerequisites.
- On sid: `steam-installer`, `mangohud`, `gamemode`, `gamescope` resolve from
  apt once `contrib`/`non-free` and i386 are enabled.
- Source builds verified in a clean Debian sid VM: Gamescope 3.17.0 (75 tests),
  MangoHud v0.8.4 (1 test), GameMode 1.8.2 (1 test), all installed to a staging
  prefix.
- Host GPU detection verified on an AMD Radeon RX 580 2048SP: `hardware` reports
  `gpu: amd`, DP-1 1920x1080@144, and Vulkan reports RADV/Mesa 26.2.7.
- `starship --version` works after the source build; `zsh -ic 'echo $PROMPT'`
  initializes starship.
- GPU/hardware commands report the detected vendor and the 144Hz mode.
