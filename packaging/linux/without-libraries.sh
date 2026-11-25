#!/bin/bash
# Runs a program as on a system without the libraries the AppImage carries (appimage-libraries.txt):
#
#   packaging/linux/without-libraries.sh program [argument...]
#
# They are hidden from that program only, in mount and user namespaces of its own. Nothing is
# changed on the system, and it doesn't need root.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
if [ -z "${ANYVECTOR_HIDDEN:-}" ]; then
  ANYVECTOR_HIDDEN=1 exec unshare --map-root-user --mount "${BASH_SOURCE[0]}" "$@"
fi
unset ANYVECTOR_HIDDEN

hidden=0
while read -r soname _; do
  case $soname in '' | '#'*) continue ;; esac
  for file in /usr/lib/x86_64-linux-gnu/"$soname" /usr/lib64/"$soname"; do
    if [ -e "$file" ]; then
      mount --bind /dev/null "$(readlink -f "$file")"
      hidden=$((hidden + 1))
    fi
  done
done <"$here/appimage-libraries.txt"
echo "without-libraries: $hidden hidden" >&2
exec "$@"
