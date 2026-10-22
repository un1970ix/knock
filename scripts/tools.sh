. "$(dirname -- "$0")/env.sh"
knock_mode=${1:-check}
case "$knock_mode" in
  install) set -- --allow-write=.cache/tools --allow-net=static.crates.io --allow-run=cargo,tar ;;
  check) set -- ;;
  *) echo 'Choose install or check.' >&2; exit 1 ;;
esac
exec deno run --no-config --no-prompt --allow-read=scripts/tools.ts,rust-toolchain.toml,tools,.cache/tools,.cache/cargo/bin --allow-env=CARGO_HOME "$@" scripts/tools.ts "$knock_mode"
