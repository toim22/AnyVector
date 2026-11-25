#!/bin/bash
# Builds AnyVector's Linux packages into dist/ (x86-64 only):
#
#   packaging/linux/build.sh                 # both
#   packaging/linux/build.sh deb             # dist/AnyVector-<version>-linux-amd64.deb
#   packaging/linux/build.sh appimage        # dist/AnyVector-<version>-linux-x86_64.AppImage
#
# The version is the one of Linux in the workspace Cargo.toml. A program built on the oldest system to
# support (ubuntu-22.04, as .github/workflows/build.yml does) runs on newer ones too.
#
# Needs dpkg-dev, squashfs-tools (and cargo, unless ANYVECTOR_BIN is set).
#   ANYVECTOR_BIN        a release build of the program to package, instead of building it
#   DIST                 the output folder instead of dist/
#   APPIMAGE_RUNTIME     the AppImage runtime (its checksum is checked); downloaded in CI when not set
#   SOURCE_DATE_EPOCH    the date of everything in the packages; the last commit's when not set. The
#                        same program, date, tools and carried libraries (appimage-libraries.txt) give
#                        the same packages, byte for byte.
set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
root=$(cd "$here/../.." && pwd)
id=io.github.toim22.AnyVector

# the official static runtime of AppImage type 2 (see appimage-runtime.txt for its licenses)
runtime_url=https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-x86_64
runtime_sha256=2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d

die() { echo "build.sh: $*" >&2; exit 1; }

want_deb=0 want_appimage=0
[ $# -gt 0 ] || set -- deb appimage
for what in "$@"; do
  case $what in
    deb) want_deb=1 ;;
    appimage) want_appimage=1 ;;
    *) die "unknown package '$what' (deb, appimage)" ;;
  esac
done

# `linux = "x.y.z"` of [workspace.metadata.versions]
version=$(sed -n '/^\[workspace\.metadata\.versions\]/,/^\[workspace\.package\]/s/^linux *= *"\([^"]*\)".*/\1/p' "$root/Cargo.toml")
[ -n "$version" ] || die "no version for linux in Cargo.toml"

dist=${DIST:-$root/dist}
mkdir -p "$dist"
dist=$(cd "$dist" && pwd)

if [ -z "${SOURCE_DATE_EPOCH:-}" ]; then
  SOURCE_DATE_EPOCH=$(git -C "$root" log -1 --format=%ct 2>/dev/null || true)
fi
[ -n "$SOURCE_DATE_EPOCH" ] || die "not a git checkout: set SOURCE_DATE_EPOCH"
export SOURCE_DATE_EPOCH
export LC_ALL=C TZ=UTC
umask 022

if [ -z "${ANYVECTOR_BIN:-}" ]; then
  (cd "$root" && cargo build --locked --release -p anyvector)
  target=${CARGO_TARGET_DIR:-$root/target}
  case $target in /*) ;; *) target=$root/$target ;; esac
  ANYVECTOR_BIN=$target/release/anyvector
fi
[ -f "$ANYVECTOR_BIN" ] || die "no program at $ANYVECTOR_BIN"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# a data file, with LF lines whatever the checkout has
text() { # source, destination
  install -d "$(dirname "$2")"
  sed 's/\r$//' "$1" >"$2"
  chmod 644 "$2"
}

# what both packages install, in the folder $1
stage() {
  local dir=$1 doc=$1/usr/share/doc/anyvector size
  install -Dm755 "$ANYVECTOR_BIN" "$dir/usr/bin/anyvector"
  text "$here/$id.desktop" "$dir/usr/share/applications/$id.desktop"
  text "$here/$id.mime.xml" "$dir/usr/share/mime/packages/$id.xml"
  text "$here/$id.metainfo.xml" "$dir/usr/share/metainfo/$id.metainfo.xml"
  for size in "$here"/icons/hicolor/*/apps; do
    install -Dm644 "$size/$id.png" "$dir/usr/share/icons/hicolor/$(basename "$(dirname "$size")")/apps/$id.png"
  done
  text "$root/crates/anyvector/assets/brand/anyvector-icon.svg" "$dir/usr/share/icons/hicolor/scalable/apps/$id.svg"
  # the licenses of what is inside the program; palettes.rs and MPL-2.0.txt as the Windows setup
  # installs them (the source of AnyVector's one MPL-2.0 file, and that license)
  text "$here/copyright" "$doc/copyright"
  text "$root/crates/anyvector/assets/legal/third-party-licenses.txt" "$doc/legal/third-party-licenses.txt"
  text "$root/crates/anyvector/assets/legal/MPL-2.0.txt" "$doc/legal/MPL-2.0.txt"
  text "$root/crates/anyvector/assets/legal/Selawik-OFL.txt" "$doc/legal/Selawik-OFL.txt"
  text "$root/crates/vector-io/src/corel/palettes.rs" "$doc/legal/palettes.rs"
}

# everything in the folder $1 dated SOURCE_DATE_EPOCH
date_all() { find "$1" -print0 | xargs -0r touch --no-dereference --date="@$SOURCE_DATE_EPOCH"; }

