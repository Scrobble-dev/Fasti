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

function check(name, path) {
  const source = `
    const { imageSizeFromFile } = require(${JSON.stringify(imageSizeFromFilePath)});
    imageSizeFromFile(process.argv[1]).then(
      () => { throw new Error("parser accepted a zero-length record"); },
      () => {},
    );
  `;
  const result = spawnSync(process.execPath, ["--eval", source, path], {
    encoding: "utf8",
    timeout: 1_000,
  });
  assert.notEqual(result.error?.code, "ETIMEDOUT", `${name} parser hung`);
  assert.equal(result.status, 0, result.stderr);
}

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
