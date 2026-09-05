import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const portalOnlyPaths = [
  /^\.github\/workflows\/docs-pages\.yml$/u,
  /^apps\/docs\//u,
  /^diagrams\/documentation-publication\.svg$/u,
  /^docs\//u,
  /\.md$/iu,
  /^packages\/deploy-plan\//u,
  /^scripts\/validate-docs(?:-build)?\.mjs$/u,
  /^tests\/js\/deploy-plan\.test\.mjs$/u,
  /^tests\/js\/docs-[^/]+\.test\.mjs$/u,
  /^xtask\/src\/docs\.rs$/u,
];

export function requiresRuntime(paths) {
  const isPortalPath = (path) =>
    portalOnlyPaths.some((pattern) => pattern.test(path));
  return paths.length === 0 || paths.some((path) => !isPortalPath(path));
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  const paths = readFileSync(0).toString("utf8").split("\0").filter(Boolean);
  process.stdout.write(`runtime=${requiresRuntime(paths)}\n`);
}
