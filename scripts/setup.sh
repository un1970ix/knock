. "$(dirname -- "$0")/env.sh"
mkdir -p .cache
if [ ! -x "$CARGO_HOME/bin/rustup" ]; then
  case "$(uname -s):$(uname -m)" in
    Darwin:arm64) knock_host=aarch64-apple-darwin ;;
    Darwin:x86_64) knock_host=x86_64-apple-darwin ;;
    Linux:x86_64) knock_host=x86_64-unknown-linux-gnu ;;
    Linux:aarch64) knock_host=aarch64-unknown-linux-gnu ;;
    *) echo 'Unsupported platform.' >&2; exit 1 ;;
  esac
  knock_url="https://static.rust-lang.org/rustup/archive/1.29.2/$knock_host/rustup-init"
  curl -fsSL "$knock_url" -o .cache/rustup-init
  curl -fsSL "$knock_url.sha256" -o .cache/rustup-init.sha256
  knock_expected=$(cut -d ' ' -f 1 .cache/rustup-init.sha256)
  knock_actual=$(openssl dgst -sha256 .cache/rustup-init | awk '{print $NF}')
  [ "$knock_expected" = "$knock_actual" ] || { echo 'Rust installer checksum mismatch.' >&2; exit 1; }
  chmod 700 .cache/rustup-init
  .cache/rustup-init -y --no-modify-path --profile minimal --default-toolchain 1.99.1 --component rustfmt,clippy --target wasm32-unknown-unknown
fi
"$CARGO_HOME/bin/rustup" toolchain install 1.99.1 --profile minimal --component rustfmt,clippy --target wasm32-unknown-unknown
export PATH="$CARGO_HOME/bin:$PATH"
cargo fetch --locked
sh scripts/tools.sh install
deno install --frozen
deno run --frozen --cached-only --no-prompt --allow-read=node_modules --allow-write=node_modules/wrangler/wrangler-dist/cli.js scripts/patch-wrangler.ts install
