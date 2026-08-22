#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
policy="$repo_root/scripts/check-no-publish.sh"
fixture_dir="$(mktemp -d)"
trap 'rm -rf "$fixture_dir"' EXIT

printf '%s\n' 'permissions:' '  packages: write' > "$fixture_dir/mutated.yml"

if bash "$policy" "$fixture_dir" "$fixture_dir" >/dev/null 2>&1; then
  echo "Policy mutation test failed: packages: write was accepted" >&2
  exit 1
fi

printf '%s\n' 'jobs:' '  build:' '    steps:' '      - uses: docker/build-push-action@v6' '        with:' '          push: true' > "$fixture_dir/mutated.yml"

if bash "$policy" "$fixture_dir" "$fixture_dir" >/dev/null 2>&1; then
  echo "Policy mutation test failed: push: true was accepted" >&2
  exit 1
fi

printf '%s\n' 'jobs:' '  publish:' '    steps:' '      - run: docker push example.invalid/fasti:test' > "$fixture_dir/mutated.yml"

if bash "$policy" "$fixture_dir" "$fixture_dir" >/dev/null 2>&1; then
  echo "Policy mutation test failed: docker push was accepted" >&2
  exit 1
fi

printf '%s\n' 'jobs:' '  release:' '    steps:' '      - uses: ncipollo/release-action@v1' > "$fixture_dir/mutated.yml"

if bash "$policy" "$fixture_dir" "$fixture_dir" >/dev/null 2>&1; then
  echo "Policy mutation test failed: alternate release action was accepted" >&2
  exit 1
fi

printf '%s\n' '#!/usr/bin/env bash' 'docker push example.invalid/fasti:test' > "$fixture_dir/publish.sh"
printf '%s\n' 'jobs:' '  publish:' '    steps:' '      - run: bash publish.sh' > "$fixture_dir/mutated.yml"

if bash "$policy" "$fixture_dir" "$fixture_dir" >/dev/null 2>&1; then
  echo "Policy mutation test failed: publishing helper script was accepted" >&2
  exit 1
fi

echo "PASS: deliberate publishing mutations fail policy"
