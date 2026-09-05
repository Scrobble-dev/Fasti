import { strict as assert } from "node:assert";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import SwaggerParser from "@apidevtools/swagger-parser";
import Ajv2020 from "ajv/dist/2020.js";

import { readStrictJson } from "./lib/strict-json.mjs";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

/**
 * Reads and parses a strict JSON file from a repository path.
 * @param {string} root - The repository root directory.
 * @param {string} relativePath - The path relative to the repository root.
 * @returns {Promise<*>} The parsed JSON content.
 */
const readJson = async (root, relativePath) =>
  readStrictJson(resolve(root, relativePath));

/**
 * Recursively visits all $ref references in a JSON Schema or OpenAPI document.
 * @param {*} value - The value to traverse (object, array, or primitive).
 * @param {Function} visit - Callback function invoked for each $ref value.
 */
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

const forbiddenAccessProperties = new Set([
  "access_token",
  "bootstrap_secret",
  "browser_binding",
  "browser_binding_digest",
  "code_verifier",
  "credential",
  "credential_digest",
  "csrf",
  "csrf_digest",
  "csrf_secret",
  "csrf_token",
  "id_token",
  "refresh_token",
  "session_digest",
  "session_secret",
  "token",
  "vendor_token",
]);

const validateAccessContractSecrets = (openapi) => {
  const seenReferences = new Set();
  const visit = (value) => {
    if (Array.isArray(value)) {
      value.forEach(visit);
      return;
    }
    if (value === null || typeof value !== "object") return;
    if (typeof value.$ref === "string") {
      const prefix = "#/components/schemas/";
      assert.ok(
        value.$ref.startsWith(prefix),
        "Access contract contains a non-local schema reference",
      );
      if (!seenReferences.has(value.$ref)) {
        seenReferences.add(value.$ref);
        const schema =
          openapi.components.schemas[value.$ref.slice(prefix.length)];
        assert.ok(schema, `Access schema reference ${value.$ref} is absent`);
        visit(schema);
      }
    }
    for (const property of Object.keys(value.properties ?? {})) {
      assert.ok(
        !forbiddenAccessProperties.has(property),
        `Access contract exposes forbidden secret property ${property}`,
      );
    }
    Object.values(value).forEach(visit);
  };

  for (const [path, pathItem] of Object.entries(openapi.paths)) {
    if (!path.startsWith("/api/access/v1/")) continue;
    for (const operation of Object.values(pathItem)) {
      for (const surface of [
        operation.parameters,
        operation.requestBody,
        operation.responses,
      ]) {
        if (surface) visit(surface);
      }
    }
  }
};

