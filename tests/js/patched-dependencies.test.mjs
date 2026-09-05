import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readdirSync } from "node:fs";
import { createRequire } from "node:module";
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
    let rejected = false;
    try { imageSize(Buffer.from(${JSON.stringify(payload.toString("base64"))}, "base64")); } catch { rejected = true; }
    if (!rejected) throw new Error("parser accepted a zero-length record");
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

test("Ajv uses the fixed URI resolver without authority-changing normalization", () => {
  const workspace = createRequire(
    new URL("../../package.json", import.meta.url),
  );
  const ajvDependency = createRequire(workspace.resolve("ajv"));
  const uri = ajvDependency("fast-uri");
  assert.equal(ajvDependency("fast-uri/package.json").version, "3.1.7");

  const base = "https://safe.example/dir/file";
  for (const input of [
    "%2f%2fevil.example:/pwn",
    "%u002f%u002fevil.example:/pwn",
    "http://[::not-valid]/",
    "http://trusted.example]/app",
  ]) {
    assert.ok(
      uri.parse(input).error,
      `malformed URI must be reported: ${input}`,
    );
    assert.equal(uri.normalize(input), input);
    assert.throws(() => uri.resolve(base, input), Error);
  }

  const nested = "http://%256c%256f%2563%2561%256c%2568%256f%2573%2574/";
  assert.equal(uri.normalize(nested), nested);
  assert.equal(
    uri.resolve(base, nested),
    nested,
    "nested escapes stay encoded",
  );
  const idn = uri.resolve(base, "//bücher.example/pwn");
  assert.equal(idn, "https://xn--bcher-kva.example/pwn");
  assert.equal(uri.parse(idn).host, new URL(idn).hostname);

  // GHSA-qw65-cvwx-89v3: a port cannot replace the configured authority.
  const components = { scheme: "http", host: "trusted.example", path: "/app" };
  assert.throws(
    () => uri.serialize({ ...components, port: "@127.0.0.1:8124" }),
    TypeError,
  );
  for (const port of [8124, "8124"]) {
    assert.equal(
      uri.serialize({ ...components, port }),
      "http://trusted.example:8124/app",
    );
  }

  // GHSA-58mr-gqgx-xq4g: the reported unclosed bracket must not hide the
  // destination in a misleading host. Here '[' is userinfo, not an IP literal.
  const unclosedAuthority = "http://[@127.0.0.1/app";
  assert.equal(uri.parse(unclosedAuthority).host, "127.0.0.1");
  assert.equal(
    uri.parse(unclosedAuthority).host,
    new URL(unclosedAuthority).hostname,
  );
  assert.equal(uri.normalize(unclosedAuthority), "http://%5B@127.0.0.1/app");
  assert.equal(
    uri.resolve(base, "http://[2001:db8::1]:8124/app"),
    "http://[2001:db8::1]:8124/app",
  );

  assert.equal(
    uri.normalize("HTTPS://EXAMPLE.com:443/a/../b?x=1#part"),
    "https://example.com/b?x=1#part",
  );
  assert.equal(uri.parse("https://[2001:db8::1]/").host, "2001:db8::1");
  assert.equal(
    uri.resolve(base, "../asset?x=1#part"),
    "https://safe.example/asset?x=1#part",
  );

  const Ajv = workspace("ajv");
  const ajv = new Ajv();
  assert.equal(
    ajv.opts.uriResolver,
    uri,
    "exercise Ajv's actual resolver owner",
  );
  const validate = ajv.compile({
    $id: "https://schema.example/root.json",
    $defs: { item: { type: "string", minLength: 1 } },
    type: "array",
    items: { $ref: "#/$defs/item" },
  });
  assert.equal(validate(["valid"]), true);
  assert.equal(validate([1]), false);
});

test("Docs Express and body-parser both resolve qs with bounded comma arrays", () => {
  const docs = createRequire(
    new URL("../../apps/docs/package.json", import.meta.url),
  );
  const docusaurus = createRequire(
    docs.resolve("@docusaurus/core/package.json"),
  );
  const server = createRequire(docusaurus.resolve("webpack-dev-server"));
  const express = createRequire(server.resolve("express"));
  const bodyParser = createRequire(express.resolve("body-parser"));
  for (const [name, consumer] of [
    ["Express", express],
    ["body-parser", bodyParser],
  ]) {
    assert.equal(
      consumer("qs/package.json").version,
      "6.16.0",
      `${name} resolved version`,
    );
    const qs = consumer("qs");
    const options = { comma: true, arrayLimit: 3, throwOnLimitExceeded: true };
    assert.throws(() => qs.parse("a[]=1,2,3,4", options), RangeError, name);
    assert.throws(() => qs.parse("a=1,2,3,4", options), RangeError, name);
    assert.deepEqual(qs.parse("a[]=1,2,3", options), { a: [["1", "2", "3"]] });
    assert.deepEqual(qs.parse("a=1,2,3", options), { a: ["1", "2", "3"] });
    assert.deepEqual(qs.parse("filter[name]=A%20B&tags[]=one&tags[]=two"), {
      filter: { name: "A B" },
      tags: ["one", "two"],
    });
  }
});
