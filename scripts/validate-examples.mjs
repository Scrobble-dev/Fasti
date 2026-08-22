import { strict as assert } from "node:assert";
import { readdir, readFile } from "node:fs/promises";
import { dirname, extname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import Ajv2020 from "ajv/dist/2020.js";
import jsonld from "jsonld";
import { parse as parseYaml } from "yaml";

import { readStrictJson } from "./lib/strict-json.mjs";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const EXAMPLE_DIRECTORY = "contracts/examples/v1";

const isLeapYear = (year) =>
  year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);

const isStrictRfc3339 = (value) => {
  const match =
    /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.\d{1,9})?(Z|([+-])(\d{2}):(\d{2}))$/u.exec(
      value,
    );
  if (!match) return false;
  const [, yearText, monthText, dayText, hourText, minuteText, secondText] =
    match;
  const [year, month, day, hour, minute, second] = [
    yearText,
    monthText,
    dayText,
    hourText,
    minuteText,
    secondText,
  ].map(Number);
  const daysInMonth = [
    31,
    isLeapYear(year) ? 29 : 28,
    31,
    30,
    31,
    30,
    31,
    31,
    30,
    31,
    30,
    31,
  ];
  if (
    year === 0 ||
    month < 1 ||
    month > 12 ||
    day < 1 ||
    day > daysInMonth[month - 1] ||
    hour > 23 ||
    minute > 59 ||
    second > 60
  ) {
    return false;
  }
  if (match[8] === undefined) return true;
  return Number(match[9]) <= 23 && Number(match[10]) <= 59;
};

const readJson = async (root, relativePath) =>
  readStrictJson(resolve(root, relativePath));

const addContractFormats = (ajv) => {
  for (const format of [
    "fasti-client-id",
    "fasti-evidence-id",
    "fasti-observation-id",
    "fasti-operation-id",
    "fasti-profile-id",
    "fasti-receipt-id",
    "fasti-workspace-id",
    "iso-date-or-rfc3339",
    "opaque-secret",
    "sha256",
  ]) {
    ajv.addFormat(format, true);
  }
  ajv.addFormat("date-time", {
    type: "string",
    validate: isStrictRfc3339,
  });
  ajv.addFormat("int32", {
    type: "number",
    validate: (value) =>
      Number.isInteger(value) && value >= -2147483648 && value <= 2147483647,
  });
  ajv.addFormat("int64", {
    type: "number",
    validate: Number.isSafeInteger,
  });
  ajv.addFormat("uint16", {
    type: "number",
    validate: (value) =>
      Number.isInteger(value) && value >= 0 && value <= 65535,
  });
};

const asJsonSchemaDefinitions = (value) => {
  if (Array.isArray(value)) return value.map(asJsonSchemaDefinitions);
  if (value === null || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value).map(([key, child]) => [
      key,
      key === "$ref" && typeof child === "string"
        ? child.replace("#/components/schemas/", "#/$defs/")
        : asJsonSchemaDefinitions(child),
    ]),
  );
};

const compileOpenApiComponent = (ajv, openapi, component) =>
  ajv.compile({
    $schema: "https://json-schema.org/draft/2020-12/schema",
    $defs: asJsonSchemaDefinitions(openapi.components.schemas),
    $ref: `#/$defs/${component}`,
  });

const assertValid = (validator, value, label, ajv) => {
  assert.ok(validator(value), `${label}: ${ajv.errorsText(validator.errors)}`);
};

const exampleId = (filename) => filename.slice(0, -extname(filename).length);

const assertNoSensitiveRepresentation = (value, path = "$") => {
  if (typeof value === "string") {
    assert.doesNotMatch(
      value,
      /\bBearer\s+\S+/iu,
      `${path} contains a bearer value`,
    );
    assert.doesNotMatch(
      value,
      /^(?:[0-9a-f]{64}|eyJ[A-Za-z0-9_-]{20,}(?:\.[A-Za-z0-9_-]+){1,2})$/u,
      `${path} contains a secret-shaped value`,
    );
    return;
  }
  if (Array.isArray(value)) {
    value.forEach((item, index) =>
      assertNoSensitiveRepresentation(item, `${path}[${index}]`),
    );
    return;
  }
  if (value === null || typeof value !== "object") return;
  for (const [key, child] of Object.entries(value)) {
    assert.doesNotMatch(
      key,
      /(?:credential|initialization_proof|secret|token)/iu,
      `${path}.${key} exposes a credential or secret field`,
    );
    assertNoSensitiveRepresentation(child, `${path}.${key}`);
  }
};

