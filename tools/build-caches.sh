#!/usr/bin/env bash
# Build Vice Lens's caches from your own copy of the game (the same as
# "Build missing caches" in the app's Setup). See docs/caches.md.
#
#   tools/build-caches.sh /path/to/your/game/folder
set -euo pipefail
game=$(readlink -f "${1:?usage: tools/build-caches.sh GAME_FOLDER}")
exec "$(dirname "$(readlink -f "$0")")/../vice-lens.sh" --build-caches --game "$game"
