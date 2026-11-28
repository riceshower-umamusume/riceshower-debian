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
- Steam itself is binary-only; gamescope/mangohud/gamemode remain apt packages
  for now because building their full stacks is heavy; the source engine can
  add recipes later.

### Source-first
- Noctalia ecosystem: already built from source.
- starship: source recipe (cargo).
- Steam: not practical from source; use `steam-installer`.
- Gaming tools: apt until pinned source recipes and dependency sets are added.

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

## Verification expectations

- `install --dry-run` lists the profile packages and source prerequisites.
- On sid: `steam-installer`, `mangohud`, `gamemode`, `gamescope` resolve from
  apt once `contrib`/`non-free` and i386 are enabled.
- `starship --version` works after the source build; `zsh -ic 'echo $PROMPT'`
  initializes starship.
- GPU/hardware commands report the detected vendor and the 144Hz mode.
