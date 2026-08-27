#!/usr/bin/env node

import { readFile, readdir } from "node:fs/promises";
import { extname, join, relative } from "node:path";
import process from "node:process";

const ROOT = process.cwd();
const SOURCE_ROOTS = ["apps/web/src", "packages/ui/src"];
const SOURCE_EXTENSIONS = new Set([".ts", ".svelte"]);

// Tabler is the product UI system. Semantic HTML, Svelte, local modules, and
// non-UI utility libraries remain valid. Competing component/icon/theme systems
// need an explicit reviewed repository-rule change instead of arriving through
// an incidental import.
const DENIED_PACKAGE_PREFIXES = [
  "@mui/",
  "@chakra-ui/",
  "@headlessui/",
  "@heroicons/",
  "@radix-ui/",
  "@shadcn/",
  "antd",
  "bootstrap-icons",
  "carbon-components",
  "daisyui",
  "flowbite",
  "heroicons",
  "lucide",
  "primevue",
  "react-bootstrap",
  "semantic-ui",
  "tailwindcss",
];

const IMPORT_PATTERN = /(?:from\s+|import\s*\(|import\s+)["']([^"']+)["']/g;

function deniedPackage(specifier) {
  return DENIED_PACKAGE_PREFIXES.find(
    (prefix) => specifier === prefix || specifier.startsWith(prefix),
  );
}

async function walk(directory) {
  const output = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) output.push(...(await walk(path)));
    else if (SOURCE_EXTENSIONS.has(extname(entry.name))) output.push(path);
  }
  return output;
}

async function checkSource() {
  const failures = [];
  for (const sourceRoot of SOURCE_ROOTS) {
    for (const file of await walk(join(ROOT, sourceRoot))) {
      const content = await readFile(file, "utf8");
      for (const match of content.matchAll(IMPORT_PATTERN)) {
        const specifier = match[1];
        const denied = deniedPackage(specifier);
        if (denied) {
          failures.push(
            `${relative(ROOT, file)} imports ${specifier} (competing UI boundary: ${denied})`,
          );
        }
      }
    }
  }
  return failures;
}

async function checkManifests() {
  const failures = [];
  const manifests = [
    "package.json",
    "apps/web/package.json",
    "packages/ui/package.json",
  ];
  for (const manifest of manifests) {
    const value = JSON.parse(await readFile(join(ROOT, manifest), "utf8"));
    for (const group of [
      "dependencies",
      "devDependencies",
      "peerDependencies",
    ]) {
      for (const name of Object.keys(value[group] ?? {})) {
        const denied = deniedPackage(name);
        if (denied) {
          failures.push(
            `${manifest} declares ${name} in ${group} (competing UI boundary: ${denied})`,
          );
        }
      }
    }
  }
  return failures;
}

const failures = [...(await checkSource()), ...(await checkManifests())];
if (failures.length > 0) {
  console.error("Tabler UI boundary failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  console.error(
    "\nUse Tabler Core/Tabler Icons first. A different UI system requires a narrow reviewed repository-rule change.",
  );
  process.exit(1);
}

console.log("Tabler UI boundary: PASS");
