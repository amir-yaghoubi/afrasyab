#!/usr/bin/env bash
set -euo pipefail

TAG="${1:?usage: bump-workspace-version.sh v0.1.0}"
VERSION="${TAG#v}"

if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "invalid semver tag: $TAG" >&2
  exit 1
fi

CARGO_TOML="Cargo.toml"
CURRENT="$(sed -n 's/^version = "\(.*\)"/\1/p' "$CARGO_TOML" | head -1)"

version_lt() {
  local a=(${1//./ }) b=(${2//./ })
  for i in 0 1 2; do
    (( ${a[$i]:-0} < ${b[$i]:-0} )) && return 0
    (( ${a[$i]:-0} > ${b[$i]:-0} )) && return 1
  done
  return 1
}

if version_lt "$VERSION" "$CURRENT"; then
  echo "refusing downgrade: tag $VERSION < Cargo.toml $CURRENT" >&2
  exit 1
fi

if [[ "$CURRENT" == "$VERSION" ]]; then
  echo "unchanged"
  exit 0
fi

sed -i.bak "s/^version = \".*\"/version = \"$VERSION\"/" "$CARGO_TOML"
rm -f "${CARGO_TOML}.bak"
echo "bumped $CURRENT -> $VERSION"
