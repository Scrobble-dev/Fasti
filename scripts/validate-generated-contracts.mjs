import { strict as assert } from "node:assert";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import SwaggerParser from "@apidevtools/swagger-parser";
import Ajv2020 from "ajv/dist/2020.js";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const readJson = async (root, relativePath) =>
  JSON.parse(await readFile(resolve(root, relativePath), "utf8"));

const visitReferences = (value, visit) => {
  if (Array.isArray(value)) {
    value.forEach((item) => visitReferences(item, visit));
    return;
  }
  if (value === null || typeof value !== "object") return;
  for (const [key, child] of Object.entries(value)) {
    if (key === "$ref") visit(child);
    visitReferences(child, visit);
  }
};

export async function validateGeneratedContracts(root = repositoryRoot) {
  const [openapi, healthSchema, problemSchema, registry] = await Promise.all([
    readJson(root, "contracts/generated/v1/openapi.json"),
    readJson(root, "packages/schemas/schemas/health-response.json"),
    readJson(root, "packages/schemas/schemas/problem-details.json"),
    readJson(root, "contracts/generated/v1/capabilities.json"),
  ]);

  visitReferences(openapi, (reference) => {
    assert.ok(
      reference.startsWith("#/"),
      `generated OpenAPI may not resolve a network or filesystem reference: ${reference}`,
    );
  });
  assert.equal(openapi.openapi, "3.1.0");
  assert.equal(
    openapi.paths["/api/v1/health"].get.responses["200"].content[
      "application/json"
    ].schema.$ref,
    "#/components/schemas/HealthResponse",
  );
  await SwaggerParser.validate(openapi);
  assert.deepEqual(Object.keys(openapi.paths), ["/api/v1/health"]);

  const ajv = new Ajv2020({ allErrors: true, strict: true });
  ajv.addFormat("uint16", {
    type: "number",
    validate: (value) =>
      Number.isInteger(value) && value >= 0 && value <= 65535,
  });
  const health = ajv.compile(healthSchema);
  const problem = ajv.compile(problemSchema);
  assert.equal(
    healthSchema.$schema,
    "https://json-schema.org/draft/2020-12/schema",
  );
  assert.equal(
    problemSchema.$schema,
    "https://json-schema.org/draft/2020-12/schema",
  );
  assert.ok(health({ status: "healthy", version: "0.1.0" }));
  assert.ok(
    !health({ status: "healthy", version: "0.1.0", committed: true }),
    "health schema must reject undeclared fields",
  );
  assert.ok(
    problem({
      type: "https://fasti.scrobble.dev/v1/problems/forbidden",
      title: "Forbidden",
      status: 403,
      detail: "request is not authorized for this capability",
      code: "forbidden",
      capability_id: "observation.accept",
      safe_state: "no_mutation",
      retryability: "not_retryable",
      next_actions: [],
      correlation_id: "req_018f0e0e7f7b70008000000000000000",
      param: null,
      actual: null,
      violations: [],
    }),
    ajv.errorsText(problem.errors),
  );

  assert.equal(registry.contract_version, "1.0.0");
  assert.equal(registry.capability_base_uri.endsWith("/v1/"), true);
  assert.equal(registry.capabilities.length, 21);
  const capabilityIds = registry.capabilities.map(({ id }) => id);
  assert.equal(new Set(capabilityIds).size, capabilityIds.length);
  assert.deepEqual(capabilityIds, [...capabilityIds].sort());
  for (const capability of registry.capabilities) {
    assert.equal(
      Object.hasOwn(capability, "application_key"),
      false,
      `${capability.id} leaked an internal application key`,
    );
    assert.ok(
      Object.hasOwn(registry.surface_profiles, capability.surface_profile),
      `${capability.id} references an unknown surface profile`,
    );
  }

  return {
    capabilityCount: registry.capabilities.length,
    openApiPathCount: Object.keys(openapi.paths).length,
    schemaCount: 2,
  };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const requestedRoot = process.argv[2]
    ? resolve(process.argv[2])
    : repositoryRoot;
  const result = await validateGeneratedContracts(requestedRoot);
  console.log(
    `PASS: generated OpenAPI, JSON Schema 2020-12, and ${result.capabilityCount} capability declarations validate without external references`,
  );
}
