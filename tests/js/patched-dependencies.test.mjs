import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import test from "node:test";

const pnpmStore = resolve("node_modules/.pnpm");
const imageSizePackage = readdirSync(pnpmStore).find((entry) =>
  entry.startsWith("image-size@2.0.2_patch_hash="),
);
assert.ok(imageSizePackage, "patched image-size package is installed");
const imageSizePath = join(
  pnpmStore,
  imageSizePackage,
  "node_modules/image-size/dist/index.cjs",
);

function check(name, payload) {
  const source = `
    const { imageSize } = require(${JSON.stringify(imageSizePath)});
    try { imageSize(Buffer.from(${JSON.stringify(payload.toString("base64"))}, "base64")); } catch {}
  `;
  const result = spawnSync(process.execPath, ["--eval", source], {
    encoding: "utf8",
    timeout: 1_000,
  });
  assert.notEqual(result.error?.code, "ETIMEDOUT", `${name} parser hung`);
  assert.equal(result.status, 0, result.stderr);
}

test("patched image parsers reject zero-length records without hanging", () => {
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

  check("ICNS", icns);
  check("JXL", jxl);
  check("HEIF", heif);
});
