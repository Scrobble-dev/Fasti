import assert from "node:assert/strict";
import { cp, mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

import { validateGeneratedContracts } from "../../scripts/validate-generated-contracts.mjs";

const repositoryRoot = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "../..",
);

const withArtifacts = async (mutate, assertRejected) => {
  const root = await mkdtemp(join(tmpdir(), "fasti-generated-contracts-"));
  try {
    for (const directory of [
      "contracts/generated/v1",
      "packages/schemas/schemas",
    ]) {
      await mkdir(join(root, directory), { recursive: true });
      await cp(join(repositoryRoot, directory), join(root, directory), {
        recursive: true,
      });
    }
    await mutate(root);
    await assertRejected(validateGeneratedContracts(root));
  } finally {
    await rm(root, { recursive: true, force: true });
  }
};

const mutateJson = async (root, relativePath, mutate) => {
  const path = join(root, relativePath);
  const value = JSON.parse(await readFile(path, "utf8"));
  mutate(value);
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`);
};

test("checked-in generated contracts validate", async () => {
  assert.deepEqual(await validateGeneratedContracts(), {
    capabilityCount: 22,
    conformanceOpenApiPathCount: 9,
    openApiPathCount: 1,
    problemCount: 61,
    schemaCount: 2,
  });
});

test("generated contracts reject duplicate JSON keys", async () => {
  await withArtifacts(
    async (root) => {
      const path = join(root, "contracts/generated/v1/openapi.json");
      const source = await readFile(path, "utf8");
      await writeFile(
        path,
        source.replace(
          '  "openapi": "3.1.0",',
          '  "openapi": "0.0.0",\n  "openapi": "3.1.0",',
        ),
      );
    },
    (result) => assert.rejects(result, /Map keys must be unique/u),
  );
});

test("OpenAPI version mutation is rejected", async () => {
  await withArtifacts(
    (root) =>
      mutateJson(root, "contracts/generated/v1/openapi.json", (document) => {
        document.openapi = "3.0.3";
      }),
    (result) => assert.rejects(result, /3\.1\.0/),
  );
});

test("conformance OpenAPI structural mutation is rejected", async () => {
  await withArtifacts(
    (root) =>
      mutateJson(
        root,
        "contracts/generated/v1/conformance-openapi.json",
        (document) => {
          document.openapi = "3.0.3";
        },
      ),
    (result) => assert.rejects(result, /3\.1\.0/u),
  );
});

test("conformance OpenAPI operation mutation is rejected", async () => {
  await withArtifacts(
    (root) =>
      mutateJson(
        root,
        "contracts/generated/v1/conformance-openapi.json",
        (document) => {
          delete document.paths["/api/v1/capabilities"].get.responses;
        },
      ),
    (result) => assert.rejects(result),
  );
});

test("OpenAPI authorization posture mutation is rejected", async () => {
  await withArtifacts(
    (root) =>
      mutateJson(
        root,
        "contracts/generated/v1/conformance-openapi.json",
        (document) => {
          document.paths["/api/v1/node/initialization"].post[
            "x-fasti-authorization"
          ] = "scoped";
        },
      ),
    (result) => assert.rejects(result),
  );
});

test("JSON Schema dialect mutation is rejected", async () => {
  await withArtifacts(
    (root) =>
      mutateJson(
        root,
        "packages/schemas/schemas/health-response.json",
        (schema) => {
          schema.$schema = "http://json-schema.org/draft-07/schema#";
        },
      ),
    (result) => assert.rejects(result),
  );
});

test("capability identity mutation is rejected", async () => {
  await withArtifacts(
    (root) =>
      mutateJson(
        root,
        "contracts/generated/v1/capabilities.json",
        (registry) => {
          registry.capabilities[1].id = registry.capabilities[0].id;
        },
      ),
    (result) => assert.rejects(result),
  );
});

test("surface profile compatibility mutation is rejected", async () => {
  await withArtifacts(
    (root) =>
      mutateJson(
        root,
        "contracts/generated/v1/capabilities.json",
        (registry) => {
          registry.capabilities.find(
            ({ id }) => id === "observation.accept",
          ).surface_profile = "later_b2";
        },
      ),
    (result) => assert.rejects(result, /incompatible with its lifecycle/u),
  );
});

test("required surface binding mutations are rejected", async () => {
  await withArtifacts(
    (root) =>
      mutateJson(
        root,
        "contracts/generated/v1/capabilities.json",
        (registry) => {
          registry.surface_profiles.health.package_smoke.binding =
            "package-smoke:unproven";
        },
      ),
    (result) => assert.rejects(result),
  );
});

test("canonical problem coverage mutation is rejected", async () => {
  await withArtifacts(
    (root) =>
      mutateJson(root, "contracts/generated/v1/problems.json", (catalogue) => {
        catalogue.problems.pop();
      }),
    (result) => assert.rejects(result, /cover every registry pair/u),
  );
});
