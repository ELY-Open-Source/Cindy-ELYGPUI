#!/bin/sh
# Usage: scripts/sync-locales.sh <cindy checkout> <commit>
set -eu
cindy=$1
commit=$2
src=apps/desktop/src/renderer/i18n/locales
out=$(cd "$(dirname "$0")/.." && pwd)/locales
rm -rf "$out"
for file in $(git -C "$cindy" ls-tree -r --name-only "$commit" "$src"); do
  mkdir -p "$out/$(basename "$(dirname "$file")")"
  git -C "$cindy" show "$commit:$file" > "$out/${file#"$src"/}"
done
git -C "$cindy" rev-parse "$commit^{commit}" > "$out/COMMIT"
echo "locales from $(cat "$out/COMMIT"): $(find "$out" -name '*.json' | wc -l) files"
