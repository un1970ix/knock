. "$(dirname -- "$0")/env.sh"
export PATH="$knock_root/.cache/deno-bin:$CARGO_HOME/bin:$PATH"
case "${1:-}" in
  build)
    if ! command -v deno >/dev/null 2>&1 || [ "$(deno --version | head -n 1 | cut -d ' ' -f 2)" != "2.9.8" ]; then
      mkdir -p .cache/deno-bin
      knock_archive=deno-x86_64-unknown-linux-gnu.zip
      knock_url="https://github.com/denoland/deno/releases/download/v2.9.8/$knock_archive"
      curl -fsSL "$knock_url" -o ".cache/$knock_archive"
      curl -fsSL "$knock_url.sha256sum" -o .cache/deno.sha256sum
      knock_expected=$(cut -d ' ' -f 1 .cache/deno.sha256sum)
      knock_actual=$(openssl dgst -sha256 ".cache/$knock_archive" | awk '{print $NF}')
      [ "$knock_expected" = "$knock_actual" ] || { echo 'Deno checksum mismatch.' >&2; exit 1; }
      unzip -oq ".cache/$knock_archive" -d .cache/deno-bin
    fi
    sh scripts/setup.sh
    just deploy-check
    ;;
  publish)
    just wrangler publish
    ;;
  *) echo 'Choose build or publish.' >&2; exit 1 ;;
esac