const indexExampleOwners = (registry) => {
  const capabilities = new Map(
    registry.capabilities.map((capability) => [capability.id, capability]),
  );
  const owners = new Map();
  for (const capability of registry.capabilities) {
    for (const id of capability.examples) {
      assert.ok(!owners.has(id), `${id} has more than one capability owner`);
      owners.set(id, capability);
    }
  }
  return { capabilities, owners };
};

const validateLinkedDataReceipt = async (
  root,
  path,
  owner,
  receiptValidator,
  ajv,
) => {
  const document = await readStrictJson(path);
  assert.equal(document.capabilityId, owner.id);
  assert.equal(document.type, "AcceptObservationReceipt");
  assert.equal(typeof document["@context"], "string");
  assert.deepEqual(
    Object.keys(document).sort(),
    [
      "@context",
      "capabilityId",
      "committedAt",
      "evidenceId",
      "id",
      "observationId",
      "operationId",
      "payloadDigest",
      "profileId",
      "receiptId",
      "receivedAt",
      "resolution",
      "sourceClientId",
      "type",
      "workspaceId",
    ],
    `${path} must be an exact linked-data projection of ObservationReceiptDto`,
  );
  assert.ok(
    !/^https?:/u.test(document["@context"]),
    `${path} attempts to load a network context`,
  );
  assert.deepEqual(
    Object.hasOwn(document, "recordId") ||
      Object.hasOwn(document, "occurrenceId"),
    false,
    `${path} invents resolved identity during observation acceptance`,
  );
  assert.equal(
    document.id,
    `https://fasti.scrobble.dev/id/${document.receiptId}`,
    `${path} receipt IRI and receipt ID disagree`,
  );
  const receipt = {
    receipt_id: document.receiptId,
    operation_id: document.operationId,
    workspace_id: document.workspaceId,
    profile_id: document.profileId,
    source_client_id: document.sourceClientId,
    observation_id: document.observationId,
    evidence_id: document.evidenceId,
    payload_digest: document.payloadDigest,
    resolution: document.resolution,
    received_at: document.receivedAt,
    committed_at: document.committedAt,
  };
  assertValid(receiptValidator, receipt, `${path} receipt`, ajv);
  assert.ok(
    Date.parse(receipt.committed_at) >= Date.parse(receipt.received_at),
    `${path} commits before it was received`,
  );
  const contextPath = resolve(dirname(path), document["@context"]);
  const contextUrl = pathToFileURL(contextPath).href;
  const expanded = await jsonld.expand(document, {
    base: pathToFileURL(path).href,
    documentLoader: async (url) => {
      assert.equal(
        url,
        contextUrl,
        `network or undeclared context load: ${url}`,
      );
      return {
        contextUrl: null,
        documentUrl: url,
        document: await readJson(root, "contracts/jsonld/v1/context.jsonld"),
      };
    },
  });
  assert.equal(expanded.length, 1);
  assert.deepEqual(expanded[0]["@type"], [
    "https://fasti.scrobble.dev/ns/v1/AcceptObservationReceipt",
  ]);
  assert.deepEqual(
    expanded[0]["https://fasti.scrobble.dev/ns/v1/capabilityId"],
    [{ "@value": owner.id }],
  );
  const valueTerms = {
    receiptId: "receiptId",
    operationId: "operationId",
    workspaceId: "workspaceId",
    profileId: "profileId",
    sourceClientId: "sourceClientId",
    observationId: "observationId",
    evidenceId: "evidenceId",
    payloadDigest: "payloadDigest",
    receivedAt: "receivedAt",
    committedAt: "committedAt",
  };
  for (const [compact, term] of Object.entries(valueTerms)) {
    assert.equal(
      expanded[0][`https://fasti.scrobble.dev/ns/v1/${term}`]?.[0]?.["@value"],
      document[compact],
      `${path} loses ${compact} during JSON-LD expansion`,
    );
  }
  const dateTimeType = "http://www.w3.org/2001/XMLSchema#dateTime";
  for (const term of ["receivedAt", "committedAt"]) {
    assert.equal(
      expanded[0][`https://fasti.scrobble.dev/ns/v1/${term}`]?.[0]?.["@type"],
      dateTimeType,
      `${path} must preserve ${term} as xsd:dateTime`,
    );
  }
  assert.deepEqual(expanded[0]["https://fasti.scrobble.dev/ns/v1/resolution"], [
    { "@id": "https://fasti.scrobble.dev/ns/v1/resolution/unresolved" },
  ]);
};

