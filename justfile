set shell := ["sh", "-eu", "-c"]

# List available commands.
default:
    @just --list

# Install pinned toolchains and dependencies.
setup:
    sh scripts/setup.sh

# Format source files, documentation, and the justfile.
fmt:
    sh scripts/rust.sh fmt
    deno fmt
    just --unstable --fmt

# Check formatting, lint, types, native and WebAssembly code, and tests.
check:
    sh scripts/rust.sh fmt --check
    deno fmt --check
    just --unstable --fmt --check
    deno lint
    deno check --frozen --cached-only scripts/*.ts tests/*.ts
    sh scripts/rust.sh clippy --locked --offline --all-targets -- -D warnings
    sh scripts/rust.sh test --locked --offline
    sh scripts/build.sh check
    deno test --frozen --cached-only --no-prompt --allow-read=client tests/client_test.ts

# Build the optimized Worker in build/.
build:
    sh scripts/build.sh

# Build and serve locally at http://127.0.0.1:8787.
dev: build
    just wrangler dev

# Run Wrangler with mode check, dev, or publish.
wrangler mode:
    deno run --frozen --cached-only --no-prompt --allow-read=wrangler.jsonc,node_modules --allow-write=.cache/wrangler --allow-env=PATH,CLOUDFLARE_ACCOUNT_ID,CLOUDFLARE_API_TOKEN,WRANGLER_CI_MATCH_TAG --allow-run=deno --allow-sys=homedir scripts/wrangler.ts {{ mode }}

# Test the running local Worker with the README test configuration.
test-integration:
    deno run --frozen --cached-only --no-prompt --allow-net=127.0.0.1:8787 scripts/integration.ts

# Audit application and build-tool dependencies for known vulnerabilities.
audit:
    sh scripts/audit.sh

# Run checks, audits, a production build, and a deployment dry run.
deploy-check: check audit build
    just wrangler check

# Run release checks and publish to Cloudflare.
deploy: deploy-check
    just wrangler publish
