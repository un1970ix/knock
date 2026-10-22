. "$(dirname -- "$0")/env.sh"
if [ "${1:-}" = check ]; then
  exec cargo clippy --locked --offline --target wasm32-unknown-unknown -- -D warnings
fi
sh scripts/tools.sh check
exec worker-build --release -- --locked --offline
