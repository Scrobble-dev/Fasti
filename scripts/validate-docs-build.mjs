import assert from "node:assert/strict";
import { readFile, readdir, stat } from "node:fs/promises";
import { resolve } from "node:path";
import { execFileSync } from "node:child_process";

const root = resolve(import.meta.dirname, "..");
const build = resolve(root, process.argv[2] ?? "apps/docs/build");
const budgets = JSON.parse(
  await readFile(resolve(root, "docs/performance-budgets.json"), "utf8"),
);
const required = [
  "index.html",
  "404.html",
  "sitemap.xml",
  "robots.txt",
  "llms.txt",
  "openapi.json",
  "openapi-conformance.json",
  "capabilities.json",
  "problems.json",
  "release.json",
  "docs-manifest.json",
  "pagefind/pagefind.js",
  "pagefind/pagefind-ui.js",
  "v1/problems/validation-failed/index.html",
  "deploy/index.html",
  "search/index.html",
  "status/index.html",
];
for (const path of required)
  assert.ok(
    (await stat(resolve(build, path))).isFile(),
    `missing built resource ${path}`,
  );

const release = JSON.parse(
  await readFile(resolve(build, "release.json"), "utf8"),
);
const commit = execFileSync("git", ["rev-parse", "HEAD"], {
  cwd: root,
  encoding: "utf8",
}).trim();
assert.equal(release.source_commit, commit);
assert.equal(release.supported_release, false);
assert.equal(release.support_state, "unsupported");

const htmlFiles = [];
const allFiles = [];
async function walk(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) await walk(path);
    else if (entry.isFile()) {
      allFiles.push(path);
      if (entry.name.endsWith(".html")) htmlFiles.push(path);
    }
  }
}
await walk(build);
assert.ok(htmlFiles.length >= 38, "built route inventory is incomplete");
for (const path of htmlFiles) {
  const html = await readFile(path, "utf8");
  assert.ok(
    !/google-analytics|googletagmanager|plausible|segment\.com/iu.test(html),
    `${path} contains runtime telemetry`,
  );
  const canonicalCount = (html.match(/<link[^>]+rel="canonical"/gu) ?? [])
    .length;
  assert.equal(
    canonicalCount,
    1,
    `${path} has an invalid canonical-link count`,
  );
}
const sitemap = await readFile(resolve(build, "sitemap.xml"), "utf8");
assert.match(sitemap, /https:\/\/fasti\.scrobble\.dev\/deploy\//u);
assert.match(
  sitemap,
  /https:\/\/fasti\.scrobble\.dev\/v1\/problems\/validation-failed\//u,
);
const sizes = await Promise.all(
  allFiles.map(async (path) => [path, (await stat(path)).size]),
);
const total = (suffix) =>
  sizes
    .filter(([path]) => path.endsWith(suffix))
    .reduce((sum, [, size]) => sum + size, 0);
const treeTotal = (fragment) =>
  sizes
    .filter(([path]) => path.includes(fragment))
    .reduce((sum, [, size]) => sum + size, 0);
assert.ok(
  sizes.reduce((sum, [, size]) => sum + size, 0) <= budgets.artifact_bytes_max,
  "artifact exceeds its byte budget",
);
assert.ok(
  Math.max(
    ...sizes.filter(([path]) => path.endsWith(".html")).map(([, size]) => size),
  ) <= budgets.html_file_bytes_max,
  "HTML file exceeds its byte budget",
);
assert.ok(
  total(".js") <= budgets.javascript_bytes_max,
  "JavaScript exceeds its byte budget",
);
assert.ok(
  Math.max(
    ...sizes.filter(([path]) => path.endsWith(".js")).map(([, size]) => size),
  ) <= budgets.javascript_file_bytes_max,
  "JavaScript file exceeds its byte budget",
);
assert.ok(
  total(".css") <= budgets.css_bytes_max,
  "CSS exceeds its byte budget",
);
assert.ok(
  treeTotal("/pagefind/") <= budgets.search_bytes_max,
  "search index exceeds its byte budget",
);
const prose = await readFile(
  resolve(build, "start/what-fasti-is/index.html"),
  "utf8",
);
assert.ok(
  !prose.includes("pagefind-ui.js"),
  "ordinary prose pages must not load the search UI",
);

console.log(
  `PASS: static documentation artifact routes=${htmlFiles.length} source_commit=${commit}`,
);