build_deb() {
  local dir=$work/deb out=$dist/AnyVector-$version-linux-amd64.deb depends
  stage "$dir"
  # "Open with"; not in the AppImage, which is not installed
  text "$here/zz-$id-open.desktop" "$dir/usr/share/applications/zz-$id-open.desktop"
  text "$here/lintian-overrides" "$dir/usr/share/lintian/overrides/anyvector"
  {
    echo "anyvector ($version) stable; urgency=medium"
    echo
    echo "  * AnyVector $version. What is new:"
    echo "    https://github.com/toim22/AnyVector/releases/tag/v$version-linux"
    echo
    echo " -- $(sed -n 's/^Maintainer: //p' "$here/control.in" | tr -d '\r')  $(date -R -d "@$SOURCE_DATE_EPOCH")"
  } | gzip -9n >"$dir/usr/share/doc/anyvector/changelog.gz"
  chmod 644 "$dir/usr/share/doc/anyvector/changelog.gz"

  # the libraries the program is linked with; dpkg-shlibdeps wants to be in a source package. The
  # ones it opens at run time are listed in control.in.
  mkdir -p "$work/shlibs/debian"
  printf 'Source: anyvector\n\nPackage: anyvector\nArchitecture: amd64\n' >"$work/shlibs/debian/control"
  depends=$(cd "$work/shlibs" && dpkg-shlibdeps -O "$dir/usr/bin/anyvector" | sed -n 's/^shlibs:Depends=//p')
  [ -n "$depends" ] || die "dpkg-shlibdeps found no libraries"

  install -d "$dir/DEBIAN"
  # blank lines and comments are for the reader of control.in only
  sed -e 's/\r$//' -e '/^#/d' -e '/^$/d' \
    -e "s/@VERSION@/$version/" -e "s/@SHLIBS@/$depends/" \
    -e "s/@SIZE@/$(du -k -s --apparent-size "$dir/usr" | cut -f 1)/" \
    "$here/control.in" >"$dir/DEBIAN/control"
  (cd "$dir" && find usr -type f -print0 | sort -z | xargs -0 md5sum) >"$dir/DEBIAN/md5sums"
  chmod 644 "$dir/DEBIAN/control" "$dir/DEBIAN/md5sums"
  date_all "$dir"
  # xz: older systems cannot read Ubuntu's default, zstd
  dpkg-deb --root-owner-group -Zxz --build "$dir" "$out"
}

runtime() {
  local file=${APPIMAGE_RUNTIME:-}
  if [ -z "$file" ]; then
    [ "${CI:-}" = true ] || die "set APPIMAGE_RUNTIME to $runtime_url (downloaded in CI only)"
    file=$work/runtime
    curl --fail --silent --show-error --location --retry 3 --output "$file" "$runtime_url"
  fi
  echo "$runtime_sha256  $file" | sha256sum --check --quiet >&2 || die "$file is not the AppImage runtime (wrong checksum)"
  cp "$file" "$work/runtime.checked"
}

# the libraries of appimage-libraries.txt and their licenses, in the AppImage's folder $1
libraries() {
  local dir=$1 legal=$1/usr/share/doc/anyvector/legal/appimage-libraries.txt soname package file
  {
    echo "The AppImage carries these libraries, unchanged, from packages of $(sed -n 's/^PRETTY_NAME="\(.*\)"/\1/p' /etc/os-release)."
    echo "AnyVector uses them only on a system that does not have its own. Their licenses:"
  } >"$legal"
  while read -r soname package; do
    case $soname in '' | '#'*) continue ;; esac
    file=/usr/lib/x86_64-linux-gnu/$soname
    [ -f "$file" ] && [ -f "/usr/share/doc/$package/copyright" ] || die "no $soname or its license: install $package"
    install -Dm644 "$file" "$dir/usr/lib/anyvector/$soname"
    {
      printf '\n==== %s: %s %s\n\n' "$soname" "$package" "$(dpkg-query -W -f '${Version}' "$package")"
      cat "/usr/share/doc/$package/copyright"
    } >>"$legal"
  done <"$here/appimage-libraries.txt"
  chmod 644 "$legal"
}

build_appimage() {
  local dir=$work/AnyVector.AppDir out=$dist/AnyVector-$version-linux-x86_64.AppImage
  runtime
  stage "$dir"
  text "$here/appimage-runtime.txt" "$dir/usr/share/doc/anyvector/legal/appimage-runtime.txt"
  libraries "$dir"
  # one desktop file here, for the programs that install AppImages: it opens AnyVector's own files
  sed -i 's/^Exec=anyvector$/Exec=anyvector %F/' "$dir/usr/share/applications/$id.desktop"
  {
    echo "MimeType=application/x-vector-drawing;"
    echo "X-AppImage-Version=$version"
  } >>"$dir/usr/share/applications/$id.desktop"
  # AppRun is the program itself: no script and no LD_LIBRARY_PATH, so what AnyVector starts
  # (xdg-open, lp, the mail program...) gets the system's environment
  ln -s usr/bin/anyvector "$dir/AppRun"
  ln -s "usr/share/applications/$id.desktop" "$dir/$id.desktop"
  ln -s "usr/share/icons/hicolor/256x256/apps/$id.png" "$dir/$id.png"
  ln -s "$id.png" "$dir/.DirIcon"
  date_all "$dir"
  # mksquashfs takes its own date from SOURCE_DATE_EPOCH
  mksquashfs "$dir" "$work/squashfs" -root-owned -noappend -comp zstd -Xcompression-level 19 -no-xattrs -no-progress -quiet
  cat "$work/runtime.checked" "$work/squashfs" >"$out"
  chmod 755 "$out"
}

if [ $want_deb = 1 ]; then build_deb; fi
if [ $want_appimage = 1 ]; then build_appimage; fi
ls -l "$dist"
