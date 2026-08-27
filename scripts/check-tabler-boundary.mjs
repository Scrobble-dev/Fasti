import { readFile, readdir } from "node:fs/promises";
import { extname, join, relative } from "node:path";
import process from "node:process";

const root = process.cwd();
const productRoots = ["packages/ui/src", "apps/web/src"];
const blockedPackages = [
  /^@mui\//,
  /^@chakra-ui\//,
  /^@mantine\//,
  /^@radix-ui\//,
  /^@headlessui\//,
  /^antd$/,
  /^bootstrap$/,
  /^bulma$/,
  /^daisyui$/,
  /^foundation-sites$/,
  /^lucide(?:-|$)/,
  /^@heroicons\//,
  /^@fortawesome\//,
  /^semantic-ui/,
  /^tailwindcss$/,
];
const importPattern = /(?:from\s+|import\s*\()\s*["']([^"']+)["']/g;
const allowedExtensions = new Set([".svelte", ".ts", ".tsx", ".js", ".mjs"]);
const exceptionPattern = /fasti-tabler-exception:\s*owner=([^\s]+)\s+expires=(\d{4}-\d{2}-\d{2})\s+reason=(.+)$/m;

async function walk(directory) {
  const entries = await readdir(join(root, directory), { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) files.push(...(await walk(path)));
    else if (allowedExtensions.has(extname(entry.name))) files.push(path);
  }
  return files;
}

function packageName(specifier) {
  if (specifier.startsWith("@")) return specifier.split("/").slice(0, 2).join("/");
  return specifier.split("/")[0];
}

function hasCurrentException(source) {
  const match = source.match(exceptionPattern);
  if (!match) return false;
  const expires = Date.parse(`${match[2]}T23:59:59Z`);
  return Number.isFinite(expires) && expires >= Date.now() && match[3].trim().length >= 12;
}

const violations = [];
const uiPackage = JSON.parse(await readFile(join(root, "packages/ui/package.json"), "utf8"));
for (const dependency of ["@tabler/core", "@tabler/icons-svelte"]) {
  if (!uiPackage.dependencies?.[dependency]) {
    violations.push(`packages/ui/package.json must depend on ${dependency}`);
  }
}

for (const manifestPath of ["package.json", "packages/ui/package.json", "apps/web/package.json"]) {
  const manifest = JSON.parse(await readFile(join(root, manifestPath), "utf8"));
  for (const section of ["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"]) {
    for (const dependency of Object.keys(manifest[section] ?? {})) {
      if (blockedPackages.some((pattern) => pattern.test(dependency))) {
        violations.push(`${manifestPath}: competing UI dependency ${dependency} is not allowed`);
      }
    }
  }
}

for (const productRoot of productRoots) {
  for (const file of await walk(productRoot)) {
    const source = await readFile(join(root, file), "utf8");
    const exempt = hasCurrentException(source);
    importPattern.lastIndex = 0;
    for (const match of source.matchAll(importPattern)) {
      const dependency = packageName(match[1]);
      if (blockedPackages.some((pattern) => pattern.test(dependency)) && !exempt) {
        violations.push(`${relative(root, join(root, file))}: competing UI import ${match[1]}`);
      }
    }
    if (file.endsWith(".svelte") && /<svg(?:\s|>)/i.test(source) && !exempt) {
      violations.push(
        `${file}: raw SVG found; use Tabler Icons first or add a dated fasti-tabler-exception with owner and reason`,
      );
    }
  }
}

if (violations.length > 0) {
  console.error("Tabler-first boundary failed:\n" + violations.map((item) => `- ${item}`).join("\n"));
  process.exit(1);
}

console.log("Tabler-first boundary: PASS");
