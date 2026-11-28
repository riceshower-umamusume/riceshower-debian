#!/usr/bin/env bash
set -Eeuo pipefail
# shellcheck source=/dev/null

# Method 2: replace Debian archive sources with unstable, apt update,
# apt full-upgrade, reboot, verify. Sid == unstable; no -security suite.

usage() {
  cat <<'USAGE'
Usage: sudo ./scripts/upgrade-to-sid.sh [--yes] [--reboot] [--dry-run]

Converts a Debian stable/testing install to Debian unstable (Sid).
USAGE
}

YES=0
REBOOT=0
DRY=0
for arg in "$@"; do
  case "$arg" in
    --yes|-y) YES=1 ;;
    --reboot) REBOOT=1 ;;
    --dry-run) DRY=1 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $arg" >&2; usage >&2; exit 2 ;;
  esac
done

# shellcheck disable=SC1091
source /etc/os-release
if [[ ${ID:-} != debian ]]; then
  echo "not Debian: ID=${ID:-unknown}" >&2
  exit 1
fi

is_debian_source() {
  grep -qE 'deb\.debian\.org|security\.debian\.org|ftp\.debian\.org' "$1"
}

has_unstable_source() {
  grep -qE '^Suites:.*\b(sid|unstable)\b|^[[:space:]]*deb[[:space:]].*[[:space:]](sid|unstable)([[:space:]]|$)' "$1"
}

has_stable_source() {
  grep -qE '^Suites:.*\b(trixie|bookworm|bullseye|stable|testing|forky)\b|^[[:space:]]*deb[[:space:]].*[[:space:]](trixie|bookworm|bullseye|stable|testing|forky)([[:space:]]|$)' "$1"
}

unstable_only=1
for file in /etc/apt/sources.list /etc/apt/sources.list.d/*.sources /etc/apt/sources.list.d/*.list; do
  [[ -f $file ]] || continue
  is_debian_source "$file" || continue
  if ! has_unstable_source "$file" || has_stable_source "$file"; then
    unstable_only=0
  fi
done

if [[ ${VERSION_CODENAME:-} == sid || ${VERSION_CODENAME:-} == unstable ]] && [[ $unstable_only -eq 1 ]]; then
  echo "already Debian Sid/unstable; nothing to do"
  exit 0
fi
if [[ $unstable_only -eq 1 ]]; then
  echo "already using Debian Sid/unstable apt sources; nothing to do"
  exit 0
fi
case ${VERSION_CODENAME:-} in
  trixie|bookworm|stable|forky|testing|sid|unstable) ;;
  *)
    echo "unsupported Debian codename: ${VERSION_CODENAME:-unknown}" >&2
    exit 1
    ;;
esac

if [[ $DRY -eq 1 ]]; then
  echo "dry-run: $PRETTY_NAME"
  echo "would back up Debian source files to /var/backups/upgrade-to-sid-<timestamp>"
  echo "would replace Debian archive sources with Suites: unstable"
  echo "would run: apt-get update && apt-get -y full-upgrade"
  echo "would verify /etc/debian_version contains sid or unstable"
  exit 0
fi

if [[ ${EUID} -ne 0 ]]; then
  exec sudo -- "$0" "$@"
fi

if [[ $YES -eq 0 ]]; then
  echo "About to convert $PRETTY_NAME to Debian unstable (Sid)."
  echo "This is one-way and can leave the system unbootable. Back up /home and data."
  read -r -p "Type YES to continue: " reply
  [[ $reply == YES ]] || { echo "aborted"; exit 1; }
fi

stamp=$(date +%Y%m%d-%H%M%S)
backup=/var/backups/upgrade-to-sid-$stamp
install -d -m 700 "$backup"
echo "backup: $backup"

disable_debian_sources() {
  local file base
  for file in /etc/apt/sources.list /etc/apt/sources.list.d/*.sources /etc/apt/sources.list.d/*.list; do
    [[ -f $file ]] || continue
    is_debian_source "$file" || continue
    base=$(basename "$file")
    [[ $base == debian-unstable.sources ]] && continue
    install -m 600 "$file" "$backup/$base"
    rm -f "$file"
    echo "disabled: $file"
  done
}

disable_debian_sources
cat > /etc/apt/sources.list.d/debian-unstable.sources <<'SOURCES'
Types: deb
URIs: http://deb.debian.org/debian
Suites: unstable
Components: main contrib non-free non-free-firmware
Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg
SOURCES
chmod 644 /etc/apt/sources.list.d/debian-unstable.sources

export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get -y full-upgrade \
  -o Dpkg::Options::=--force-confdef \
  -o Dpkg::Options::=--force-confold

# A package or cloud-init may recreate a stable sources file during the upgrade.
disable_debian_sources
apt-get update
apt-get -y full-upgrade \
  -o Dpkg::Options::=--force-confdef \
  -o Dpkg::Options::=--force-confold

# shellcheck disable=SC1091
source /etc/os-release
if [[ ${VERSION_CODENAME:-} != sid && ${VERSION_CODENAME:-} != unstable ]] \
  && ! grep -qiE 'sid|unstable' /etc/debian_version; then
  echo "upgrade finished but neither VERSION_CODENAME nor /etc/debian_version indicates Sid" >&2
  echo "VERSION_CODENAME=${VERSION_CODENAME:-unknown} debian_version=$(cat /etc/debian_version)" >&2
  echo "source backup: $backup" >&2
  exit 1
fi

echo "Debian Sid/unstable is active: $(cat /etc/debian_version)"
echo "sources backup: $backup"
if [[ $REBOOT -eq 1 ]]; then
  echo "rebooting into Sid"
  reboot
else
  echo "reboot to load the Sid kernel: sudo reboot"
fi
