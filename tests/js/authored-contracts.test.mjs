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
      "contracts/generated/v1",
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
          "application/schema+json;version=draft-2020-12",
          "application/schema+json;version=draft-06",
        ),
      );
    },
    (result) => assert.rejects(result),
  );
});

test("AsyncAPI duplicate keys are rejected", async () => {
  await withContracts(
    async (root) => {
      const path = join(root, "contracts/asyncapi/v1/transport.yaml");
      const source = await readFile(path, "utf8");
      await writeFile(
        path,
        source.replace(
          "    x-fasti-durability: none",
          "    x-fasti-durability: none\n    x-fasti-durability: none",
        ),
      );
    },
    (result) => assert.rejects(result, /Map keys must be unique/u),
  );
});

for (const [label, reference] of [
  ["HTTP", "https://example.invalid/message.yaml"],
  ["file", "file:///etc/hosts"],
]) {
  test(`AsyncAPI ${label} reference is rejected before resolution`, async () => {
    await withContracts(
      async (root) => {
        const path = join(root, "contracts/asyncapi/v1/transport.yaml");
        const source = await readFile(path, "utf8");
        await writeFile(
          path,
          source.replace(
            "payload:\n        schemaFormat:",
            `payload:\n        $ref: ${reference}\n        schemaFormat:`,
          ),
        );
      },
      (result) => assert.rejects(result, /forbidden external reference/u),
    );
  });
}

test("AsyncAPI identifier pattern drift is rejected", async () => {
  await withContracts(
    async (root) => {
      const path = join(root, "contracts/asyncapi/v1/transport.yaml");
      const source = await readFile(path, "utf8");
      await writeFile(
        path,
        source.replace(
          "^rcp_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$",
          "^rcp_[0-9a-f]{32}$",
        ),
      );
    },
    (result) => assert.rejects(result, /identifier constraint/u),
  );
});

test("AsyncAPI timestamp constraint drift is rejected", async () => {
  await withContracts(
    async (root) => {
      const path = join(root, "contracts/asyncapi/v1/transport.yaml");
      const source = await readFile(path, "utf8");
      await writeFile(
        path,
        source.replace(
          "              maxLength: 35\n              pattern: ^[0-9]{4}",
          "              maxLength: 64\n              pattern: ^[0-9]{4}",
        ),
      );
    },
    (result) => assert.rejects(result, /timestamp constraint/u),
  );
});

test("AsyncAPI resolution drift is rejected", async () => {
  await withContracts(
    async (root) => {
      const path = join(root, "contracts/asyncapi/v1/transport.yaml");
      const source = await readFile(path, "utf8");
      await writeFile(
        path,
        source.replace("const: unresolved", "const: resolved"),
      );
    },
    (result) => assert.rejects(result, /sole Utoipa receipt resolution/u),
  );
});

test("AsyncAPI SSE cursor relation drift is rejected", async () => {
  await withContracts(
    async (root) => {
      const path = join(root, "contracts/asyncapi/v1/transport.yaml");
      const source = await readFile(path, "utf8");
      await writeFile(
        path,
        source.replace(
          "x-fasti-sse-id-pointer: $message.payload#/receipt_id",
          "x-fasti-sse-id-pointer: $message.payload#/operation_id",
        ),
      );
    },
    (result) => assert.rejects(result, /SSE cursor/u),
  );
});

for (const [field, from, to, expected] of [
  [
    "durability",
    "x-fasti-durability: none",
    "x-fasti-durability: local",
    /durable fixture/u,
  ],
  [
    "fixture delivery",
    "x-fasti-fixture-delivery: finite_replay_then_close",
    "x-fasti-fixture-delivery: waits_for_future_receipts",
    /finite replay/u,
  ],
]) {
  test(`AsyncAPI ${field} claim drift is rejected`, async () => {
    await withContracts(
      async (root) => {
        const path = join(root, "contracts/asyncapi/v1/transport.yaml");
        const source = await readFile(path, "utf8");
        await writeFile(path, source.replace(from, to));
      },
      (result) => assert.rejects(result, expected),
    );
  });
}

test("JSON-LD network context mutation is rejected", async () => {
  await withContracts(
    async (root) => {
      const path = join(
        root,
        "contracts/examples/v1/observation.accept.receipt.jsonld",
      );
      const document = JSON.parse(await readFile(path, "utf8"));
      document["@context"] = "https://example.invalid/context.jsonld";
      await writeFile(path, `${JSON.stringify(document, null, 2)}\n`);
    },
    (result) => assert.rejects(result, /network access is forbidden/),
  );
});

test("JSON-LD context duplicate terms are rejected", async () => {
  await withContracts(
    async (root) => {
      const path = join(root, "contracts/jsonld/v1/context.jsonld");
      const source = await readFile(path, "utf8");
      await writeFile(
        path,
        source.replace(
          '    "fasti": "https://fasti.scrobble.dev/ns/v1/",',
          '    "fasti": "https://example.invalid/ns/",\n    "fasti": "https://fasti.scrobble.dev/ns/v1/",',
        ),
      );
    },
    (result) => assert.rejects(result, /Map keys must be unique/u),
  );
});

test("JSON-LD external file context mutation is rejected before read", async () => {
  await withContracts(
    async (root) => {
      const path = join(
        root,
        "contracts/examples/v1/observation.accept.receipt.jsonld",
      );
      const document = JSON.parse(await readFile(path, "utf8"));
      document["@context"] = "file:///etc/hosts";
      await writeFile(path, `${JSON.stringify(document, null, 2)}\n`);
    },
    (result) => assert.rejects(result, /file:\/\/\/etc\/hosts/u),
  );
});
