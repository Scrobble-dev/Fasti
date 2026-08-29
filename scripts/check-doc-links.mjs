import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const markdownFiles = execFileSync(
  "git",
  ["ls-files", "--cached", "--others", "--exclude-standard", "--", "*.md"],
  {
    cwd: repoRoot,
    encoding: "utf8",
  },
)
  .trim()
  .split("\n")
  .filter(Boolean);

const inlineLink = /!?\[[^\]]*\]\(([^)]+)\)/g;
// Markdown footnotes use the same leading shape as reference-link
// definitions, but the text after `:` is prose rather than a path.
const definitionLink = /^\[(?!\^)[^\]]+\]:\s+(\S+)/gm;
const failures = [];

function checkTarget(file, rawTarget) {
  let target = rawTarget.trim().replace(/^<|>$/g, "");
  target = target.replace(/\s+"[^"]*"$/, "");

  if (/^(https?:|mailto:|tel:)/i.test(target) || target.startsWith("#")) {
    return;
  }

  if (/^file:/i.test(target)) {
    failures.push(`${file}: non-portable file URL ${target}`);
    return;
  }

  const pathPart = decodeURIComponent(target.split("#", 1)[0]);
  if (!pathPart || pathPart.startsWith("/")) {
    return;
  }

  const absolute = resolve(repoRoot, dirname(file), pathPart);
  const relativeToRoot = relative(repoRoot, absolute);
  if (relativeToRoot.startsWith("..") || isAbsolute(relativeToRoot)) {
    failures.push(`${file}: link target escapes the repository ${pathPart}`);
    return;
  }
  // absolute is confirmed to stay within repoRoot above.
  // eslint-disable-next-line security/detect-non-literal-fs-filename
  if (!existsSync(absolute)) {
    failures.push(`${file}: missing local link target ${pathPart}`);
  }
}

for (const file of markdownFiles) {
  const absoluteFile = resolve(repoRoot, file);
  // file is a path returned by `git ls-files` above, which never emits ".."
  // path components, so absoluteFile cannot escape repoRoot.
  /* eslint-disable security/detect-non-literal-fs-filename */
  if (!existsSync(absoluteFile)) {
    continue;
  }

  const content = readFileSync(absoluteFile, "utf8");
  /* eslint-enable security/detect-non-literal-fs-filename */
  for (const match of content.matchAll(inlineLink)) {
    checkTarget(file, match[1]);
  }
  for (const match of content.matchAll(definitionLink)) {
    checkTarget(file, match[1]);
  }
}

if (failures.length > 0) {
  console.error(failures.join("\n"));
  process.exit(1);
}

console.log(
  `PASS: ${markdownFiles.length} Markdown files have valid local links`,
);
