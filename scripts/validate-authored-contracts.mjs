import { strict as assert } from "node:assert";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { Parser } from "@asyncapi/parser";
import jsonld from "jsonld";
import { parse as parseYaml } from "yaml";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const asyncApiPath = resolve(
  repositoryRoot,
  "contracts/asyncapi/v1/transport.yaml",
);
const contextPath = resolve(
  repositoryRoot,
  "contracts/jsonld/v1/context.jsonld",
);
const vocabularyPath = resolve(
  repositoryRoot,
  "contracts/jsonld/v1/vocabulary.jsonld",
);
const examplePath = resolve(
  repositoryRoot,
  "contracts/examples/v1/observation-accepted.jsonld",
);

const asyncApiSource = await readFile(asyncApiPath, "utf8");
const asyncApiValue = parseYaml(asyncApiSource);
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
assert.ok(asyncApiResult.document, "AsyncAPI parser did not return a document");
assert.equal(asyncApiValue.asyncapi, "3.1.0");
assert.equal(
  asyncApiValue.operations.sendReceiptCommitted["x-fasti-runtime-availability"],
  "fixture_only",
);
assert.equal(
  asyncApiValue.components.messages.receiptCommitted.payload.schemaFormat,
  "application/schema+json;version=draft-07",
);

const loadedFileUrls = [];
const localDocumentLoader = async (url) => {
  assert.ok(url.startsWith("file:"), `network access is forbidden: ${url}`);
  loadedFileUrls.push(url);
  return {
    contextUrl: null,
    documentUrl: url,
    document: JSON.parse(await readFile(fileURLToPath(url), "utf8")),
  };
};

const expandLocal = async (path) => {
  const url = pathToFileURL(path);
  const document = JSON.parse(await readFile(path, "utf8"));
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
assert.ok(
  JSON.stringify(expandedVocabulary).includes(
    "https://fasti.scrobble.dev/ns/v1/Observation",
  ),
  "expanded vocabulary must define Observation",
);
assert.deepEqual(loadedFileUrls, [pathToFileURL(contextPath).href]);

console.log(
  "PASS: authored AsyncAPI 3.1 and JSON-LD 1.1 contracts validate without network access",
);
