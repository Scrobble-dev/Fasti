import assert from "node:assert/strict";
import { readFile, readdir, stat } from "node:fs/promises";
import { relative, resolve, sep } from "node:path";
import { execFileSync } from "node:child_process";

/* eslint-disable security/detect-non-literal-fs-filename -- every dynamic path is a fixed build resource, a confined internal link, or an entry discovered under the build root */
/* eslint-disable security-node/detect-unhandled-async-errors -- top-level await intentionally turns traversal errors into a failing validator process */
/* eslint-disable xss/no-mixed-html -- built HTML is inspected as inert text and is never rendered or returned by this validator */

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
for (const path of required) {
  const information = await stat(resolve(build, path)).catch(() => undefined);
  assert.ok(information?.isFile(), `missing built resource ${path}`);
}

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
const internalLinks = new Set();
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
  assert.equal(
    (html.match(/<h1(?:\s|>)/gu) ?? []).length,
    1,
    `${path} must contain one H1`,
  );
  for (const match of html.matchAll(/<a\b[^>]*\bhref="([^"]+)"/gu)) {
    const url = new URL(match[1], "https://fasti.scrobble.dev");
    if (url.origin === "https://fasti.scrobble.dev")
      internalLinks.add(decodeURIComponent(url.pathname));
  }
}
for (const pathname of internalLinks) {
  const candidate = resolve(build, `.${pathname}`);
  const target = pathname.endsWith("/")
    ? resolve(candidate, "index.html")
    : candidate;
  const local = relative(build, target);
  assert.ok(
    local && !local.startsWith("..") && !local.startsWith("/"),
    `unsafe built link ${pathname}`,
  );
  assert.ok(
    await stat(target).catch(() => false),
    `broken built link ${pathname}`,
  );
}
assert.ok(
  internalLinks.size >= 25,
  "built internal-link inventory is incomplete",
);
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
const treeTotal = (prefix) =>
  sizes
    .filter(([path]) =>
      relative(build, path).split(sep).join("/").startsWith(prefix),
    )
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
  treeTotal("pagefind/") <= budgets.search_bytes_max,
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

const home = await readFile(resolve(build, "index.html"), "utf8");
assert.equal(
  (home.match(/href="https:\/\/scrobble\.dev\/learn\/scrobbling\/"/gu) ?? [])
    .length,
  1,
  "home must contain one contextual Scrobble.dev field-guide link",
);
assert.match(
  home,
  /href="https:\/\/scrobble\.dev\/"[^>]*class="footer__link-item">Scrobble\.dev independent field guide/u,
  "footer must contain the related Scrobble.dev link",
);
assert.match(home, /Read Scrobble\.dev’s scrobbling definition/u);

const notFound = await readFile(resolve(build, "404.html"), "utf8");
assert.match(notFound, /href="\/start\/choose-a-path\/"/u);
assert.match(notFound, /href="\/search\/"/u);

const firstObservation = await readFile(
  resolve(build, "integrate/first-observation/index.html"),
  "utf8",
);
assert.match(firstObservation, /FASTI_CREDENTIAL_FILE/u);
assert.match(firstObservation, /client\.submitObservation/u);
assert.match(firstObservation, /source_event_id/u);
assert.match(firstObservation, /consumption_occurrence/u);
assert.match(firstObservation, /Replace every example value/u);

console.log(
  `PASS: static documentation artifact routes=${htmlFiles.length} source_commit=${commit}`,
);

/* eslint-enable security/detect-non-literal-fs-filename, security-node/detect-unhandled-async-errors, xss/no-mixed-html */
