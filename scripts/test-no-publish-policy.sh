#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
policy="$repo_root/scripts/check-no-publish.sh"
fixture_dir="$(mktemp -d)"
fixture_workflows="$fixture_dir/workflows"
fixture_scripts="$fixture_dir/scripts"
mkdir -p "$fixture_workflows" "$fixture_scripts"
trap 'rm -rf "$fixture_dir"' EXIT

assert_rejected() {
  local label="$1"
  if bash "$policy" "$fixture_workflows" "$fixture_scripts" >/dev/null 2>&1; then
    echo "Policy mutation test failed: $label was accepted" >&2
    exit 1
  fi
}

cp "$repo_root/.github/workflows/docs-pages.yml" "$fixture_workflows/docs-pages.yml"
bash "$policy" "$fixture_workflows" "$fixture_scripts" >/dev/null
rm "$fixture_workflows/docs-pages.yml"

printf '%s\n' 'permissions:' '  packages: write' > "$fixture_workflows/mutated.yml"

assert_rejected "packages: write"

printf '%s\n' 'permissions: write-all' > "$fixture_workflows/mutated.yml"
assert_rejected "permissions: write-all"

printf '%s\n' 'permissions:' '  contents: write' > "$fixture_workflows/mutated.yml"
assert_rejected "contents: write"

printf '%s\n' 'jobs:' '  mutate:' '    steps:' '      - run: git push origin HEAD:review-branch' > "$fixture_workflows/mutated.yml"
assert_rejected "git push"

printf '%s\n' 'jobs:' '  mutate:' '    steps:' '      - run: gh pr merge 14 --squash' > "$fixture_workflows/mutated.yml"
assert_rejected "pull-request mutation"

printf '%s\n' 'jobs:' '  build:' '    steps:' '      - uses: docker/build-push-action@v6' '        with:' '          push: true' > "$fixture_workflows/mutated.yml"

assert_rejected "push: true"

printf '%s\n' 'jobs:' '  build:' '    steps:' '      - run: docker buildx build --platform linux/arm64 --push .' > "$fixture_workflows/mutated.yml"
assert_rejected "docker buildx build --push"

printf '%s\n' 'jobs:' '  publish:' '    steps:' '      - run: docker push example.invalid/fasti:test' > "$fixture_workflows/mutated.yml"

assert_rejected "docker push"

printf '%s\n' 'jobs:' '  publish:' '    steps:' '      - run: pnpm run publish' > "$fixture_workflows/mutated.yml"
assert_rejected "pnpm run publish"

printf '%s\n' 'jobs:' '  release:' '    steps:' '      - uses: ncipollo/release-action@v1' > "$fixture_workflows/mutated.yml"

assert_rejected "alternate release action"

printf '%s\n' '#!/usr/bin/env bash' 'docker push example.invalid/fasti:test' > "$fixture_scripts/publish.sh"
printf '%s\n' 'jobs:' '  publish:' '    steps:' '      - run: bash publish.sh' > "$fixture_workflows/mutated.yml"

assert_rejected "publishing shell helper"

rm "$fixture_scripts/publish.sh"
printf '%s\n' 'import { execFileSync } from "node:child_process";' 'execFileSync("npm", ["publish"]);' > "$fixture_scripts/publish.mjs"
assert_rejected "publishing non-shell helper"

echo "PASS: deliberate publishing mutations fail policy"
