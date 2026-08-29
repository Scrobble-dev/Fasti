import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, realpathSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repoRootReal = realpathSync(repoRoot);

// resolve()/relative() only prove lexical containment; a symlinked ancestor
// directory or the target itself could still resolve outside the repo on
// the real filesystem. Call this only after existsSync() has confirmed the
/**
 * Determines whether an existing path resolves within the repository.
 * @param {string} lexicallyContainedPath - The path to check.
 * @returns {boolean} `true` if the resolved path is the repository or one of its descendants, `false` otherwise.
 */
function isReallyContained(lexicallyContainedPath) {
  // Every caller passes a path that existsSync() has just confirmed exists
  // and that lexical containment has already checked immediately before.
  // eslint-disable-next-line security/detect-non-literal-fs-filename
  const real = realpathSync(lexicallyContainedPath);
  return real === repoRootReal || real.startsWith(repoRootReal + sep);
}
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

/**
 * Validates a Markdown link target against the repository boundaries.
 * @param {string} file - The Markdown file containing the link.
 * @param {string} rawTarget - The link target to validate.
 */
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
    return;
  }
  if (!isReallyContained(absolute)) {
    failures.push(
      `${file}: link target resolves outside the repository via a symlink ${pathPart}`,
    );
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

  /* eslint-enable security/detect-non-literal-fs-filename */
  if (!isReallyContained(absoluteFile)) {
    failures.push(
      `${file}: tracked file resolves outside the repository via a symlink`,
    );
    continue;
  }
  // eslint-disable-next-line security/detect-non-literal-fs-filename
  const content = readFileSync(absoluteFile, "utf8");
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
