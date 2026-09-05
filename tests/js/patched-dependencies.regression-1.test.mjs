import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { createRequire } from "node:module";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

const docsRequire = createRequire(
  new URL("../../apps/docs/package.json", import.meta.url),
);
const docusaurusCoreRequire = createRequire(
  docsRequire.resolve("@docusaurus/core/package.json"),
);
const mdxLoaderRequire = createRequire(
  docusaurusCoreRequire.resolve("@docusaurus/mdx-loader/package.json"),
);
const imageSizeFromFilePath = mdxLoaderRequire.resolve("image-size/fromFile");

function runParser(path, mode = "real") {
  const source = `
    const { imageSizeFromFile } = require(${JSON.stringify(imageSizeFromFilePath)});
    const mode = process.argv[2];
    const result = mode === "fulfilled"
      ? Promise.resolve()
      : mode === "rejected"
        ? Promise.reject(new Error("expected parser rejection"))
        : mode === "stalled"
          ? new Promise(() => {})
          : imageSizeFromFile(process.argv[1]);
    let completed = false;
    const watchdog = setTimeout(() => {
      completed = true;
      console.error("parser promise did not settle");
      process.exitCode = 2;
    }, 750);
    result.then(
      () => {
        if (completed) return;
        completed = true;
        clearTimeout(watchdog);
        console.error("parser accepted a zero-length record");
        process.exitCode = 1;
      },
      () => {
        if (completed) return;
        completed = true;
        clearTimeout(watchdog);
      },
    );
  `;
  return spawnSync(process.execPath, ["--eval", source, path, mode], {
    encoding: "utf8",
    timeout: 2_000,
  });
}

function check(name, path) {
  const result = runParser(path);
  assert.notEqual(result.error?.code, "ETIMEDOUT", `${name} parser hung`);
  assert.equal(result.status, 0, result.stderr);
}

test("parser child distinguishes rejection, fulfillment, and stalled promises", () => {
  const rejected = runParser(process.execPath, "rejected");
  assert.equal(rejected.status, 0, rejected.stderr);

  const fulfilled = runParser(process.execPath, "fulfilled");
  assert.equal(fulfilled.status, 1, fulfilled.stderr);
  assert.match(fulfilled.stderr, /parser accepted a zero-length record/u);

  const stalled = runParser(process.execPath, "stalled");
  assert.notEqual(
    stalled.error?.code,
    "ETIMEDOUT",
    "stalled control hit outer timeout",
  );
  assert.equal(stalled.status, 2, stalled.stderr);
  assert.match(stalled.stderr, /parser promise did not settle/u);
});

test("Docusaurus image-size/fromFile rejects zero-length records without hanging", () => {
  const directory = mkdtempSync(join(tmpdir(), "fasti-image-size-"));
  try {
    const icns = Buffer.alloc(16);
    icns.write("icns", 0);
    icns.writeUInt32BE(16, 4);
    icns.write("ic07", 8);

    const jxl = Buffer.alloc(40);
    jxl.writeUInt32BE(12, 0);
    jxl.write("JXL ", 4);
    jxl.writeUInt32BE(20, 12);
    jxl.write("ftyp", 16);
    jxl.write("jxl ", 20);
    jxl.write("jxlp", 36);

    const heif = Buffer.alloc(68);
    heif.writeUInt32BE(20, 0);
    heif.write("ftyp", 4);
    heif.write("avif", 8);
    heif.writeUInt32BE(12, 20);
    heif.write("meta", 24);
    heif.writeUInt32BE(8, 32);
    heif.write("iprp", 36);
    heif.writeUInt32BE(28, 40);
    heif.write("ipco", 44);
    heif.write("ispe", 52);
    heif.writeUInt32BE(100, 60);
    heif.writeUInt32BE(100, 64);

    for (const [name, payload] of [
      ["ICNS", icns],
      ["JXL", jxl],
      ["HEIF", heif],
    ]) {
      const path = join(directory, name.toLowerCase());
      writeFileSync(path, payload);
      check(name, path);
    }
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
