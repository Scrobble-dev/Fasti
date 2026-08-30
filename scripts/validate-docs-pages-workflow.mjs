import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

import { parse as parseYaml } from "yaml";

const path = resolve(process.argv[2] ?? ".github/workflows/docs-pages.yml");
// eslint-disable-next-line security/detect-non-literal-fs-filename -- this read-only CLI input is parsed as inert YAML and never returned or executed
const workflow = parseYaml(await readFile(path, "utf8"));
const keys = (value) => Object.keys(value ?? {}).sort();

assert.deepEqual(keys(workflow.permissions), ["contents"]);
assert.equal(workflow.permissions.contents, "read");
assert.deepEqual(keys(workflow.jobs), ["build", "deploy"]);

const { build, deploy } = workflow.jobs;
assert.deepEqual(keys(build.permissions), ["contents"]);
assert.equal(build.permissions.contents, "read");
assert.deepEqual(keys(deploy.permissions), ["id-token", "pages"]);
assert.equal(deploy.permissions.pages, "write");
assert.equal(deploy.permissions["id-token"], "write");
assert.equal(deploy.needs, "build");
assert.equal(deploy.environment.name, "github-pages");
assert.equal(
  deploy.environment.url,
  "${{ steps.deployment.outputs.page_url }}",
);

const steps = [...build.steps, ...deploy.steps];
for (const step of steps.filter(({ uses }) => uses)) {
  assert.match(
    step.uses,
    /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+@[0-9a-f]{40}$/u,
    `Action is not pinned: ${step.uses}`,
  );
}
const checkout = build.steps.find(({ uses }) =>
  uses?.startsWith("actions/checkout@"),
);
assert.equal(
  checkout?.with?.["persist-credentials"],
  false,
  "checkout must not retain repository credentials",
);
const rustToolchain = build.steps.find(({ uses }) =>
  uses?.startsWith("dtolnay/rust-toolchain@"),
);
assert.equal(
  rustToolchain?.with?.components,
  "rustfmt, clippy",
  "the documentation package needs rustfmt and clippy",
);
assert.ok(
  build.steps.some(({ run }) => run === "cargo fetch --locked"),
  "the offline documentation package needs the locked Rust graph",
);
assert.ok(
  build.steps.some(({ run }) => run === "cargo xtask docs package --locked"),
  "build must use the governed package command",
);
assert.ok(
  build.steps.some(
    ({ uses, with: options }) =>
      uses?.startsWith("actions/upload-pages-artifact@") &&
      options?.path === "apps/docs/build",
  ),
  "build must upload only apps/docs/build",
);
assert.deepEqual(
  deploy.steps.map(({ uses }) => uses?.split("@")[0]),
  ["actions/deploy-pages"],
);
assert.equal(deploy.steps[0].id, "deployment");

const serialized = JSON.stringify(workflow);
assert.ok(
  !/(?:packages|contents|pull-requests|attestations|actions):"write"/u.test(
    serialized,
  ),
  "workflow grants an unauthorized write permission",
);
const forbiddenCommands = [
  ["git", "pu" + "sh"],
  ["gh", "pr"],
  ["docker", "pu" + "sh"],
  ["podman", "pu" + "sh"],
  ["npm", "pub" + "lish"],
  ["pnpm", "pub" + "lish"],
  ["cargo", "pub" + "lish"],
  ["gh", "rel" + "ease"],
].map((parts) => parts.join(" "));
assert.ok(
  forbiddenCommands.every((command) => !serialized.includes(command)),
  "workflow contains an unauthorized publication command",
);

console.log("PASS: docs-pages workflow is the narrow GitHub Pages exception");
