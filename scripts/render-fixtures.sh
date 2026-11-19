#!/usr/bin/env bash
#
# Render index.template against every fixture in tests/fixtures/ into <out_dir>/<site_url>/index.html.
#
# Each fixture is rendered through the real generator CLI with minification on, so the output is what
# the Lambda would upload. The generator reads and writes relative to ./buckets/, so rendering happens
# in a scratch directory and never touches the local buckets/.
#
# Usage:
#   scripts/render-fixtures.sh <out_dir>
#
# CI validates the output with vnu-jar; see .github/workflows/validate-template.yml for the pinned version and
# flags to run locally (needs Java).

set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <out_dir>" >&2
  exit 1
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "$1"
out_dir="$(cd "$1" && pwd)"

cargo build --quiet --manifest-path "$repo_root/Cargo.toml" --bin main
generator="$(cargo metadata --format-version 1 --no-deps --manifest-path "$repo_root/Cargo.toml" \
  | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')/debug/main"

work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

mkdir -p "$work_dir/buckets/generator"
cp "$repo_root/index.template" "$work_dir/buckets/generator/"

for fixture in "$repo_root"/tests/fixtures/*.json; do
  site_url="$(basename "$fixture" .json)"
  cp "$fixture" "$work_dir/buckets/generator/"
  (cd "$work_dir" && "$generator" --site-url "$site_url" --minify)
  mkdir -p "$out_dir/$site_url"
  cp "$work_dir/buckets/$site_url/index.html" "$out_dir/$site_url/"
done
