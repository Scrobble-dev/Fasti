import assert from "node:assert/strict";
import { cp, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

import { validateExamples } from "../../scripts/validate-examples.mjs";

const repositoryRoot = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../..",
);

const withContractCopy = async (mutate, assertion) => {
  const root = await mkdtemp(join(tmpdir(), "fasti-examples-"));
  try {
    for (const relative of [
      "contracts/examples/v1",
      "contracts/asyncapi/v1",
      "contracts/generated/v1",
      "contracts/jsonld/v1",
      "packages/schemas/schemas",
    ]) {
      await cp(join(repositoryRoot, relative), join(root, relative), {
        recursive: true,
      });
    }
    await mutate(root);
    await assertion(validateExamples(root));
  } finally {
    await rm(root, { recursive: true, force: true });
  }
};

const mutateJson = async (root, relative, mutate) => {
  const path = join(root, relative);
  const value = JSON.parse(await readFile(path, "utf8"));
  mutate(value);
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`);
};

test("governed examples validate semantically", async () => {
  const result = await validateExamples();
  assert.equal(result.linkedDataCount, 1);
  assert.ok(result.exampleCount >= 14);
  assert.ok(result.problemCount >= 11);
});

test("problem examples cannot claim another capability", async () => {
  await withContractCopy(
    (root) =>
      mutateJson(
        root,
        "contracts/examples/v1/receipt.replay.receipt_not_found.json",
        (example) => {
          example.capability_id = "receipt.stream";
        },
      ),
    (result) => assert.rejects(result, /wrong owner/u),
  );
});

test("example schemas reject undeclared fields", async () => {
  await withContractCopy(
    (root) =>
      mutateJson(
        root,
        "contracts/examples/v1/system.health.success.json",
        (example) => {
          example.durable = true;
        },
      ),
    (result) => assert.rejects(result, /additional properties/u),
  );
});

test("capability discovery examples cannot omit registry entries", async () => {
  await withContractCopy(
    (root) =>
      mutateJson(
        root,
        "contracts/examples/v1/system.capabilities.success.json",
        (example) => {
          example.capabilities.pop();
        },
      ),
    (result) =>
      assert.rejects(
        result,
        /complete generated public registry|must NOT have fewer than/u,
      ),
  );
});

test("problem examples reject secret-shaped values in ordinary fields", async () => {
  await withContractCopy(
    (root) =>
      mutateJson(
        root,
        "contracts/examples/v1/system.capabilities.forbidden.json",
        (example) => {
          example.detail = "a".repeat(64);
        },
      ),
    (result) => assert.rejects(result, /secret-shaped value/u),
  );
});

test("problem examples reject duplicate keys that hide secret-shaped values", async () => {
  await withContractCopy(
    async (root) => {
      const path = join(
        root,
        "contracts/examples/v1/system.capabilities.forbidden.json",
      );
      const source = await readFile(path, "utf8");
      await writeFile(
        path,
        source.replace(
          '  "detail": "request is not authorized for this capability",',
          '  "detail": "Bearer hidden-secret",\n  "detail": "request is not authorized for this capability",',
        ),
      );
    },
    (result) => assert.rejects(result, /Map keys must be unique/u),
  );
});

test("problem examples cannot drift from canonical recovery semantics", async () => {
  await withContractCopy(
    (root) =>
      mutateJson(
        root,
        "contracts/examples/v1/receipt.replay.receipt_not_found.json",
        (example) => {
          example.safe_state = "prior_state_retained";
        },
      ),
    (result) => assert.rejects(result, /canonical problem field safe_state/u),
  );
});

test("problem examples must match an operation response", async () => {
  await withContractCopy(
    (root) =>
      mutateJson(
        root,
        "contracts/generated/v1/conformance-openapi.json",
        (openapi) => {
          delete openapi.paths["/api/v1/receipts/{receipt_id}"].get.responses[
            "404"
          ];
        },
      ),
    (result) => assert.rejects(result, /status 404 is absent/u),
  );
});

test("embedded OpenAPI examples cannot drift from governed files", async () => {
  await withContractCopy(
    (root) =>
      mutateJson(
        root,
        "contracts/generated/v1/conformance-openapi.json",
        (openapi) => {
          openapi.paths["/api/v1/observations"].post.responses["507"].content[
            "application/problem+json"
          ].examples["observation.accept.capacity_exceeded"].value.status = 500;
        },
      ),
    (result) => assert.rejects(result, /embedded OpenAPI problem example/u),
  );
});

test("receipt stream events must match the AsyncAPI payload", async () => {
  await withContractCopy(
    (root) =>
      mutateJson(
        root,
        "contracts/examples/v1/receipt.stream.event.json",
        (example) => {
          delete example.receipt_id;
        },
      ),
    (result) => assert.rejects(result, /required property 'receipt_id'/u),
  );
});

test("linked-data examples cannot load a network context", async () => {
  await withContractCopy(
    (root) =>
      mutateJson(
        root,
        "contracts/examples/v1/observation.accept.receipt.jsonld",
        (example) => {
          example["@context"] = "https://example.com/context.jsonld";
        },
      ),
    (result) => assert.rejects(result, /network context/u),
  );
});

test("linked-data receipts must retain the governed DTO fields", async () => {
  await withContractCopy(
    (root) =>
      mutateJson(
        root,
        "contracts/examples/v1/observation.accept.receipt.jsonld",
        (example) => {
          delete example.evidenceId;
        },
      ),
    (result) => assert.rejects(result, /exact linked-data projection/u),
  );
});

test("linked-data receipt IRIs must agree with receipt IDs", async () => {
  await withContractCopy(
    (root) =>
      mutateJson(
        root,
        "contracts/examples/v1/observation.accept.receipt.jsonld",
        (example) => {
          example.id = "https://fasti.scrobble.dev/id/rcp_wrong";
        },
      ),
    (result) => assert.rejects(result, /receipt IRI and receipt ID disagree/u),
  );
});

test("linked-data receipts cannot commit before receipt", async () => {
  await withContractCopy(
    (root) =>
      mutateJson(
        root,
        "contracts/examples/v1/observation.accept.receipt.jsonld",
        (example) => {
          example.committedAt = "2026-08-21T17:44:15Z";
        },
      ),
    (result) => assert.rejects(result, /commits before it was received/u),
  );
});

test("linked-data receipt dates must be real RFC3339 calendar values", async () => {
  await withContractCopy(
    (root) =>
      mutateJson(
        root,
        "contracts/examples/v1/observation.accept.receipt.jsonld",
        (example) => {
          example.committedAt = "2026-02-30T00:00:00Z";
        },
      ),
    (result) => assert.rejects(result, /date-time/u),
  );
});

test("linked-data receipt date-time datatype drift is rejected", async () => {
  await withContractCopy(
    (root) =>
      mutateJson(root, "contracts/jsonld/v1/context.jsonld", (context) => {
        context["@context"].committedAt["@type"] = "xsd:string";
      }),
    (result) => assert.rejects(result, /xsd:dateTime/u),
  );
});
