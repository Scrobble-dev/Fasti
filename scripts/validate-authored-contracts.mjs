import { strict as assert } from "node:assert";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { Parser } from "@asyncapi/parser";
import jsonld from "jsonld";
import { parseDocument as parseYamlDocument } from "yaml";

import { readStrictJson } from "./lib/strict-json.mjs";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const assertInternalReferencesOnly = (value, path = "$") => {
  if (Array.isArray(value)) {
    value.forEach((child, index) =>
      assertInternalReferencesOnly(child, `${path}[${index}]`),
    );
    return;
  }
  if (value === null || typeof value !== "object") return;
  for (const [key, child] of Object.entries(value)) {
    if (key === "$ref") {
      assert.equal(typeof child, "string", `${path}.$ref must be a string`);
      assert.ok(
        child.startsWith("#/"),
        `${path} contains forbidden external reference ${child}`,
      );
    }
    assertInternalReferencesOnly(child, `${path}.${key}`);
  }
};

export async function validateAuthoredContracts(root = repositoryRoot) {
  const asyncApiPath = resolve(root, "contracts/asyncapi/v1/transport.yaml");
  const conformanceOpenApiPath = resolve(
    root,
    "contracts/generated/v1/conformance-openapi.json",
  );
  const problemCatalogPath = resolve(
    root,
    "contracts/generated/v1/problems.json",
  );
  const capabilityRegistryPath = resolve(
    root,
    "contracts/generated/v1/capabilities.json",
  );
  const contextPath = resolve(root, "contracts/jsonld/v1/context.jsonld");
  const vocabularyPath = resolve(root, "contracts/jsonld/v1/vocabulary.jsonld");
  const examplePath = resolve(
    root,
    "contracts/examples/v1/observation.accept.receipt.jsonld",
  );

  const asyncApiSource = await readFile(asyncApiPath, "utf8");
  const asyncApiDocument = parseYamlDocument(asyncApiSource, {
    uniqueKeys: true,
  });
  assert.deepEqual(
    asyncApiDocument.errors,
    [],
    `AsyncAPI YAML errors:\n${asyncApiDocument.errors.join("\n")}`,
  );
  const asyncApiValue = asyncApiDocument.toJS();
  assertInternalReferencesOnly(asyncApiValue);
  const conformanceOpenApi = await readStrictJson(conformanceOpenApiPath);
  const problemCatalog = await readStrictJson(problemCatalogPath);
  const capabilityRegistry = await readStrictJson(capabilityRegistryPath);
  const streamCapabilities = capabilityRegistry.capabilities.filter(
    ({ surface_profile: profileName }) =>
      capabilityRegistry.surface_profiles[profileName].sse_asyncapi.state ===
      "required",
  );
  assert.equal(
    streamCapabilities.length,
    1,
    "B1 must have exactly one AsyncAPI-bound capability",
  );
  const [streamCapability] = streamCapabilities;
  const asyncApiResult = await new Parser().parse(asyncApiSource, {
    source: asyncApiPath,
  });
  const blockingDiagnostics = asyncApiResult.diagnostics.filter(
    ({ severity }) => severity <= 1,
  );
  assert.deepEqual(
    blockingDiagnostics,
    [],
    `AsyncAPI diagnostics:\n${blockingDiagnostics
      .map(({ message, path }) => `${path?.join(".") ?? "$"}: ${message}`)
      .join("\n")}`,
  );
  assert.ok(
    asyncApiResult.document,
    "AsyncAPI parser did not return a document",
  );
  assert.equal(asyncApiValue.asyncapi, "3.1.0");
  assert.equal(
    asyncApiValue.operations.sendReceiptCommitted[
      "x-fasti-runtime-availability"
    ],
    streamCapability.lifecycle.runtime_availability,
  );
  assert.equal(
    asyncApiValue.operations.sendReceiptCommitted["x-fasti-durability"],
    "none",
    "the B1 receipt stream must not claim durable fixture delivery",
  );
  assert.equal(
    asyncApiValue.operations.sendReceiptCommitted["x-fasti-fixture-delivery"],
    "finite_replay_then_close",
    "the B1 receipt stream must disclose finite replay and clean close",
  );
  assert.equal(
    asyncApiValue.components.messages.receiptCommitted.payload.schemaFormat,
    "application/schema+json;version=draft-2020-12",
  );
  assert.equal(
    asyncApiValue.operations.sendReceiptCommitted["x-fasti-capability-id"],
    streamCapability.id,
  );
  assert.deepEqual(
    asyncApiValue.operations.sendReceiptCommitted["x-fasti-required-scopes"],
    streamCapability.scopes,
  );
  assert.deepEqual(
    asyncApiValue.operations.sendReceiptCommitted["x-fasti-http-problems"],
    {
      contentType: "application/problem+json",
      responses: problemCatalog.problems
        .filter(
          ({ capability_id: capabilityId }) =>
            capabilityId === streamCapability.id,
        )
        .map(({ status, code }) => ({ status, code }))
        .sort((left, right) => left.status - right.status),
    },
  );
  assert.deepEqual(
    asyncApiValue.operations.sendReceiptCommitted["x-fasti-example-ids"],
    streamCapability.examples,
  );
  const messageExampleIds = [];
  for (const id of streamCapability.examples) {
    const example = await readStrictJson(
      resolve(root, `contracts/examples/v1/${id}.json`),
    );
    if (!Object.hasOwn(example, "code")) messageExampleIds.push(id);
  }
  assert.deepEqual(
    asyncApiValue.components.messages.receiptCommitted["x-fasti-example-ids"],
    messageExampleIds,
  );
  assert.equal(
    asyncApiValue.components.messages.receiptCommitted[
      "x-fasti-sse-id-pointer"
    ],
    "$message.payload#/receipt_id",
    "receipt stream SSE cursor must be the payload receipt_id",
  );
  const eventProperties =
    asyncApiValue.components.messages.receiptCommitted.payload.schema
      .properties;
  const receiptProperties =
    conformanceOpenApi.components.schemas.ObservationReceiptDto.properties;
  for (const field of ["receipt_id", "operation_id", "observation_id"]) {
    const eventConstraint = Object.fromEntries(
      ["type", "format", "minLength", "maxLength", "pattern"].map((key) => [
        key,
        eventProperties[field][key],
      ]),
    );
    const receiptConstraint = Object.fromEntries(
      ["type", "format", "minLength", "maxLength", "pattern"].map((key) => [
        key,
        receiptProperties[field][key],
      ]),
    );
    assert.deepEqual(
      eventConstraint,
      receiptConstraint,
      `AsyncAPI ${field} must reuse every Utoipa receipt identifier constraint`,
    );
  }
  assert.deepEqual(
    eventProperties.committed_at,
    receiptProperties.committed_at,
    "AsyncAPI committed_at must reuse every Utoipa receipt timestamp constraint",
  );
  const resolutionSchemaName = receiptProperties.resolution.$ref
    .split("/")
    .at(-1);
  assert.equal(
    eventProperties.resolution.const,
    conformanceOpenApi.components.schemas[resolutionSchemaName].enum[0],
    "AsyncAPI resolution must reuse the sole Utoipa receipt resolution",
  );
  assert.equal(
    conformanceOpenApi.components.schemas[resolutionSchemaName].enum.length,
    1,
    "B1 receipt events require a single governed resolution",
  );
  assert.equal(
    eventProperties.correlation_id.pattern,
    "^req_[0-9a-f]{12}7[0-9a-f]{3}[89ab][0-9a-f]{15}$",
  );

  const loadedFileUrls = [];
  const expectedContextUrl = pathToFileURL(contextPath).href;
  const localContext = await readStrictJson(contextPath);
  const localDocumentLoader = async (url) => {
    assert.equal(
      url,
      expectedContextUrl,
      `network or undeclared file access is forbidden: ${url}`,
    );
    loadedFileUrls.push(url);
    return {
      contextUrl: null,
      documentUrl: url,
      document: localContext,
    };
  };

  const expandLocal = async (path) => {
    const url = pathToFileURL(path);
    const document = await readStrictJson(path);
    if (typeof document["@context"] === "string") {
      assert.ok(
        !/^https?:/u.test(document["@context"]),
        `network access is forbidden: ${document["@context"]}`,
      );
    }
    return jsonld.expand(document, {
      base: url.href,
      documentLoader: localDocumentLoader,
    });
  };

  const [expandedExample, expandedVocabulary] = await Promise.all([
    expandLocal(examplePath),
    expandLocal(vocabularyPath),
  ]);
  assert.equal(expandedExample.length, 1);
  assert.deepEqual(expandedExample[0]["@type"], [
    "https://fasti.scrobble.dev/ns/v1/AcceptObservationReceipt",
  ]);
  assert.deepEqual(
    expandedExample[0]["https://fasti.scrobble.dev/ns/v1/resolution"],
    [{ "@id": "https://fasti.scrobble.dev/ns/v1/resolution/unresolved" }],
  );
  for (const term of ["receivedAt", "committedAt"]) {
    assert.equal(
      expandedExample[0][`https://fasti.scrobble.dev/ns/v1/${term}`]?.[0]?.[
        "@type"
      ],
      "http://www.w3.org/2001/XMLSchema#dateTime",
      `${term} must expand as xsd:dateTime`,
    );
  }
  assert.ok(
    JSON.stringify(expandedVocabulary).includes(
      "https://fasti.scrobble.dev/ns/v1/Observation",
    ),
    "expanded vocabulary must define Observation",
  );
  assert.deepEqual(loadedFileUrls, [expectedContextUrl]);

  return {
    asyncApiVersion: asyncApiValue.asyncapi,
    expandedDocumentCount: expandedExample.length + expandedVocabulary.length,
  };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const requestedRoot = process.argv[2]
    ? resolve(process.argv[2])
    : repositoryRoot;
  await validateAuthoredContracts(requestedRoot);
  console.log(
    "PASS: authored AsyncAPI 3.1 and JSON-LD 1.1 contracts validate without network access",
  );
}
