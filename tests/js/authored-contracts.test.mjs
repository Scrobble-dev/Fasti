import assert from "node:assert/strict";
import { cp, mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

import { validateAuthoredContracts } from "../../scripts/validate-authored-contracts.mjs";

const repositoryRoot = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../..",
);

const withContracts = async (mutate, assertRejected) => {
  const root = await mkdtemp(join(tmpdir(), "fasti-authored-contracts-"));
  try {
    for (const directory of [
      "contracts/asyncapi/v1",
      "contracts/jsonld/v1",
      "contracts/examples/v1",
    ]) {
      await mkdir(join(root, directory), { recursive: true });
      await cp(join(repositoryRoot, directory), join(root, directory), {
        recursive: true,
      });
    }
    await mutate(root);
    await assertRejected(validateAuthoredContracts(root));
  } finally {
    await rm(root, { recursive: true, force: true });
  }
};

test("authored AsyncAPI and JSON-LD validate locally", async () => {
  const result = await validateAuthoredContracts();
  assert.equal(result.asyncApiVersion, "3.1.0");
  assert.ok(result.expandedDocumentCount >= 2);
});

test("AsyncAPI version mutation is rejected", async () => {
  await withContracts(
    async (root) => {
      const path = join(root, "contracts/asyncapi/v1/transport.yaml");
      const source = await readFile(path, "utf8");
      await writeFile(
        path,
        source.replace("asyncapi: 3.1.0", "asyncapi: 2.6.0"),
      );
    },
    (result) => assert.rejects(result),
  );
});

test("AsyncAPI payload dialect mutation is rejected", async () => {
  await withContracts(
    async (root) => {
      const path = join(root, "contracts/asyncapi/v1/transport.yaml");
      const source = await readFile(path, "utf8");
      await writeFile(
        path,
        source.replace(
          "application/schema+json;version=draft-07",
          "application/schema+json;version=draft-06",
        ),
      );
    },
    (result) => assert.rejects(result),
  );
});

test("JSON-LD network context mutation is rejected", async () => {
  await withContracts(
    async (root) => {
      const path = join(
        root,
        "contracts/examples/v1/observation-accepted.jsonld",
      );
      const document = JSON.parse(await readFile(path, "utf8"));
      document["@context"] = "https://example.invalid/context.jsonld";
      await writeFile(path, `${JSON.stringify(document, null, 2)}\n`);
    },
    (result) => assert.rejects(result, /network access is forbidden/),
  );
});
