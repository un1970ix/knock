. "$(dirname -- "$0")/env.sh"
sh scripts/tools.sh check
for knock_lock in Cargo.lock tools/*.lock; do
  cargo audit --file "$knock_lock" --deny warnings
done
deno audit