/**
 * Validates generated OpenAPI, JSON Schema, capability registry, and problem catalog contracts.
 * @param {string} [root=repositoryRoot] - The repository root directory containing the generated contracts.
 * @returns {Object} Validation counts for capabilities, OpenAPI paths, problems, and schemas.
 * @throws {AssertionError} If a generated contract fails validation.
 */
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
  validateAccessContractSecrets(openapi);
  assert.deepEqual(Object.keys(openapi.paths), [
    "/api/access/v1/browser-session",
    "/api/access/v1/browser-session/profile",
    "/api/access/v1/browser-session/rotation",
    "/api/access/v1/browser-sessions",
    "/api/access/v1/browser-sessions/others",
    "/api/access/v1/browser-sessions/{browser_session_id}",
    "/api/access/v1/projection",
    "/api/access/v1/trailbase/callback",
    "/api/access/v1/trailbase/continuation",
    "/api/access/v1/trailbase/sign-in",
    "/api/v1/client-enrollments",
    "/api/v1/health",
    "/api/v1/integrations",
    "/api/v1/integrations/emby/webhook",
    "/api/v1/integrations/jellyfin/webhook",
    "/api/v1/integrations/nuvio/webhook",
    "/api/v1/integrations/plex/webhook",
    "/api/v1/integrations/tautulli/webhook",
    "/api/v1/metadata/claims/refresh",
    "/api/v1/namespaces",
    "/api/v1/node/initialization",
    "/api/v1/observations",
    "/api/v1/profile/anime-grouping-policy",
    "/api/v1/profile/anime-grouping-policy/preview",
    "/api/v1/profile/metadata-projection",
    "/api/v1/profile/nuvio-collections",
    "/api/v1/profile/record-tracking-dispositions",
    "/api/v1/profile/record-tracking-dispositions/{record_id}",
    "/api/v1/providers",
    "/api/v1/providers/{provider_id}/credentials/{capability_id}",
    "/api/v1/providers/{provider_id}/credentials/{capability_id}/tests",
    "/api/v1/providers/{provider_id}/health",
    "/api/v1/records",
    "/api/v1/records/identifiers",
    "/api/v1/records/{record_id}/identity-route",
    "/api/v1/records/{record_id}/metadata-projection",
  ]);
  assert.deepEqual(Object.keys(openapi.components.securitySchemes), [
    "auth_binding_cookie",
    "auth_continuation_cookie",
    "bootstrap_bearer",
    "browser_session_cookie",
    "credential_bearer",
    "csrf_cookie",
    "csrf_header",
  ]);
  assert.deepEqual(
    openapi.paths["/api/v1/records"].get.security,
    [{ credential_bearer: [] }, { browser_session_cookie: [] }],
    "list_records security must match hybrid authorization",
  );
  const nuvioCollections = openapi.paths["/api/v1/profile/nuvio-collections"];
  const hybridReadSecurity = [
    { credential_bearer: [] },
    { browser_session_cookie: [] },
  ];
  const hybridMutationSecurity = [
    { credential_bearer: [] },
    {
      browser_session_cookie: [],
      csrf_cookie: [],
      csrf_header: [],
    },
  ];
  assert.deepEqual(nuvioCollections.delete.security, hybridMutationSecurity);
  assert.deepEqual(nuvioCollections.get.security, hybridReadSecurity);
  assert.deepEqual(nuvioCollections.put.security, hybridMutationSecurity);

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
  assert.equal(registry.capabilities.length, 52);
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
    if (capability.id === "integration.status") return "b1_integration_status";
    if (["client.enroll", "node.initialize"].includes(capability.id)) {
      return "b1_durable_bootstrap";
    }
    if (capability.id === "observation.accept") return "b1_observation_accept";
    if (capability.id === "receipt.replay") return "b1_receipt_replay";
    if (capability.id === "receipt.stream") return "b1_receipt_stream";
    if (capability.id === "access.identity.bootstrap") {
      return "c1_identity_bootstrap";
    }
    if (capability.id === "access.projection.read") {
      return "c1_access_projection";
    }
    if (capability.id.startsWith("browser.")) {
      return "c1_browser_session_foundation";
    }
    if (capability.id.startsWith("provider.")) return "m1_providers";
    if (capability.id.startsWith("metadata.")) return "m2_metadata";
    if (
      capability.id === "identity.route.resolve" ||
      capability.id.startsWith("profile.anime_grouping_policy.")
    ) {
      return "m3_identity_routing";
    }
    if (
      capability.id.startsWith("profile.record.tracking_disposition.") ||
      capability.id.startsWith("profile.nuvio_collections.")
    ) {
      return "b2_profile_state";
    }
    if (
      [
        "identity.record.create",
        "identity.identifier.attach",
        "identity.record.list",
        "identity.namespace.register",
      ].includes(capability.id)
    ) {
      return "b1_records";
    }
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
    assert.equal(
      operation["x-fasti-authorization"],
      operation.operationId === "accept_observation"
        ? "scoped"
        : capability.authorization,
    );
    assert.deepEqual(operation["x-fasti-required-scopes"], capability.scopes);
    assert.equal(operation["x-fasti-runtime-availability"], "fixture_only");
    assert.ok(
      operation["x-fasti-problem-codes"].every((code) =>
        capability.problems.includes(code),
      ),
    );
    assert.deepEqual(operation["x-fasti-example-ids"], capability.examples);
    for (const code of operation["x-fasti-problem-codes"]) {
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
        capability.contract_body === "b1" &&
        capability.lifecycle.contract_state === "finalized" &&
        ![
          "receipt.stream",
          "system.health",
          "integration.status",
          "identity.record.create",
          "identity.identifier.attach",
          "identity.record.list",
          "identity.namespace.register",
        ].includes(capability.id),
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
  assert.deepEqual(Object.keys(openapi.components.securitySchemes).sort(), [
    "auth_binding_cookie",
    "auth_continuation_cookie",
    "bootstrap_bearer",
    "browser_session_cookie",
    "credential_bearer",
    "csrf_cookie",
    "csrf_header",
  ]);
  for (const scheme of [
    openapi.components.securitySchemes.bootstrap_bearer,
    openapi.components.securitySchemes.credential_bearer,
  ]) {
    assert.equal(scheme.type, "http");
    assert.equal(scheme.scheme, "bearer");
  }
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
  assert.equal(healthOperation.security, undefined);

  const browserReads = new Set([
    "read_access_projection",
    "read_browser_session",
    "list_browser_sessions",
  ]);
  const browserMutations = new Set([
    "end_browser_session",
    "revoke_browser_session",
    "revoke_other_browser_sessions",
    "revoke_all_browser_sessions",
    "rotate_browser_session",
    "select_browser_session_profile",
  ]);
  const hybridOperations = new Set([
    "submit_observation",
    "create_record",
    "attach_identifier",
    "list_records",
    "register_namespace",
    "list_tracking_dispositions",
    "set_tracking_disposition",
    "get_nuvio_collections",
    "replace_nuvio_collections",
    "clear_nuvio_collections",
    "resolve_identity_route",
    "read_anime_grouping_policy",
    "preview_anime_grouping_policy_change",
    "apply_anime_grouping_policy_change",
  ]);
  const hybridMutations = new Set([
    "submit_observation",
    "create_record",
    "attach_identifier",
    "register_namespace",
    "set_tracking_disposition",
    "replace_nuvio_collections",
    "clear_nuvio_collections",
    "apply_anime_grouping_policy_change",
  ]);

  for (const pathItem of Object.values(openapi.paths)) {
    for (const operation of Object.values(pathItem)) {
      const capability = capabilities.get(operation["x-fasti-capability-id"]);
      assert.ok(
        capability,
        `production operation ${operation.operationId} has no capability`,
      );
      let security;
      if (operation.operationId === "initialize_node") {
        security = [{ bootstrap_bearer: [] }];
      } else if (
        [
          "health_check",
          "enroll_first_client",
          "integration_status",
          "start_trailbase_sign_in",
        ].includes(operation.operationId)
      ) {
        security = undefined;
      } else if (
        operation.operationId === "complete_trailbase_authentication"
      ) {
        security = [{ auth_binding_cookie: [] }];
      } else if (
        [
          "read_trailbase_continuation",
          "complete_trailbase_continuation",
          "cancel_trailbase_continuation",
        ].includes(operation.operationId)
      ) {
        security = [{ auth_continuation_cookie: [] }];
      } else if (browserReads.has(operation.operationId)) {
        security = [{ browser_session_cookie: [] }];
      } else if (browserMutations.has(operation.operationId)) {
        security = [
          {
            browser_session_cookie: [],
            csrf_cookie: [],
            csrf_header: [],
          },
        ];
      } else if (hybridMutations.has(operation.operationId)) {
        security = hybridMutationSecurity;
      } else if (hybridOperations.has(operation.operationId)) {
        security = hybridReadSecurity;
      } else {
        security = [{ credential_bearer: [] }];
      }
      assert.deepEqual(
        operation.security,
        security,
        `production operation ${operation.operationId} security must match ${capability.authorization} authorization`,
      );
    }
  }
  const accessProblems = {
    start_trailbase_sign_in: [
      "capacity_exceeded",
      "forbidden",
      "integrity_failed",
      "malformed_json",
      "payload_too_large",
      "storage_unavailable",
      "trailbase_trust_unavailable",
      "unsupported_media_type",
      "validation_failed",
    ],
    complete_trailbase_authentication: [],
    read_trailbase_continuation: [
      "auth_browser_binding_invalid",
      "auth_continuation_persistence_failed",
      "auth_subject_unaffiliated",
      "capacity_exceeded",
      "forbidden",
      "identity_service_unavailable",
      "integrity_failed",
      "storage_unavailable",
      "trailbase_proof_invalid",
      "trailbase_session_cleanup_failed",
      "trailbase_trust_unavailable",
      "validation_failed",
    ],
    complete_trailbase_continuation: [
      "auth_browser_binding_invalid",
      "auth_continuation_persistence_failed",
      "auth_selection_changed",
      "auth_subject_unaffiliated",
      "capacity_exceeded",
      "forbidden",
      "identity_service_unavailable",
      "integrity_failed",
      "malformed_json",
      "payload_too_large",
      "storage_unavailable",
      "trailbase_proof_invalid",
      "trailbase_session_cleanup_failed",
      "trailbase_trust_unavailable",
      "unsupported_media_type",
      "validation_failed",
    ],
    cancel_trailbase_continuation: [
      "auth_browser_binding_invalid",
      "forbidden",
      "integrity_failed",
      "storage_unavailable",
      "trailbase_proof_invalid",
      "validation_failed",
    ],
    read_access_projection: [
      "browser_session_expired",
      "browser_session_revoked",
      "integrity_failed",
      "session_policy_changed",
      "storage_unavailable",
    ],
    read_browser_session: [
      "browser_session_expired",
      "browser_session_revoked",
      "integrity_failed",
      "session_policy_changed",
      "storage_unavailable",
    ],
    list_browser_sessions: [
      "browser_session_expired",
      "browser_session_revoked",
      "integrity_failed",
      "session_policy_changed",
      "storage_unavailable",
    ],
    end_browser_session: [
      "browser_session_expired",
      "browser_session_revoked",
      "forbidden",
      "integrity_failed",
      "session_policy_changed",
      "storage_unavailable",
    ],
    revoke_browser_session: [
      "browser_session_expired",
      "browser_session_revoked",
      "forbidden",
      "integrity_failed",
      "session_policy_changed",
      "storage_unavailable",
      "validation_failed",
    ],
    revoke_other_browser_sessions: [
      "browser_session_expired",
      "browser_session_revoked",
      "forbidden",
      "integrity_failed",
      "session_policy_changed",
      "storage_unavailable",
    ],
    revoke_all_browser_sessions: [
      "browser_session_expired",
      "browser_session_revoked",
      "forbidden",
      "integrity_failed",
      "session_policy_changed",
      "storage_unavailable",
    ],
    rotate_browser_session: [
      "browser_session_expired",
      "browser_session_revoked",
      "forbidden",
      "integrity_failed",
      "session_policy_changed",
      "storage_unavailable",
    ],
    select_browser_session_profile: [
      "browser_session_expired",
      "browser_session_revoked",
      "forbidden",
      "integrity_failed",
      "malformed_json",
      "payload_too_large",
      "session_policy_changed",
      "storage_unavailable",
      "unsupported_media_type",
      "validation_failed",
    ],
  };
  for (const pathItem of Object.values(openapi.paths)) {
    for (const operation of Object.values(pathItem)) {
      if (Object.hasOwn(accessProblems, operation.operationId)) {
        assert.deepEqual(
          operation["x-fasti-problem-codes"],
          accessProblems[operation.operationId],
          `${operation.operationId} problem subset drifted`,
        );
      }
    }
  }

  const httpOperations = new Map(
    operations.map(({ operation }) => [
      operation["x-fasti-capability-id"],
      operation,
    ]),
  );
  for (const [path, pathItem] of Object.entries(openapi.paths)) {
    for (const operation of Object.values(pathItem)) {
      const capabilityId = operation["x-fasti-capability-id"];
      if (capabilityId !== "system.health") {
        const capability = capabilities.get(capabilityId);
        assert.ok(capability, `production operation ${path} has no capability`);
        assert.equal(
          operation["x-fasti-runtime-availability"],
          capability.lifecycle.runtime_availability,
        );
        assert.ok(
          operation["x-fasti-problem-codes"].every((code) =>
            capability.problems.includes(code),
          ),
          `${operation.operationId} claims a problem outside ${capabilityId}`,
        );
        httpOperations.set(capabilityId, operation);
      }
    }
  }
  httpOperations.set("system.health", healthOperation);
  for (const capability of registry.capabilities.filter(
    ({ lifecycle }) => lifecycle.contract_state === "finalized",
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
          if (capability.id === "access.identity.bootstrap") {
            assert.equal(binding, "cli:access-identity-bootstrap");
          } else {
            assert.equal(binding, "cli:capability-discovery");
          }
          break;
        case "json_schema":
          if (binding === "schema:health-response") {
            assert.equal(capability.id, "system.health");
          } else if (
            binding.startsWith("schema:openapi-operation:") ||
            binding.startsWith("schema:production-openapi-operation:")
          ) {
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
            capability.id === "access.identity.bootstrap"
              ? "package-smoke:c1-operator-bootstrap"
              : capability.id === "system.health"
                ? "package-smoke:production-health"
                : capability.id.startsWith("provider.")
                  ? "package-smoke:production-providers"
                  : capability.id.startsWith("metadata.")
                    ? "package-smoke:production-metadata"
                    : capability.surface_profile === "m3_identity_routing"
                      ? "package-smoke:production-identity-routing"
                      : ["client.enroll", "node.initialize"].includes(
                            capability.id,
                          )
                        ? "package-smoke:production-bootstrap"
                        : "package-smoke:b1-conformance-fixture",
          );
          break;
        case "ui":
          assert.equal(
            binding,
            capability.id === "access.projection.read"
              ? "ui:account-security"
              : capability.id.startsWith("provider.")
                ? "ui:provider-settings"
                : capability.id.startsWith("metadata.")
                  ? "ui:metadata-provenance"
                  : capability.surface_profile === "m3_identity_routing"
                    ? "ui:anime-grouping-policy"
                    : `ui:${capability.id}`,
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
