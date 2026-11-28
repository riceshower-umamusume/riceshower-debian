#!/usr/bin/env bash
set -Eeuo pipefail
# shellcheck source=/dev/null

# Method 2 of https://www.atulhost.com/how-to-install-debian-unstable:
# point Debian sources at unstable, apt update, apt full-upgrade, reboot, verify.
# TheLinuxVault notes: Sid == unstable; no -security repository; back up first.

usage() {
  cat <<'USAGE'
Usage: sudo ./scripts/upgrade-to-sid.sh [--yes] [--reboot] [--dry-run]

Converts a Debian stable/testing install to Debian unstable (Sid) by replacing
Debian archive sources with unstable and running apt full-upgrade.
The script refuses to run if the system is already Sid/unstable.
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
if [[ ${VERSION_CODENAME:-} == sid || ${VERSION_CODENAME:-} == unstable ]]; then
  echo "already Debian Sid/unstable; nothing to do"
  exit 0
fi
case ${VERSION_CODENAME:-} in
  trixie|bookworm|stable|forky|testing) ;;
  *)
    echo "unsupported Debian codename: ${VERSION_CODENAME:-unknown}" >&2
    exit 1
    ;;
esac
if grep -RqsE '^Suites:.*\b(sid|unstable)\b|^[[:space:]]*deb[[:space:]].*[[:space:]](sid|unstable)([[:space:]]|$)' /etc/apt/sources.list /etc/apt/sources.list.d 2>/dev/null; then
  echo "already using Debian Sid/unstable apt sources; nothing to do"
  exit 0
fi

if [[ $DRY -eq 1 ]]; then
  echo "dry-run: $PRETTY_NAME"
  echo "would back up Debian source files to /var/backups/upgrade-to-sid-<timestamp>"
  echo "would replace Debian archive sources with:"
  echo "  Suites: unstable"
  echo "  Components: main contrib non-free non-free-firmware"
  echo "would run: apt-get update && apt-get -y full-upgrade"
  echo "would verify VERSION_CODENAME=sid and /etc/debian_version contains sid"
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

# Disable Debian archive sources, leave third-party sources alone.
for file in /etc/apt/sources.list /etc/apt/sources.list.d/*.sources /etc/apt/sources.list.d/*.list; do
  [[ -f $file ]] || continue
  if grep -qE 'deb\.debian\.org|security\.debian\.org|ftp\.debian\.org' "$file"; then
    install -m 600 "$file" "$backup/$(basename "$file")"
    rm -f "$file"
    echo "disabled: $file"
  fi
done

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

# shellcheck disable=SC1091
source /etc/os-release
if [[ ${VERSION_CODENAME:-} != sid && ${VERSION_CODENAME:-} != unstable ]]; then
  echo "upgrade finished but VERSION_CODENAME is '${VERSION_CODENAME:-unknown}', not sid" >&2
  echo "source backup: $backup" >&2
  exit 1
fi
if ! grep -qiE 'sid|unstable' /etc/debian_version; then
  echo "/etc/debian_version does not contain sid/unstable" >&2
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
