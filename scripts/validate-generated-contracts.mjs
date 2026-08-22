import { strict as assert } from "node:assert";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import SwaggerParser from "@apidevtools/swagger-parser";
import Ajv2020 from "ajv/dist/2020.js";

import { readStrictJson } from "./lib/strict-json.mjs";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const readJson = async (root, relativePath) =>
  readStrictJson(resolve(root, relativePath));

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
  const [
    openapi,
    conformanceOpenapi,
    healthSchema,
    problemSchema,
    registry,
    problemCatalog,
  ] = await Promise.all([
    readJson(root, "contracts/generated/v1/openapi.json"),
    readJson(root, "contracts/generated/v1/conformance-openapi.json"),
    readJson(root, "packages/schemas/schemas/health-response.json"),
    readJson(root, "packages/schemas/schemas/problem-details.json"),
    readJson(root, "contracts/generated/v1/capabilities.json"),
    readJson(root, "contracts/generated/v1/problems.json"),
  ]);

  for (const document of [openapi, conformanceOpenapi]) {
    visitReferences(document, (reference) => {
      assert.ok(
        reference.startsWith("#/"),
        `generated OpenAPI may not resolve a network or filesystem reference: ${reference}`,
      );
    });
  }
  assert.equal(openapi.openapi, "3.1.0");
  assert.equal(conformanceOpenapi.openapi, "3.1.0");
  assert.equal(
    openapi.paths["/api/v1/health"].get.responses["200"].content[
      "application/json"
    ].schema.$ref,
    "#/components/schemas/HealthResponse",
  );
  await Promise.all([
    SwaggerParser.validate(openapi),
    SwaggerParser.validate(conformanceOpenapi),
  ]);
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
      next_actions: [
        {
          id: "verify_request_authorization",
          label: "Verify the request context and local grant",
        },
      ],
      correlation_id: "req_018f0e0e7f7b70008000000000000000",
      param: null,
      actual: null,
      violations: [],
    }),
    ajv.errorsText(problem.errors),
  );

  assert.equal(registry.contract_version, "1.0.0");
  assert.equal(registry.capability_base_uri.endsWith("/v1/"), true);
  assert.equal(registry.capabilities.length, 22);
  const capabilityIds = registry.capabilities.map(({ id }) => id);
  assert.equal(new Set(capabilityIds).size, capabilityIds.length);
  assert.deepEqual(capabilityIds, [...capabilityIds].sort());
  const capabilities = new Map(
    registry.capabilities.map((capability) => [capability.id, capability]),
  );
  const expectedProfile = (capability) => {
    if (capability.lifecycle.contract_state === "reserved") {
      return `later_${capability.contract_body}`;
    }
    if (capability.id === "system.health") return "health";
    if (capability.id === "observation.accept") return "b1_observation_accept";
    if (capability.id === "receipt.replay") return "b1_receipt_replay";
    if (capability.id === "receipt.stream") return "b1_receipt_stream";
    return "b1_http_fixture";
  };
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
    assert.equal(
      capability.surface_profile,
      expectedProfile(capability),
      `${capability.id} uses a surface profile incompatible with its lifecycle`,
    );
  }

  const problemPairs = new Set();
  for (const entry of problemCatalog.problems) {
    const capability = capabilities.get(entry.capability_id);
    assert.ok(
      capability,
      `problem catalogue references ${entry.capability_id}`,
    );
    assert.ok(
      capability.problems.includes(entry.code),
      `${entry.capability_id}:${entry.code} is absent from the capability registry`,
    );
    const pair = `${entry.capability_id}:${entry.code}`;
    assert.ok(!problemPairs.has(pair), `duplicate canonical problem ${pair}`);
    problemPairs.add(pair);
    assert.equal(entry.type.startsWith("https://fasti.scrobble.dev/"), true);
    assert.equal(
      entry.next_actions.length,
      1,
      `${pair} needs one canonical action`,
    );
  }
  const expectedProblemPairs = new Set(
    registry.capabilities.flatMap((capability) =>
      capability.problems.map((code) => `${capability.id}:${code}`),
    ),
  );
  assert.deepEqual(
    [...problemPairs].sort(),
    [...expectedProblemPairs].sort(),
    "canonical problem catalogue must cover every registry pair exactly",
  );

  const operations = [];
  for (const [path, pathItem] of Object.entries(conformanceOpenapi.paths)) {
    for (const [method, operation] of Object.entries(pathItem)) {
      operations.push({ method, operation, path });
    }
  }
  assert.equal(operations.length, 9);
  const operationCapabilities = new Set();
  for (const { method, operation, path } of operations) {
    const capability = capabilities.get(operation["x-fasti-capability-id"]);
    assert.ok(capability, `${method} ${path} has no registry capability`);
    assert.ok(!operationCapabilities.has(capability.id));
    operationCapabilities.add(capability.id);
    assert.equal(operation["x-fasti-authorization"], capability.authorization);
    assert.deepEqual(operation["x-fasti-required-scopes"], capability.scopes);
    assert.equal(
      operation["x-fasti-runtime-availability"],
      capability.lifecycle.runtime_availability,
    );
    assert.deepEqual(operation["x-fasti-problem-codes"], capability.problems);
    assert.deepEqual(operation["x-fasti-example-ids"], capability.examples);
    for (const code of capability.problems) {
      const canonical = problemCatalog.problems.find(
        (problem) =>
          problem.capability_id === capability.id && problem.code === code,
      );
      assert.ok(
        Object.hasOwn(operation.responses, String(canonical.status)),
        `${method} ${path} omits status ${canonical.status} for ${code}`,
      );
    }
  }
  const expectedFiniteCapabilities = registry.capabilities
    .filter(
      (capability) =>
        registry.surface_profiles[capability.surface_profile].http_openapi
          .state === "required" && capability.id !== "system.health",
    )
    .map(({ id }) => id)
    .sort();
  assert.deepEqual(
    [...operationCapabilities].sort(),
    expectedFiniteCapabilities,
    "conformance OpenAPI operations must exactly cover required finite bindings",
  );

  const healthOperation = openapi.paths["/api/v1/health"].get;
  const healthCapability = capabilities.get("system.health");
  assert.equal(healthOperation["x-fasti-capability-id"], healthCapability.id);
  assert.equal(
    healthOperation["x-fasti-authorization"],
    healthCapability.authorization,
  );
  assert.deepEqual(
    healthOperation["x-fasti-required-scopes"],
    healthCapability.scopes,
  );
  assert.equal(
    healthOperation["x-fasti-runtime-availability"],
    healthCapability.lifecycle.runtime_availability,
  );
  assert.deepEqual(
    healthOperation["x-fasti-example-ids"],
    healthCapability.examples,
  );

  const httpOperations = new Map(
    operations.map(({ operation }) => [
      operation["x-fasti-capability-id"],
      operation,
    ]),
  );
  httpOperations.set("system.health", healthOperation);
  for (const capability of registry.capabilities.filter(
    ({ contract_body: contractBody, lifecycle }) =>
      contractBody === "b1" && lifecycle.contract_state === "finalized",
  )) {
    const profile = registry.surface_profiles[capability.surface_profile];
    for (const [surface, disposition] of Object.entries(profile)) {
      if (disposition.state !== "required") continue;
      if (disposition.binding_visibility === "internal") {
        assert.equal(surface, "domain_application");
        assert.equal(disposition.binding, undefined);
        continue;
      }
      assert.equal(disposition.binding_visibility, "public");
      assert.equal(typeof disposition.binding, "string");
      const binding = disposition.binding.replace(
        "{capability_id}",
        capability.id,
      );
      switch (surface) {
        case "http_openapi":
          assert.equal(binding, `openapi:${capability.id}`);
          assert.ok(httpOperations.has(capability.id));
          break;
        case "sse_asyncapi":
          assert.equal(binding, `asyncapi:${capability.id}`);
          assert.equal(capability.id, "receipt.stream");
          break;
        case "cli":
          assert.equal(binding, "cli:capability-discovery");
          break;
        case "json_schema":
          if (binding === "schema:health-response") {
            assert.equal(capability.id, "system.health");
          } else if (binding.startsWith("schema:openapi-operation:")) {
            assert.ok(httpOperations.has(capability.id));
          } else {
            assert.equal(binding, "schema:asyncapi-message:receiptCommitted");
            assert.equal(capability.id, "receipt.stream");
          }
          break;
        case "json_ld":
          assert.equal(binding, "json-ld:observation-receipt");
          assert.ok(
            ["observation.accept", "receipt.replay"].includes(capability.id),
          );
          break;
        case "okf":
          assert.equal(binding, "okf:capability-catalog");
          break;
        case "sdk":
          assert.equal(binding, `sdk:${capability.id}`);
          break;
        case "knowledge":
          assert.equal(binding, "knowledge:problem-catalog");
          assert.ok(
            problemCatalog.problems.some(
              ({ capability_id: capabilityId }) =>
                capabilityId === capability.id,
            ),
          );
          break;
        case "package_smoke":
          assert.equal(
            binding,
            capability.id === "system.health"
              ? "package-smoke:production-health"
              : "package-smoke:b1-conformance-fixture",
          );
          break;
        default:
          assert.fail(`unresolved required surface ${surface}`);
      }
    }
  }

  return {
    capabilityCount: registry.capabilities.length,
    conformanceOpenApiPathCount: Object.keys(conformanceOpenapi.paths).length,
    openApiPathCount: Object.keys(openapi.paths).length,
    problemCount: problemCatalog.problems.length,
    schemaCount: 2,
  };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const requestedRoot = process.argv[2]
    ? resolve(process.argv[2])
    : repositoryRoot;
  const result = await validateGeneratedContracts(requestedRoot);
  console.log(
    `PASS: production and conformance OpenAPI, JSON Schema 2020-12, ${result.capabilityCount} capabilities, and ${result.problemCount} canonical problems validate without external references`,
  );
}
