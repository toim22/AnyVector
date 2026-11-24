#!/bin/bash
# Starts AnyVector on a virtual X display and passes when its main window opens (a window of the
# desktop file's StartupWMClass, larger than the splash) and is still there a few seconds later:
#
#   packaging/linux/smoke-test.sh screenshot.png anyvector [file...]
#
# Needs xvfb, xdotool and imagemagick. Takes a home folder of its own, so the user's settings are
# neither read nor changed.
set -euo pipefail

class=io.github.toim22.AnyVector
shot=$1
shift

if [ -z "${ANYVECTOR_SMOKE:-}" ]; then
  home=$(mktemp -d)
  trap 'rm -rf "$home"' EXIT
  mkdir -m 700 "$home/run"
  env -u WAYLAND_DISPLAY -u XDG_CONFIG_HOME -u XDG_DATA_HOME -u XDG_CACHE_HOME -u XDG_STATE_HOME \
    HOME="$home" XDG_RUNTIME_DIR="$home/run" ANYVECTOR_SMOKE=1 \
    xvfb-run -a -s "-screen 0 1600x1000x24" "${BASH_SOURCE[0]}" "$shot" "$@"
  exit
fi

# the widest window of the program
width() {
  local w width best=0
  for w in $(xdotool search --onlyvisible --class "$class" 2>/dev/null); do
    width=$(xdotool getwindowgeometry --shell "$w" 2>/dev/null | sed -n 's/^WIDTH=//p')
    if [ "${width:-0}" -gt "$best" ]; then best=$width; fi
  done
  echo "$best"
}

"$@" &
pid=$!
trap 'kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true' EXIT

for ((i = 0; i < 60; i++)); do
  kill -0 "$pid" 2>/dev/null || { echo "smoke-test: the program ended before its window opened" >&2; exit 1; }
  # the splash is 620 wide
  if [ "$(width)" -gt 700 ]; then break; fi
  sleep 1
done
sleep 4
import -window root "$shot" || true
kill -0 "$pid" 2>/dev/null || { echo "smoke-test: the program ended" >&2; exit 1; }
[ "$(width)" -gt 700 ] || { echo "smoke-test: no main window after $i s" >&2; exit 1; }
echo "smoke-test: main window open after about $i s, $(width) wide"
