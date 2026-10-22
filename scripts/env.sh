set -eu
knock_root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
cd "$knock_root"
export CARGO_HOME="$knock_root/.cache/cargo"
export RUSTUP_HOME="$knock_root/.cache/rustup"
export PATH="$CARGO_HOME/bin:$PATH"
export XDG_CACHE_HOME="$knock_root/.cache"
export WRANGLER_SEND_METRICS=false
export WRANGLER_SEND_ERROR_REPORTS=false
export DO_NOT_TRACK=1
