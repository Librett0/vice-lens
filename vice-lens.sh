#!/usr/bin/env bash
# Run Vice Lens, building it first when the sources are newer than the binary.
# Arguments pass through: --serve, --world X,Y --window, --make-map, ...
# The game folder comes from --game DIR, VICE_LENS_GAME, or the one chosen in
# Vice Lens. VICE_LENS_BUILD_PREFIX wraps the build (e.g. a memory cap).
set -euo pipefail
here=$(cd "$(dirname "$(readlink -f "$0")")" && pwd)
bin=$here/target/release/vice-lens
if [ ! -x "$bin" ] || [ -n "$(find "$here/crates" \( -name '*.rs' -o -path '*/lens/*' \) -newer "$bin" -print -quit)" ]; then
    ${VICE_LENS_BUILD_PREFIX:-} cargo build --release -p vice-lens --manifest-path "$here/Cargo.toml"
fi
exec "$bin" "$@"
