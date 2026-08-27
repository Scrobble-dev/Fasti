import { strict as assert } from "node:assert";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { Parser } from "@asyncapi/parser";
import { parseDocument as parseYamlDocument } from "yaml";

import { readStrictJson } from "./lib/strict-json.mjs";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

export async function validateIntegrationContracts(root = repositoryRoot) {
  const asyncApiPath = resolve(root, "contracts/asyncapi/v1/integrations.yaml");
  const openApiPath = resolve(root, "contracts/generated/v1/openapi.json");
  const source = await readFile(asyncApiPath, "utf8");
  const document = parseYamlDocument(source, { uniqueKeys: true });
  assert.deepEqual(
    document.errors,
    [],
    `Integration AsyncAPI YAML errors:\n${document.errors.join("\n")}`,
  );
  const value = document.toJS();
  const result = await new Parser().parse(source, { source: asyncApiPath });
  const blocking = result.diagnostics.filter(({ severity }) => severity <= 1);
  assert.deepEqual(
    blocking,
    [],
    `Integration AsyncAPI diagnostics:\n${blocking
      .map(({ message, path }) => `${path?.join(".") ?? "$"}: ${message}`)
      .join("\n")}`,
  );
  assert.ok(result.document, "integration AsyncAPI parser returned no document");
  assert.equal(value.asyncapi, "3.1.0");

  const openapi = await readStrictJson(openApiPath);
  const expectedChannels = new Map([
    ["nuvioObservation", "/api/v1/integrations/nuvio/webhook"],
    ["tautulliObservation", "/api/v1/integrations/tautulli/webhook"],
    ["jellyfinObservation", "/api/v1/integrations/jellyfin/webhook"],
    ["embyObservation", "/api/v1/integrations/emby/webhook"],
    ["plexObservation", "/api/v1/integrations/plex/webhook"],
  ]);
  for (const [channel, address] of expectedChannels) {
    assert.equal(value.channels[channel]?.address, address, `${channel} address drifted`);
    assert.ok(openapi.paths[address]?.post, `${address} is missing from generated OpenAPI`);
  }
  assert.ok(
    openapi.paths["/api/v1/integrations"]?.get,
    "runtime integration status is missing from generated OpenAPI",
  );

  for (const [operationName, operation] of Object.entries(value.operations)) {
    assert.equal(operation.action, "receive", `${operationName} must be provider-to-Fasti receive`);
    assert.equal(operation.bindings?.http?.method, "POST", `${operationName} must use POST`);
    assert.equal(operation["x-fasti-capability-id"], "observation.accept");
    assert.deepEqual(operation["x-fasti-required-scopes"], ["observation_accept"]);
    assert.equal(operation["x-fasti-runtime-availability"], "implemented");
  }

  const asyncSchema = value.components.schemas.integrationObservationRequest;
  const openApiSchema = openapi.components.schemas.IntegrationObservationRequest;
  assert.ok(openApiSchema, "generated OpenAPI lacks IntegrationObservationRequest");
  assert.deepEqual(
    [...Object.keys(asyncSchema.properties)].sort(),
    [...Object.keys(openApiSchema.properties)].sort(),
    "AsyncAPI and OpenAPI integration request fields drifted",
  );
  assert.deepEqual(
    [...asyncSchema.required].sort(),
    [...openApiSchema.required].sort(),
    "AsyncAPI and OpenAPI integration required fields drifted",
  );

  assert.equal(
    value.components.messages.plexWebhook.contentType,
    "multipart/form-data",
    "Plex must remain a bounded multipart profile",
  );
  assert.match(
    value.operations.receivePlexObservation["x-fasti-auth-note"],
    /never placed in the webhook URL/u,
    "Plex contract must preserve the no-secret-in-URL rule",
  );

  return {
    asyncApiVersion: value.asyncapi,
    channelCount: expectedChannels.size,
  };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const requestedRoot = process.argv[2] ? resolve(process.argv[2]) : repositoryRoot;
  await validateIntegrationContracts(requestedRoot);
  console.log(
    "PASS: production integration AsyncAPI matches generated OpenAPI and the scoped observation boundary",
  );
}