export async function validateExamples(root = repositoryRoot) {
  const [
    registry,
    openapi,
    conformanceOpenapi,
    problemCatalog,
    healthSchema,
    asyncApiSource,
  ] = await Promise.all([
    readJson(root, "contracts/generated/v1/capabilities.json"),
    readJson(root, "contracts/generated/v1/openapi.json"),
    readJson(root, "contracts/generated/v1/conformance-openapi.json"),
    readJson(root, "contracts/generated/v1/problems.json"),
    readJson(root, "packages/schemas/schemas/health-response.json"),
    readFile(resolve(root, "contracts/asyncapi/v1/transport.yaml"), "utf8"),
  ]);
  const asyncApi = parseYaml(asyncApiSource);
  const { capabilities, owners } = indexExampleOwners(registry);
  const problems = new Map(
    problemCatalog.problems.map((problem) => [
      `${problem.capability_id}:${problem.code}`,
      problem,
    ]),
  );
  assert.ok(owners.size > 0, "the registry must govern at least one example");

  const files = (await readdir(resolve(root, EXAMPLE_DIRECTORY)))
    .filter((filename) => [".json", ".jsonld"].includes(extname(filename)))
    .sort();
  const present = new Set(files.map(exampleId));
  assert.deepEqual(
    [...present].sort(),
    [...owners.keys()].sort(),
    "the example directory must contain exactly one file for every registry example",
  );

  const ajv = new Ajv2020({ allErrors: true, strict: true });
  addContractFormats(ajv);
  const health = ajv.compile(healthSchema);
  const problem = compileOpenApiComponent(
    ajv,
    conformanceOpenapi,
    "ProblemDetails",
  );
  const capabilityDiscovery = compileOpenApiComponent(
    ajv,
    conformanceOpenapi,
    "CapabilityDiscoveryResponse",
  );
  const observationReceipt = compileOpenApiComponent(
    ajv,
    conformanceOpenapi,
    "ObservationReceiptDto",
  );
  const receiptEvent = ajv.compile(
    asyncApi.components.messages.receiptCommitted.payload.schema,
  );
  const productionHealthResponse =
    openapi.paths["/api/v1/health"].get.responses["200"].content[
      "application/json"
    ].schema.$ref;
  assert.equal(productionHealthResponse, "#/components/schemas/HealthResponse");
  const httpOperations = new Map();
  for (const [path, pathItem] of Object.entries(conformanceOpenapi.paths)) {
    for (const [method, operation] of Object.entries(pathItem)) {
      const capabilityId = operation["x-fasti-capability-id"];
      assert.equal(
        typeof capabilityId,
        "string",
        `${method} ${path} has no capability ID`,
      );
      assert.ok(
        !httpOperations.has(capabilityId),
        `${capabilityId} has more than one finite HTTP operation`,
      );
      httpOperations.set(capabilityId, { method, operation, path });
    }
  }
  const productionHealthOperation = openapi.paths["/api/v1/health"].get;
  httpOperations.set(productionHealthOperation["x-fasti-capability-id"], {
    method: "get",
    operation: productionHealthOperation,
    path: "/api/v1/health",
  });
  const streamProblems =
    asyncApi.operations.sendReceiptCommitted["x-fasti-http-problems"];
  assert.equal(streamProblems.contentType, "application/problem+json");

  let linkedDataCount = 0;
  let problemCount = 0;
  for (const filename of files) {
    const id = exampleId(filename);
    const owner = owners.get(id);
    const path = resolve(root, EXAMPLE_DIRECTORY, filename);
    if (extname(filename) === ".jsonld") {
      await validateLinkedDataReceipt(
        root,
        path,
        owner,
        observationReceipt,
        ajv,
      );
      assert.ok(
        httpOperations
          .get(owner.id)
          ?.operation["x-fasti-example-ids"].includes(id),
        `${id} is not linked from its owning OpenAPI operation`,
      );
      linkedDataCount += 1;
      continue;
    }

    const value = await readStrictJson(path);
    assertNoSensitiveRepresentation(value);
    if (id === "system.health.success") {
      assertValid(health, value, id, ajv);
      assert.equal(value.status, "healthy");
      assert.deepEqual(
        httpOperations.get(owner.id).operation.responses["200"].content[
          "application/json"
        ].examples[id].value,
        value,
        `${id} differs from the embedded production OpenAPI example`,
      );
      continue;
    }
    if (id === "system.capabilities.success") {
      assertValid(capabilityDiscovery, value, id, ajv);
      assert.deepEqual(
        value.capabilities,
        registry.capabilities,
        `${id} must contain the complete generated public registry in canonical order`,
      );
      assert.deepEqual(
        httpOperations.get(owner.id).operation.responses["200"].content[
          "application/json"
        ].examples[id].value,
        value,
        `${id} differs from the embedded conformance OpenAPI example`,
      );
      continue;
    }
    if (id === "receipt.stream.event") {
      assert.equal(owner.id, "receipt.stream");
      assertValid(receiptEvent, value, id, ajv);
      assert.equal(value.capability_id, "observation.accept");
      assert.equal(Object.hasOwn(value, "record_id"), false);
      assert.equal(Object.hasOwn(value, "occurrence_id"), false);
      assert.ok(
        asyncApi.components.messages.receiptCommitted[
          "x-fasti-example-ids"
        ].includes(id),
        `${id} is not linked from the AsyncAPI message`,
      );
      assert.ok(
        asyncApi.operations.sendReceiptCommitted[
          "x-fasti-example-ids"
        ].includes(id),
        `${id} is not linked from the AsyncAPI operation`,
      );
      continue;
    }

    assertValid(problem, value, id, ajv);
    assert.equal(value.capability_id, owner.id, `${id} has the wrong owner`);
    assert.ok(
      owner.problems.includes(value.code),
      `${id} uses ungoverned problem ${value.code}`,
    );
    const canonicalProblem = problems.get(`${owner.id}:${value.code}`);
    assert.ok(canonicalProblem, `${id} has no canonical problem entry`);
    assert.ok(
      id.endsWith(`.${value.code}`),
      `${id} must end with canonical problem code ${value.code}`,
    );
    for (const field of [
      "type",
      "title",
      "status",
      "detail",
      "safe_state",
      "retryability",
      "next_actions",
      "param",
    ]) {
      assert.deepEqual(
        value[field],
        canonicalProblem[field],
        `${id} drifts from canonical problem field ${field}`,
      );
    }
    assert.equal(value.actual, null, `${id} must not echo submitted data`);
    if (owner.id === "receipt.stream") {
      assert.ok(
        asyncApi.operations.sendReceiptCommitted[
          "x-fasti-example-ids"
        ].includes(id),
        `${id} is not linked from the AsyncAPI operation`,
      );
      assert.ok(
        streamProblems.responses.some(
          ({ status, code }) => status === value.status && code === value.code,
        ),
        `${id} has no matching AsyncAPI HTTP problem binding`,
      );
    } else {
      const binding = httpOperations.get(owner.id);
      assert.ok(binding, `${id} has no finite HTTP operation`);
      const response = binding.operation.responses[String(value.status)];
      assert.ok(
        response,
        `${id} status ${value.status} is absent from ${binding.method} ${binding.path}`,
      );
      assert.equal(
        response.content?.["application/problem+json"]?.schema?.$ref,
        "#/components/schemas/ProblemDetails",
        `${id} is not bound to application/problem+json ProblemDetails`,
      );
      assert.deepEqual(
        response.content["application/problem+json"].examples[id].value,
        value,
        `${id} differs from the embedded OpenAPI problem example`,
      );
    }
    problemCount += 1;
  }

  assert.equal(linkedDataCount, 1, "B1 must govern one linked-data receipt");
  return { exampleCount: files.length, linkedDataCount, problemCount };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const requestedRoot = process.argv[2]
    ? resolve(process.argv[2])
    : repositoryRoot;
  const result = await validateExamples(requestedRoot);
  console.log(
    `PASS: ${result.exampleCount} governed examples validate against registry, OpenAPI 3.1, JSON Schema 2020-12, and local JSON-LD`,
  );
}
