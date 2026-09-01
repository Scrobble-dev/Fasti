import assert from "node:assert/strict";
import { execFile, spawn } from "node:child_process";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { test } from "node:test";

import {
  FastiAbortError,
  FastiClient,
  FastiContractParseError,
  FastiProblemError,
  FastiProtocolError,
  FastiTimeoutError,
  FastiTransportError,
  connectionEndpoint,
  normalizeBaseUrl,
  parseAcceptObservationRequest,
  parseHealthResponse,
  parseListRecordsResponse,
  parseConfigureMetadataProjectionRequest,
  parseRefreshMetadataClaimsRequest,
  parseReceiptCommittedEvent,
  PUBLIC_CAPABILITY_REGISTRY,
  RECEIPT_STREAM_CONTRACT,
} from "../../packages/sdk/dist/transport.js";

const execFileAsync = promisify(execFile);
const repositoryRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../..",
);

const ids = {
  correlation: v7("req", "1"),
  observation: v7("obs", "2"),
  operation: v7("op", "3"),
  receiptA: v7("rcp", "4"),
  receiptB: v7("rcp", "5"),
};

const contractIds = {
  operation: v7("op", "1"),
  receipt: v7("rcp", "2"),
  workspace: v7("wsp", "3"),
  profile: v7("prf", "4"),
  client: v7("cli", "5"),
  observation: v7("obs", "6"),
  evidence: v7("evd", "7"),
  record: v7("rec", "8"),
  browserSession: v7("ses", "9"),
};

test("metadata refresh parser enforces canonical operation IDs", () => {
  const request = {
    operation_id: contractIds.operation,
    record_id: contractIds.record,
    provider_id: "tmdb",
    field_groups: ["basic_info"],
    locale: "en-ie",
    region: "IE",
    mode: "prefer_cache",
  };
  assert.deepEqual(parseRefreshMetadataClaimsRequest(request), request);
  assert.throws(
    () =>
      parseRefreshMetadataClaimsRequest({
        ...request,
        operation_id: "x".repeat(35),
      }),
    FastiContractParseError,
  );
});

test("identity routing SDK binds queries, policy bodies, and response identity", async () => {
  const policy = {
    profile_id: contractIds.profile,
    scope: { kind: "profile", client_id: null },
    source: "profile_default",
    preference: "automatic",
    revision: 0,
  };
  const route = {
    record_id: contractIds.record,
    intent: "metadata_enrichment",
    target_provider: "tmdb",
    status: "missing",
    known_identifiers: [],
    candidate_routes: [],
    selected_route: null,
    nuvio_content_id: null,
  };
  const previewRequest = {
    scope: policy.scope,
    change: { kind: "set", preference: "group_by_tv_work" },
    after_record_id: null,
    limit: 10,
  };
  const preview = {
    policy,
    proposed_preference: "group_by_tv_work",
    proposed_source: "profile_default",
    total_records: 0,
    affected_records: 0,
    unresolved_routes: 0,
    possible_season_regroupings: 0,
    records: [],
    next_after_record_id: null,
  };
  const applyRequest = {
    operation_id: contractIds.operation,
    scope: policy.scope,
    expected_revision: 0,
    change: previewRequest.change,
  };
  const applied = {
    operation_id: contractIds.operation,
    change: applyRequest.change,
    previous_preference: "automatic",
    previous_source: "profile_default",
    policy: { ...policy, preference: "group_by_tv_work", revision: 1 },
    affected_records: 0,
    unresolved_routes: 0,
    possible_season_regroupings: 0,
    rolled_back_operation_id: null,
  };
  const requests = [];
  const client = new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    credential: "credential",
    fetch: async (url, init) => {
      requests.push({ url: String(url), init });
      const path = new URL(String(url)).pathname;
      const response = path.endsWith("/identity-route")
        ? route
        : path.endsWith("/preview")
          ? preview
          : init.method === "PUT"
            ? applied
            : { policy };
      return new Response(JSON.stringify(response), {
        headers: { "content-type": "application/json" },
      });
    },
  });

  assert.deepEqual(
    await client.resolveIdentityRoute(contractIds.record, {
      intent: "metadata_enrichment",
      target_provider: "tmdb",
    }),
    route,
  );
  assert.deepEqual(await client.readAnimeGroupingPolicy({ scope: "profile" }), {
    policy,
  });
  assert.deepEqual(
    await client.previewAnimeGroupingPolicyChange(previewRequest),
    preview,
  );
  assert.deepEqual(
    await client.applyAnimeGroupingPolicyChange(applyRequest),
    applied,
  );
  assert.deepEqual(
    requests.map(({ url, init }) => ({
      url,
      method: init.method,
      authorization: new Headers(init.headers).get("authorization"),
      body: init.body === undefined ? undefined : JSON.parse(init.body),
    })),
    [
      {
        url: `http://127.0.0.1:8420/api/v1/records/${contractIds.record}/identity-route?intent=metadata_enrichment&target_provider=tmdb`,
        method: "GET",
        authorization: "Bearer credential",
        body: undefined,
      },
      {
        url: "http://127.0.0.1:8420/api/v1/profile/anime-grouping-policy?scope=profile",
        method: "GET",
        authorization: "Bearer credential",
        body: undefined,
      },
      {
        url: "http://127.0.0.1:8420/api/v1/profile/anime-grouping-policy/preview",
        method: "POST",
        authorization: "Bearer credential",
        body: previewRequest,
      },
      {
        url: "http://127.0.0.1:8420/api/v1/profile/anime-grouping-policy",
        method: "PUT",
        authorization: "Bearer credential",
        body: applyRequest,
      },
    ],
  );
  assert.throws(
    () =>
      client.resolveIdentityRoute(contractIds.record, {
        intent: "metadata_enrichment",
        target_provider: "TMDB with spaces",
      }),
    TypeError,
  );

  const mismatched = new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    credential: "credential",
    fetch: async () =>
      new Response(JSON.stringify({ ...route, record_id: v7("rec", "f") }), {
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    mismatched.resolveIdentityRoute(contractIds.record, {
      intent: "metadata_enrichment",
      target_provider: "tmdb",
    }),
    FastiProtocolError,
  );
});

test("browser authentication SDK exposes callable operations but never the callback", () => {
  const client = new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
  });
  for (const method of [
    "startTrailBaseSignIn",
    "readTrailBaseContinuation",
    "completeTrailBaseContinuation",
    "cancelTrailBaseContinuation",
    "readAccessProjection",
    "readBrowserSession",
    "endBrowserSession",
    "listBrowserSessions",
    "revokeBrowserSession",
    "revokeOtherBrowserSessions",
    "revokeAllBrowserSessions",
    "rotateBrowserSession",
    "selectBrowserSessionProfile",
  ]) {
    assert.equal(typeof client[method], "function", method);
  }
  assert.equal(client.completeTrailBaseAuthentication, undefined);
});

test("TrailBase continuation SDK keeps binding authority in same-origin cookies", async () => {
  const revision = `sha256:${"a".repeat(64)}`;
  const projection = {
    expires_at: "2026-08-31T12:00:00Z",
    remembered: true,
    candidate_revision: revision,
    choices: [
      {
        choice_ordinal: 0,
        workspace_ordinal: 1,
        profile_ordinal: 1,
        workspace_created_at: "2026-08-30T10:00:00Z",
        profile_created_at: "2026-08-30T10:01:00Z",
        membership_state: "active",
        role: "member",
      },
    ],
  };
  const requests = [];
  const client = new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    credential: "must-not-be-sent",
    retryPolicy: { maxAttempts: 3 },
    fetch: async (url, init) => {
      requests.push({ url: String(url), init });
      return init?.method === "GET"
        ? new Response(JSON.stringify(projection), {
            headers: { "content-type": "application/json" },
          })
        : new Response(null, { status: 204 });
    },
  });

  assert.deepEqual(await client.readTrailBaseContinuation(), projection);
  await client.completeTrailBaseContinuation({
    choice_ordinal: 0,
    candidate_revision: revision,
  });
  await client.cancelTrailBaseContinuation();

  assert.deepEqual(
    requests.map(({ url, init }) => ({
      url,
      method: init.method,
      body: init.body,
      credentials: init.credentials,
      authorization: new Headers(init.headers).get("authorization"),
      csrf: new Headers(init.headers).get("x-csrf-token"),
    })),
    [
      {
        url: "http://127.0.0.1:8420/api/access/v1/trailbase/continuation",
        method: "GET",
        body: undefined,
        credentials: "same-origin",
        authorization: null,
        csrf: null,
      },
      {
        url: "http://127.0.0.1:8420/api/access/v1/trailbase/continuation",
        method: "POST",
        body: JSON.stringify({
          choice_ordinal: 0,
          candidate_revision: revision,
        }),
        credentials: "same-origin",
        authorization: null,
        csrf: null,
      },
      {
        url: "http://127.0.0.1:8420/api/access/v1/trailbase/continuation",
        method: "DELETE",
        body: undefined,
        credentials: "same-origin",
        authorization: null,
        csrf: null,
      },
    ],
  );
  assert.throws(
    () =>
      client.completeTrailBaseContinuation({
        choice_ordinal: 0,
        candidate_revision: revision,
        workspace_id: contractIds.workspace,
      }),
    FastiProtocolError,
  );
  assert.equal(requests.length, 3);

  const leakingClient = new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    fetch: async () =>
      new Response(
        JSON.stringify({
          ...projection,
          choices: [
            {
              ...projection.choices[0],
              workspace_id: contractIds.workspace,
            },
          ],
        }),
        { headers: { "content-type": "application/json" } },
      ),
  });
  await assert.rejects(
    leakingClient.readTrailBaseContinuation(),
    FastiProtocolError,
  );
});

test("TrailBase continuation mutations make one attempt on network failure", async () => {
  const revision = `sha256:${"a".repeat(64)}`;
  for (const [label, invoke] of [
    [
      "complete",
      (client) =>
        client.completeTrailBaseContinuation({
          choice_ordinal: 0,
          candidate_revision: revision,
        }),
    ],
    ["cancel", (client) => client.cancelTrailBaseContinuation()],
  ]) {
    let attempts = 0;
    const client = new FastiClient({
      baseUrl: "http://127.0.0.1:8420",
      retryPolicy: { maxAttempts: 3, baseDelayMs: 0, maxDelayMs: 0 },
      fetch: async () => {
        attempts += 1;
        throw new Error("network unavailable");
      },
    });
    await assert.rejects(invoke(client), FastiTransportError, label);
    assert.equal(attempts, 1, label);
  }
});

test("TrailBase continuation SDK accepts governed continuation evidence problems", async () => {
  const catalog = JSON.parse(
    await readFile(
      path.join(repositoryRoot, "contracts/generated/v1/problems.json"),
      "utf8",
    ),
  );
  const revision = `sha256:${"a".repeat(64)}`;
  for (const code of [
    "auth_continuation_persistence_failed",
    "auth_subject_unaffiliated",
    "identity_service_unavailable",
    "storage_unavailable",
    "trailbase_session_cleanup_failed",
    "trailbase_trust_unavailable",
  ]) {
    const { param_policy: _paramPolicy, ...definition } = catalog.problems.find(
      (problem) =>
        problem.capability_id === "browser.session.create" &&
        problem.code === code,
    );
    const problem = {
      ...definition,
      actual: null,
      correlation_id: ids.correlation,
      violations: [],
    };
    for (const [label, invoke] of [
      ["read", (client) => client.readTrailBaseContinuation()],
      [
        "complete",
        (client) =>
          client.completeTrailBaseContinuation({
            choice_ordinal: 0,
            candidate_revision: revision,
          }),
      ],
    ]) {
      let attempts = 0;
      const client = new FastiClient({
        baseUrl: "http://127.0.0.1:8420",
        retryPolicy: { maxAttempts: 3, baseDelayMs: 0, maxDelayMs: 0 },
        fetch: async () => {
          attempts += 1;
          return new Response(JSON.stringify(problem), {
            status: problem.status,
            headers: { "content-type": "application/problem+json" },
          });
        },
      });
      await assert.rejects(invoke(client), (error) => {
        assert.ok(error instanceof FastiProblemError, `${label}: ${code}`);
        assert.equal(error.problem.code, code, label);
        return true;
      });
      assert.equal(
        attempts,
        label === "read" && problem.retryability === "retry_safe" ? 3 : 1,
        `${label}: ${code}`,
      );
    }
  }
});

test("browser mutations copy the exact CSRF cookie and omit bearer credentials", async () => {
  const csrf = "a".repeat(64);
  const original = Object.getOwnPropertyDescriptor(globalThis, "document");
  Object.defineProperty(globalThis, "document", {
    configurable: true,
    value: { cookie: `unrelated=1; __Host-fasti_csrf=${csrf}` },
  });
  try {
    let request;
    const client = new FastiClient({
      baseUrl: "http://127.0.0.1:8420",
      credential: "must-not-be-sent",
      fetch: async (url, init) => {
        request = { url: String(url), init };
        return new Response(JSON.stringify({ revoked_count: 1 }), {
          headers: { "content-type": "application/json" },
        });
      },
    });
    assert.deepEqual(
      await client.revokeBrowserSession(contractIds.browserSession),
      { revoked_count: 1 },
    );
    const headers = new Headers(request.init.headers);
    assert.equal(
      request.url,
      `http://127.0.0.1:8420/api/access/v1/browser-sessions/${contractIds.browserSession}`,
    );
    assert.equal(request.init.credentials, "same-origin");
    assert.equal(headers.get("x-csrf-token"), csrf);
    assert.equal(headers.get("authorization"), null);
  } finally {
    if (original) Object.defineProperty(globalThis, "document", original);
    else delete globalThis.document;
  }
});

test("browser mutations fail locally without one valid CSRF cookie", async () => {
  let called = false;
  const client = new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    fetch: async () => {
      called = true;
      throw new Error("must not call fetch");
    },
  });
  await assert.rejects(client.endBrowserSession(), FastiProtocolError);
  assert.equal(called, false);
});

test("health omits credentials and returns the exact public contract", async () => {
  const credential = "local-secret-that-must-not-leak";
  await withServer(
    (request, response) => {
      assert.equal(request.url, "/api/v1/health");
      assert.equal(request.headers.authorization, undefined);
      json(response, 200, { status: "healthy", version: "0.1.0" });
    },
    async (baseUrl) => {
      const client = new FastiClient({ baseUrl: `${baseUrl}/`, credential });
      assert.deepEqual(await client.health(), {
        status: "healthy",
        version: "0.1.0",
      });
    },
  );
});

test("durable bootstrap SDK keeps one-time secrets in JSON bodies", async () => {
  const proof = "a".repeat(64);
  const credential = "b".repeat(64);
  const requests = [];
  const client = new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    fetch: async (url, init) => {
      requests.push({
        url: String(url),
        authorization: new Headers(init?.headers).get("authorization"),
        body: JSON.parse(init?.body),
      });
      if (String(url).endsWith("/api/v1/node/initialization")) {
        return new Response(JSON.stringify({ initialization_proof: proof }), {
          headers: { "content-type": "application/json" },
        });
      }
      return new Response(
        JSON.stringify({ credential_scheme: "Bearer", credential }),
        { headers: { "content-type": "application/json" } },
      );
    },
  });

  const initialized = await client.initializeDurableNode();
  assert.equal(initialized.initialization_proof, proof);
  const enrolled = await client.enrollDurableFirstClient({
    initialization_proof: initialized.initialization_proof,
  });
  assert.equal(enrolled.credential, credential);

  assert.deepEqual(requests, [
    {
      url: "http://127.0.0.1:8420/api/v1/node/initialization",
      authorization: null,
      body: {},
    },
    {
      url: "http://127.0.0.1:8420/api/v1/client-enrollments",
      authorization: null,
      body: { initialization_proof: proof },
    },
  ]);
});

test("default fetch keeps the platform receiver", async () => {
  const originalFetch = globalThis.fetch;
  let receiver;
  globalThis.fetch = function () {
    receiver = this;
    return Promise.resolve(
      new Response(JSON.stringify({ status: "healthy", version: "0.1.0" }), {
        headers: { "content-type": "application/json" },
      }),
    );
  };
  try {
    const client = new FastiClient({ baseUrl: "http://127.0.0.1:8420" });
    assert.equal((await client.health()).status, "healthy");
    assert.equal(receiver, globalThis);
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("media types are matched case-insensitively", async () => {
  const client = new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    fetch: async () =>
      new Response(JSON.stringify({ status: "healthy", version: "0.1.0" }), {
        headers: { "content-type": "Application/JSON; Charset=UTF-8" },
      }),
  });
  assert.equal((await client.health()).status, "healthy");
});

test("RFC 9457 responses become typed Fasti problem errors", async () => {
  const problem = {
    type: "https://fasti.scrobble.dev/v1/problems/forbidden",
    title: "Forbidden",
    status: 403,
    detail: "request is not authorized for this capability",
    code: "forbidden",
    capability_id: "system.capabilities.discover",
    safe_state: "no_mutation",
    retryability: "not_retryable",
    next_actions: [
      {
        id: "verify_request_authorization",
        label: "Verify the request context and local grant",
      },
    ],
    correlation_id: ids.correlation,
    param: null,
    actual: null,
    violations: [],
  };
  await withServer(
    (_request, response) => {
      response.writeHead(403, { "content-type": "application/problem+json" });
      response.end(JSON.stringify(problem));
    },
    async (baseUrl) => {
      const client = new FastiClient({ baseUrl });
      await assert.rejects(client.discoverCapabilities(), (error) => {
        assert.ok(error instanceof FastiProblemError);
        assert.deepEqual(error.problem, problem);
        return true;
      });
    },
  );
});

test("problem parsing is operation-bound and canonical", async () => {
  const canonical = {
    type: "https://fasti.scrobble.dev/v1/problems/forbidden",
    title: "Forbidden",
    status: 403,
    detail: "request is not authorized for this capability",
    code: "forbidden",
    capability_id: "system.capabilities.discover",
    safe_state: "no_mutation",
    retryability: "not_retryable",
    next_actions: [
      {
        id: "verify_request_authorization",
        label: "Verify the request context and local grant",
      },
    ],
    correlation_id: ids.correlation,
    param: null,
    actual: null,
    violations: [],
  };
  const mutations = [
    (problem) =>
      (problem.type = "https://fasti.scrobble.dev/v1/problems/wrong"),
    (problem) => (problem.title = "Denied"),
    (problem) => (problem.status = 404),
    (problem) => (problem.detail = "different detail"),
    (problem) => (problem.code = "validation_failed"),
    (problem) => (problem.capability_id = "profile.select"),
    (problem) => (problem.safe_state = "prior_state_retained"),
    (problem) => (problem.retryability = "retry_safe"),
    (problem) => (problem.next_actions[0].id = "authenticate"),
    (problem) => (problem.next_actions = []),
    (problem) => problem.next_actions.push(problem.next_actions[0]),
    (problem) => (problem.param = "/credential"),
    (problem) => (problem.actual = "echoed-secret"),
    (problem) => (problem.correlation_id = `req_${"1".repeat(32)}`),
    (problem) =>
      problem.violations.push({
        code: "invalid_value",
        pointer: "/field",
        reason: "field is invalid",
        expected: "valid field",
        actual: "echoed-value",
      }),
    (problem) =>
      (problem.violations = Array.from({ length: 33 }, () => ({
        code: "invalid_value",
        pointer: "/field",
        reason: "field is invalid",
        expected: "valid field",
        actual: null,
      }))),
  ];
  for (const mutate of mutations) {
    const problem = structuredClone(canonical);
    mutate(problem);
    const client = new FastiClient({
      baseUrl: "http://127.0.0.1:8420",
      fetch: async () =>
        new Response(JSON.stringify(problem), {
          status: 403,
          headers: { "content-type": "application/problem+json" },
        }),
    });
    await assert.rejects(client.discoverCapabilities(), FastiProtocolError);
  }
});

test("health honors caller cancellation and its declared timeout", async (context) => {
  await context.test("caller cancellation", async () => {
    await withServer(
      () => {},
      async (baseUrl) => {
        const controller = new AbortController();
        const client = new FastiClient({ baseUrl, timeoutMs: 2_000 });
        const request = client.health({ signal: controller.signal });
        controller.abort();
        await assert.rejects(request, FastiAbortError);
      },
    );
  });

  await context.test("timeout", async () => {
    await withServer(
      () => {},
      async (baseUrl) => {
        const client = new FastiClient({ baseUrl, timeoutMs: 20 });
        await assert.rejects(client.health(), FastiTimeoutError);
      },
    );
  });
});

test("transient health retries are bounded by the declared policy", async () => {
  assert.throws(
    () =>
      new FastiClient({
        baseUrl: "http://127.0.0.1:8420",
        retryPolicy: { maxAttempts: 11 },
      }),
    /must not exceed 10/,
  );

  let attempts = 0;
  await withServer(
    (_request, response) => {
      attempts += 1;
      if (attempts < 3) {
        response.writeHead(503);
        response.end();
        return;
      }
      json(response, 200, { status: "healthy", version: "0.1.0" });
    },
    async (baseUrl) => {
      const client = new FastiClient({
        baseUrl,
        retryPolicy: {
          maxAttempts: 3,
          baseDelayMs: 0,
          maxDelayMs: 0,
        },
      });
      assert.equal((await client.health()).status, "healthy");
      assert.equal(attempts, 3);
    },
  );

  attempts = 0;
  await withServer(
    (_request, response) => {
      attempts += 1;
      response.writeHead(503);
      response.end();
    },
    async (baseUrl) => {
      const client = new FastiClient({
        baseUrl,
        retryPolicy: {
          maxAttempts: 2,
          baseDelayMs: 0,
          maxDelayMs: 0,
        },
      });
      await assert.rejects(client.health(), (error) => {
        assert.ok(error instanceof FastiTransportError);
        assert.equal(error.status, 503);
        return true;
      });
      assert.equal(attempts, 2);
    },
  );
});

test("Retry-After HTTP dates apply the bounded server delay", async () => {
  let attempts = 0;
  const startedAt = Date.now();
  const client = new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    retryPolicy: { maxAttempts: 2, baseDelayMs: 0, maxDelayMs: 40 },
    fetch: async () => {
      attempts += 1;
      if (attempts === 1) {
        return new Response(null, {
          status: 503,
          headers: {
            "retry-after": new Date(Date.now() + 60_000).toUTCString(),
          },
        });
      }
      return new Response(
        JSON.stringify({ status: "healthy", version: "0.1.0" }),
        { headers: { "content-type": "application/json" } },
      );
    },
  });
  assert.equal((await client.health()).status, "healthy");
  assert.equal(attempts, 2);
  assert.ok(Date.now() - startedAt >= 30);
});

test("receipt SSE treats clean finite fixture EOF as successful completion", async () => {
  let connections = 0;
  const credential = "receipt-reader-secret";
  await withServer(
    (request, response) => {
      assert.equal(request.url, RECEIPT_STREAM_CONTRACT.path);
      assert.equal(request.headers.accept, "text/event-stream");
      assert.equal(request.headers.authorization, `Bearer ${credential}`);
      connections += 1;
      response.writeHead(200, { "content-type": "text/event-stream" });
      assert.equal(request.headers["last-event-id"], undefined);
      response.end(sse(ids.receiptA, receipt(ids.receiptA)));
    },
    async (baseUrl) => {
      const client = new FastiClient({ baseUrl, credential });
      const events = client.receiptEvents({
        retryPolicy: {
          maxAttempts: 2,
          baseDelayMs: 0,
          maxDelayMs: 0,
        },
      });
      const first = await events.next();
      const second = await events.next();
      assert.equal(first.done, false);
      assert.equal(first.value.id, ids.receiptA);
      assert.deepEqual(first.value.data, receipt(ids.receiptA));
      assert.equal(second.done, true);
      assert.equal(second.value, undefined);
      assert.equal(connections, 1);
    },
  );
});

test("receipt SSE refuses malformed events instead of widening the contract", async () => {
  await withServer(
    (_request, response) => {
      response.writeHead(200, { "content-type": "text/event-stream" });
      response.end(
        sse(ids.receiptA, { ...receipt(ids.receiptA), unexpected: true }),
      );
    },
    async (baseUrl) => {
      const client = new FastiClient({ baseUrl });
      const events = client.receiptEvents({
        retryPolicy: {
          maxAttempts: 1,
          baseDelayMs: 0,
          maxDelayMs: 0,
        },
      });
      await assert.rejects(events.next(), FastiProtocolError);
    },
  );
});

test("receipt SSE refuses a cursor that differs from the governed receipt id", async () => {
  await withServer(
    (_request, response) => {
      response.writeHead(200, { "content-type": "text/event-stream" });
      response.end(sse(ids.receiptB, receipt(ids.receiptA)));
    },
    async (baseUrl) => {
      const events = new FastiClient({ baseUrl }).receiptEvents({
        retryPolicy: { maxAttempts: 1, baseDelayMs: 0, maxDelayMs: 0 },
      });
      await assert.rejects(events.next(), /cursor must equal.*receipt_id/);
    },
  );
});

test("receipt replay rejects a response for a different receipt", async () => {
  const replayed = observationResponse();
  delete replayed.disposition;
  replayed.receipt.receipt_id = ids.receiptB;
  const client = new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    fetch: async () =>
      new Response(JSON.stringify(replayed), {
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(client.replayReceipt(ids.receiptA), (error) => {
    assert.ok(error instanceof FastiProtocolError);
    assert.match(error.message, /violates the generated contract/);
    assert.match(error.cause.message, /requested receipt id/);
    return true;
  });
});

test("credentials are header-only on authenticated surfaces and no offline queue is exposed", async () => {
  const credential = "credential-never-in-url-or-log";
  const logLines = [];
  const original = {
    error: console.error,
    log: console.log,
    warn: console.warn,
  };
  console.error = (...values) => logLines.push(values.join(" "));
  console.log = (...values) => logLines.push(values.join(" "));
  console.warn = (...values) => logLines.push(values.join(" "));
  try {
    await withServer(
      (request, response) => {
        assert.equal(request.url?.includes(credential), false);
        assert.equal(request.headers.authorization, undefined);
        json(response, 200, { status: "healthy", version: "0.1.0" });
      },
      async (baseUrl) => {
        const client = new FastiClient({ baseUrl, credential });
        await client.health();
        const methods = Object.getOwnPropertyNames(
          Object.getPrototypeOf(client),
        ).sort();
        assert.deepEqual(methods, [
          "acceptObservation",
          "applyAnimeGroupingPolicyChange",
          "attachIdentifier",
          "cancelTrailBaseContinuation",
          "clearNuvioCollections",
          "completeTrailBaseContinuation",
          "configureListener",
          "configureMetadataProjection",
          "configureProviderCredential",
          "constructor",
          "createRecord",
          "discoverCapabilities",
          "endBrowserSession",
          "enrollDurableFirstClient",
          "enrollFirstClient",
          "getNuvioCollections",
          "health",
          "initializeDurableNode",
          "initializeNode",
          "listBrowserSessions",
          "listIntegrations",
          "listProviders",
          "listRecords",
          "listTrackingDispositions",
          "previewAnimeGroupingPolicyChange",
          "readAccessProjection",
          "readAnimeGroupingPolicy",
          "readBrowserSession",
          "readMetadataProjection",
          "readProviderHealth",
          "readTrailBaseContinuation",
          "receiptEvents",
          "refreshMetadataClaims",
          "registerNamespace",
          "removeProviderCredential",
          "replaceNuvioCollections",
          "replayReceipt",
          "resolveIdentityRoute",
          "revokeAllBrowserSessions",
          "revokeBrowserSession",
          "revokeCredential",
          "revokeOtherBrowserSessions",
          "rotateBrowserSession",
          "rotateCredential",
          "selectBrowserSessionProfile",
          "selectProfile",
          "setTrackingDisposition",
          "startTrailBaseSignIn",
          "submitObservation",
          "testProviderCredential",
        ]);
      },
    );
    const authenticatedClient = new FastiClient({
      baseUrl: "http://127.0.0.1:8420",
      credential,
      fetch: async (url, init) => {
        assert.equal(String(url).includes(credential), false);
        assert.equal(
          new Headers(init?.headers).get("authorization"),
          `Bearer ${credential}`,
        );
        return new Response(
          JSON.stringify({
            conformance: conformanceMarker(),
            contract_version: PUBLIC_CAPABILITY_REGISTRY.contract_version,
            capability_base_uri: PUBLIC_CAPABILITY_REGISTRY.capability_base_uri,
            surface_profiles: PUBLIC_CAPABILITY_REGISTRY.surface_profiles,
            capabilities: PUBLIC_CAPABILITY_REGISTRY.capabilities,
          }),
          { headers: { "content-type": "application/json" } },
        );
      },
    });
    await authenticatedClient.discoverCapabilities();
  } finally {
    console.error = original.error;
    console.log = original.log;
    console.warn = original.warn;
  }
  assert.equal(logLines.join("\n").includes(credential), false);
  assert.throws(
    () => new FastiClient({ baseUrl: "http://user:secret@localhost:8420" }),
    /must not contain credentials/,
  );
});

test("receipt SSE rejects unbounded chunks and cursors", async (context) => {
  await context.test("oversized transport chunk", async () => {
    await withServer(
      (_request, response) => {
        response.writeHead(200, { "content-type": "text/event-stream" });
        response.end(`data: ${"x".repeat(70 * 1_024)}\n\n`);
      },
      async (baseUrl) => {
        const events = new FastiClient({ baseUrl }).receiptEvents({
          retryPolicy: { maxAttempts: 1, baseDelayMs: 0, maxDelayMs: 0 },
        });
        await assert.rejects(events.next(), FastiProtocolError);
      },
    );
  });

  await context.test("oversized replay cursor", async () => {
    const client = new FastiClient({ baseUrl: "http://127.0.0.1:8420" });
    const events = client.receiptEvents({ cursor: "x".repeat(513) });
    await assert.rejects(events.next(), /single-line value/);
  });
});

test("exact generated parsers reject inherited fields, class instances, and impossible timestamps", () => {
  const inherited = Object.create({ status: "healthy", version: "0.1.0" });
  assert.throws(() => parseHealthResponse(inherited), /plain object/);

  class FakeHealth {
    constructor() {
      this.status = "healthy";
      this.version = "0.1.0";
    }
  }
  assert.throws(() => parseHealthResponse(new FakeHealth()), /plain object/);
  assert.throws(
    () =>
      parseReceiptCommittedEvent({
        ...receipt(ids.receiptA),
        correlation_id: `req_${"1".repeat(32)}`,
      }),
    /invalid format/,
  );
  assert.throws(
    () =>
      parseReceiptCommittedEvent({
        ...receipt(ids.receiptA),
        committed_at: "2026-02-30T03:00:00Z",
      }),
    /real RFC3339/,
  );
  assert.throws(
    () =>
      parseReceiptCommittedEvent({
        ...receipt(ids.receiptA),
        committed_at: "2026-08-22T03:00:00.1234567890Z",
      }),
    /real RFC3339/,
  );
  assert.throws(
    () =>
      parseReceiptCommittedEvent({
        ...receipt(ids.receiptA),
        committed_at: "2026-08-22T25:00:00Z",
      }),
    /real RFC3339/,
  );
  assert.throws(
    () =>
      parseAcceptObservationRequest({
        ...observationRequest(),
        observed_at: {
          ...observationRequest().observed_at,
          original: "2026-08-22T03:00:00+24:00",
        },
      }),
    /real RFC3339/,
  );
});

test("generated record parser accepts required boolean fields", () => {
  const record = parseListRecordsResponse({
    records: [
      {
        record_id: "018f7f2d-8f58-7a0a-8000-000000000001",
        grain: "work",
        status: "active",
        title: {
          tier: "preferred_provider_claim",
          value: "A real local record",
          source: "google-books",
          is_stale: false,
        },
        poster: {
          tier: "empty",
          value: null,
          source: null,
          is_stale: false,
        },
        latest_activity: null,
      },
    ],
    truncated: false,
  }).records[0];
  assert.equal(record.title.is_stale, false);
  assert.equal(record.poster.is_stale, false);
});

test("generated record parser rejects a response missing truncated", () => {
  assert.throws(
    () =>
      parseListRecordsResponse({
        records: [],
      }),
    FastiContractParseError,
  );
});

test("client.listRecords() surfaces the truncated flag through the transport", async () => {
  const client = new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    credential: "records-secret",
    fetch: async () =>
      new Response(JSON.stringify({ records: [], truncated: true }), {
        headers: { "content-type": "application/json" },
      }),
  });

  assert.deepEqual(await client.listRecords(), {
    records: [],
    truncated: true,
  });
});

test("provider SDK keeps reads retry-safe and credential mutations single-attempt", async (context) => {
  await context.test("provider list retries a transient response", async () => {
    let attempts = 0;
    const client = new FastiClient({
      baseUrl: "http://127.0.0.1:8420",
      credential: "provider-reader",
      retryPolicy: { maxAttempts: 2, baseDelayMs: 0, maxDelayMs: 0 },
      fetch: async (_url, init) => {
        attempts += 1;
        assert.equal(init?.method, "GET");
        if (attempts === 1) return new Response(null, { status: 503 });
        return new Response(JSON.stringify(providerListResponse()), {
          headers: { "content-type": "application/json" },
        });
      },
    });
    assert.equal(
      (await client.listProviders()).providers[0].provider_id,
      "tmdb",
    );
    assert.equal(attempts, 2);
  });

  for (const [name, invoke, method] of [
    [
      "configure",
      (client) =>
        client.configureProviderCredential("tmdb", "metadata.search", {
          secret: "write-only-token",
        }),
      "PUT",
    ],
    [
      "remove",
      (client) => client.removeProviderCredential("tmdb", "metadata.search"),
      "DELETE",
    ],
    [
      "test",
      (client) => client.testProviderCredential("tmdb", "metadata.search"),
      "POST",
    ],
  ]) {
    await context.test(`${name} is never retried`, async () => {
      let attempts = 0;
      const client = new FastiClient({
        baseUrl: "http://127.0.0.1:8420",
        credential: "provider-writer",
        retryPolicy: { maxAttempts: 3, baseDelayMs: 0, maxDelayMs: 0 },
        fetch: async (url, init) => {
          attempts += 1;
          assert.equal(init?.method, method);
          assert.match(
            String(url),
            /\/api\/v1\/providers\/tmdb\/credentials\/metadata\.search/,
          );
          return new Response(null, { status: 503 });
        },
      });
      await assert.rejects(invoke(client), FastiTransportError);
      assert.equal(attempts, 1);
    });
  }
});

test("provider SDK rejects ambiguous path identifiers before transport", () => {
  let called = false;
  const client = new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    fetch: async () => {
      called = true;
      return new Response();
    },
  });
  assert.throws(
    () => client.readProviderHealth("tmdb/other"),
    /providerId does not match/,
  );
  assert.equal(called, false);
});

test("base URL semantics reject application paths instead of silently discarding them", () => {
  assert.throws(
    () => new FastiClient({ baseUrl: "http://127.0.0.1:8420/fasti" }),
    /without an application path/,
  );
  assert.doesNotThrow(
    () => new FastiClient({ baseUrl: "http://127.0.0.1:8420/" }),
  );
});

test("connection endpoints preserve custom domains and expose loopback alternatives", () => {
  assert.equal(
    normalizeBaseUrl("https://fasti.internal:9443").origin,
    "https://fasti.internal:9443",
  );
  assert.deepEqual(connectionEndpoint("https://fasti.internal", "build"), {
    url: "https://fasti.internal",
    port: 443,
    source: "build",
    managed: true,
    scheme: "https",
    loopbackAliases: [],
  });
  assert.deepEqual(
    connectionEndpoint("http://localhost:8420").loopbackAliases,
    ["http://localhost:8420", "http://127.0.0.1:8420"],
  );
});

test("connection endpoints reject unsafe origins", () => {
  for (const value of [
    "ftp://fasti.internal",
    "http://user:secret@fasti.internal",
    "http://fasti.internal",
    "https://fasti.internal/path",
    "https://fasti.internal?query=yes",
    "https://fasti.internal#fragment",
    "https://fasti.internal:0",
    "http://127.0.0.1:0",
  ]) {
    assert.throws(() => connectionEndpoint(value));
  }
});
test("generated public metadata preserves complete registry and surface dispositions", () => {
  assert.equal(PUBLIC_CAPABILITY_REGISTRY.capabilities.length, 52);
  assert.equal(
    Object.keys(PUBLIC_CAPABILITY_REGISTRY.surface_profiles).length,
    17,
  );
  const stream = PUBLIC_CAPABILITY_REGISTRY.capabilities.find(
    (capability) => capability.id === "receipt.stream",
  );
  assert.equal(stream.bounded_context, "observation.receipts");
  assert.deepEqual(stream.scopes, ["receipt_read"]);
  assert.ok(stream.problems.length > 0);
  assert.ok(stream.examples.length > 0);
  assert.ok(stream.uat.length > 0);
  const profile = PUBLIC_CAPABILITY_REGISTRY.surface_profiles.b1_receipt_stream;
  assert.deepEqual(profile.json_ld, {
    reason:
      "Receipt stream events are transport envelopes governed by AsyncAPI; their referenced receipt semantics reuse the observation receipt contract rather than a second linked-data class.",
    state: "not_applicable",
  });
  assert.deepEqual(profile.okf, {
    binding: "okf:capability-catalog",
    binding_visibility: "public",
    state: "required",
  });
  assert.notStrictEqual(profile.json_ld, profile.okf);
  assert.equal(RECEIPT_STREAM_CONTRACT.runtimeAvailability, "fixture_only");
  assert.equal(RECEIPT_STREAM_CONTRACT.durability, "none");
  assert.equal(
    RECEIPT_STREAM_CONTRACT.fixtureDelivery,
    "finite_replay_then_close",
  );
  assert.equal(
    RECEIPT_STREAM_CONTRACT.sseIdPointer,
    "$message.payload#/receipt_id",
  );
  assert.deepEqual(
    PUBLIC_CAPABILITY_REGISTRY.surface_profiles.b2_profile_state.ui,
    {
      binding: "ui:{capability_id}",
      binding_visibility: "public",
      state: "required",
    },
  );
});

test("profile tracking disposition SDK is authenticated, exact, and record-bound", async () => {
  const calls = [];
  const client = new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    credential: "profile-state-secret",
    fetch: async (url, init) => {
      calls.push({
        url: String(url),
        method: init?.method,
        authorization: new Headers(init?.headers).get("authorization"),
        body: init?.body === undefined ? undefined : JSON.parse(init.body),
      });
      const body = String(url).endsWith(
        "/api/v1/profile/record-tracking-dispositions",
      )
        ? { states: [], truncated: true }
        : { record_id: contractIds.record, disposition: "watching" };
      return new Response(JSON.stringify(body), {
        headers: { "content-type": "application/json" },
      });
    },
  });

  assert.deepEqual(await client.listTrackingDispositions(), {
    states: [],
    truncated: true,
  });
  assert.deepEqual(
    await client.setTrackingDisposition(contractIds.record, {
      disposition: "watching",
    }),
    { record_id: contractIds.record, disposition: "watching" },
  );
  assert.deepEqual(calls, [
    {
      url: "http://127.0.0.1:8420/api/v1/profile/record-tracking-dispositions",
      method: "GET",
      authorization: "Bearer profile-state-secret",
      body: undefined,
    },
    {
      url: `http://127.0.0.1:8420/api/v1/profile/record-tracking-dispositions/${contractIds.record}`,
      method: "PUT",
      authorization: "Bearer profile-state-secret",
      body: { disposition: "watching" },
    },
  ]);
  assert.throws(
    () =>
      client.setTrackingDisposition("not-a-record", {
        disposition: "dropped",
      }),
    /recordId does not match the generated contract/,
  );
});

test("Nuvio Collections SDK preserves the bare document and its larger bounded response", async () => {
  const largeDocument = Array.from({ length: 64 }, (_, index) => ({
    id: `collection-${index}`,
    title: `Collection ${index}`,
    folders: [],
    extension: "x".repeat(8_192),
  }));
  const calls = [];
  const client = new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    credential: "profile-state-secret",
    fetch: async (url, init) => {
      calls.push({
        url: String(url),
        method: init?.method,
        authorization: new Headers(init?.headers).get("authorization"),
        body: init?.body === undefined ? undefined : JSON.parse(init.body),
      });
      const document =
        init?.method === "DELETE"
          ? null
          : init?.method === "PUT"
            ? JSON.parse(init.body)
            : largeDocument;
      return new Response(JSON.stringify({ document }), {
        headers: { "content-type": "application/json" },
      });
    },
  });

  assert.deepEqual(await client.getNuvioCollections(), {
    document: largeDocument,
  });
  const replacement = [{ id: "collection", title: "Collection", folders: [] }];
  assert.deepEqual(await client.replaceNuvioCollections(replacement), {
    document: replacement,
  });
  assert.deepEqual(await client.clearNuvioCollections(), { document: null });
  assert.deepEqual(
    calls.map(({ url, method, authorization, body }) => ({
      url,
      method,
      authorization,
      body,
    })),
    [
      {
        url: "http://127.0.0.1:8420/api/v1/profile/nuvio-collections",
        method: "GET",
        authorization: "Bearer profile-state-secret",
        body: undefined,
      },
      {
        url: "http://127.0.0.1:8420/api/v1/profile/nuvio-collections",
        method: "PUT",
        authorization: "Bearer profile-state-secret",
        body: replacement,
      },
      {
        url: "http://127.0.0.1:8420/api/v1/profile/nuvio-collections",
        method: "DELETE",
        authorization: "Bearer profile-state-secret",
        body: undefined,
      },
    ],
  );
});

test("capability discovery rejects bogus map keys, values, and collection bounds", async () => {
  const response = () => ({
    conformance: conformanceMarker(),
    contract_version: PUBLIC_CAPABILITY_REGISTRY.contract_version,
    capability_base_uri: PUBLIC_CAPABILITY_REGISTRY.capability_base_uri,
    surface_profiles: structuredClone(
      PUBLIC_CAPABILITY_REGISTRY.surface_profiles,
    ),
    capabilities: structuredClone(PUBLIC_CAPABILITY_REGISTRY.capabilities),
  });
  const mutations = [
    (value) => (value.surface_profiles.bogus = value.surface_profiles.health),
    (value) =>
      (value.surface_profiles.health.bogus = value.surface_profiles.health.sdk),
    (value) => (value.surface_profiles.health.sdk = "bogus"),
    (value) => value.capabilities.pop(),
    (value) => value.capabilities.push(value.capabilities[0]),
  ];
  for (const mutate of mutations) {
    const value = response();
    mutate(value);
    const client = new FastiClient({
      baseUrl: "http://127.0.0.1:8420",
      fetch: async () =>
        new Response(JSON.stringify(value), {
          headers: { "content-type": "application/json" },
        }),
    });
    await assert.rejects(client.discoverCapabilities(), FastiProtocolError);
  }
});

test("async credential resolution honors timeout and caller cancellation", async (context) => {
  const never = () => new Promise(() => {});
  const fetchMustNotRun = async () => {
    assert.fail("network request ran before credential resolution completed");
  };

  await context.test("timeout", async () => {
    const client = new FastiClient({
      baseUrl: "http://127.0.0.1:8420",
      credential: never,
      fetch: fetchMustNotRun,
      timeoutMs: 20,
    });
    await assert.rejects(client.discoverCapabilities(), FastiTimeoutError);
  });

  await context.test("caller cancellation", async () => {
    const controller = new AbortController();
    const client = new FastiClient({
      baseUrl: "http://127.0.0.1:8420",
      credential: never,
      fetch: fetchMustNotRun,
    });
    const pending = client.discoverCapabilities({ signal: controller.signal });
    controller.abort();
    await assert.rejects(pending, FastiAbortError);
  });
});

test("JSON responses are byte bounded before contract parsing", async () => {
  const client = new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    fetch: async () =>
      new Response(
        JSON.stringify({
          status: "healthy",
          version: "0.1.0",
          padding: "x".repeat(513 * 1_024),
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      ),
  });
  await assert.rejects(client.health(), (error) => {
    assert.ok(error instanceof FastiProtocolError);
    assert.match(error.message, /bounded body size/);
    return true;
  });
});

test("receipt SSE reconnects after a reader failure using the last delivered cursor", async () => {
  let connections = 0;
  const encoder = new TextEncoder();
  const client = new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    credential: "reader-secret",
    fetch: async (_url, init) => {
      connections += 1;
      const headers = new Headers(init?.headers);
      assert.equal(headers.get("authorization"), "Bearer reader-secret");
      if (connections === 1) {
        assert.equal(headers.get("last-event-id"), null);
        let delivered = false;
        return new Response(
          new ReadableStream({
            pull(controller) {
              if (!delivered) {
                delivered = true;
                controller.enqueue(
                  encoder.encode(sse(ids.receiptA, receipt(ids.receiptA))),
                );
              } else {
                controller.error(new Error("simulated socket reset"));
              }
            },
          }),
          { headers: { "content-type": "text/event-stream" } },
        );
      }
      assert.equal(headers.get("last-event-id"), ids.receiptA);
      return new Response(sse(ids.receiptB, receipt(ids.receiptB)), {
        headers: { "content-type": "text/event-stream" },
      });
    },
  });
  const events = client.receiptEvents({
    retryPolicy: { maxAttempts: 2, baseDelayMs: 0, maxDelayMs: 0 },
  });
  assert.equal((await events.next()).value.id, ids.receiptA);
  assert.equal((await events.next()).value.id, ids.receiptB);
  await events.return();
  assert.equal(connections, 2);
});

test("invalid SSE UTF-8 is a protocol failure and is never reconnected", async () => {
  let connections = 0;
  const client = new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    fetch: async () => {
      connections += 1;
      return new Response(new Uint8Array([0xff]), {
        headers: { "content-type": "text/event-stream" },
      });
    },
  });
  const events = client.receiptEvents({
    retryPolicy: { maxAttempts: 3, baseDelayMs: 0, maxDelayMs: 0 },
  });
  await assert.rejects(events.next(), FastiProtocolError);
  assert.equal(connections, 1);
});

test("invalid server-provided SSE cursor is a typed protocol failure", async () => {
  const events = streamClient(
    `id: \nevent: receiptCommitted\ndata: ${JSON.stringify(receipt(ids.receiptA))}\n\n`,
  ).receiptEvents({
    retryPolicy: { maxAttempts: 1, baseDelayMs: 0, maxDelayMs: 0 },
  });
  await assert.rejects(events.next(), FastiProtocolError);
});

test("SSE event limits count empty data lines and aggregate bytes", async (context) => {
  await context.test("empty data lines", async () => {
    const client = streamClient(`${`data:\n`.repeat(257)}\n`);
    const events = client.receiptEvents({
      retryPolicy: { maxAttempts: 1, baseDelayMs: 0, maxDelayMs: 0 },
    });
    await assert.rejects(events.next(), /bounded line count/);
  });

  await context.test("aggregate bytes across bounded lines", async () => {
    const line = `data: ${"x".repeat(60 * 1_024)}\n`;
    const client = streamClient(`${line.repeat(5)}\n`);
    const events = client.receiptEvents({
      retryPolicy: { maxAttempts: 1, baseDelayMs: 0, maxDelayMs: 0 },
    });
    await assert.rejects(events.next(), /bounded aggregate size/);
  });
});

test("mutation retries require stable idempotency and preserve exact serialized bytes", async (context) => {
  await context.test("bootstrap mutation is never retried", async () => {
    let attempts = 0;
    const client = new FastiClient({
      baseUrl: "http://127.0.0.1:8420",
      retryPolicy: { maxAttempts: 3, baseDelayMs: 0, maxDelayMs: 0 },
      fetch: async (_url, init) => {
        attempts += 1;
        assert.equal(init?.method, "POST");
        assert.equal(init?.body, "{}");
        return new Response(null, { status: 503 });
      },
    });
    await assert.rejects(client.initializeNode(), (error) => {
      assert.ok(error instanceof FastiTransportError);
      assert.equal(error.status, 503);
      return true;
    });
    assert.equal(attempts, 1);
  });

  await context.test(
    "operation-ID mutation retries byte-identically",
    async () => {
      const bodies = [];
      let attempts = 0;
      const credential = "writer-secret";
      const client = new FastiClient({
        baseUrl: "http://127.0.0.1:8420",
        credential,
        retryPolicy: { maxAttempts: 2, baseDelayMs: 0, maxDelayMs: 0 },
        fetch: async (url, init) => {
          attempts += 1;
          assert.equal(
            String(url),
            "http://127.0.0.1:8420/api/v1/observations",
          );
          assert.equal(
            new Headers(init?.headers).get("authorization"),
            `Bearer ${credential}`,
          );
          bodies.push(init?.body);
          if (attempts === 1) return new Response(null, { status: 503 });
          return new Response(JSON.stringify(observationResponse()), {
            status: 200,
            headers: { "content-type": "application/json" },
          });
        },
      });
      const result = await client.acceptObservation(observationRequest());
      assert.equal(result.receipt.operation_id, contractIds.operation);
      assert.equal(attempts, 2);
      assert.equal(bodies[0], bodies[1]);
      assert.equal(JSON.parse(bodies[0]).operation_id, contractIds.operation);
    },
  );
});

test("metadata retries require a stable operation ID", async (context) => {
  for (const [name, method, path, attempts, invoke] of [
    [
      "claim refresh",
      "POST",
      "/api/v1/metadata/claims/refresh",
      3,
      (client) =>
        client.refreshMetadataClaims({
          operation_id: "op_018f0e0e7f7b70008000000000000004",
          record_id: contractIds.record,
          provider_id: "tmdb",
          field_groups: ["basic_info"],
          locale: "en-IE",
          region: "IE",
          mode: "revalidate",
        }),
    ],
    [
      "projection configuration",
      "PUT",
      "/api/v1/profile/metadata-projection",
      1,
      (client) =>
        client.configureMetadataProjection({
          preferred_provider_id: "tmdb",
          preferred_locale: "en-IE",
          original_locale: null,
          allow_english_fallback: true,
          last_known_good: "allow",
          region: "IE",
          enabled_field_groups: ["basic_info"],
          overrides: [],
        }),
    ],
  ]) {
    await context.test(name, async () => {
      let requests = 0;
      const bodies = [];
      const client = new FastiClient({
        baseUrl: "http://127.0.0.1:8420",
        credential: "metadata-writer",
        retryPolicy: { maxAttempts: 3, baseDelayMs: 0, maxDelayMs: 0 },
        fetch: async (url, init) => {
          requests += 1;
          bodies.push(init?.body);
          assert.equal(init?.method, method);
          assert.equal(new URL(url).pathname, path);
          return new Response(
            new ReadableStream({
              start(controller) {
                controller.error(new Error("simulated response socket reset"));
              },
            }),
            { status: 200, headers: { "content-type": "application/json" } },
          );
        },
      });

      await assert.rejects(invoke(client), FastiTransportError);
      assert.equal(requests, attempts);
      assert.equal(new Set(bodies).size, 1);
    });
  }
});

test("metadata override parser rejects mismatched discriminated operations", () => {
  const base = {
    preferred_provider_id: "tmdb",
    preferred_locale: "en-IE",
    original_locale: null,
    allow_english_fallback: true,
    last_known_good: "allow",
    region: "IE",
    enabled_field_groups: ["basic_info"],
  };
  for (const override of [
    {
      operation: "set",
      record_id: contractIds.record,
      field_key: "core.title",
    },
    {
      operation: "clear",
      record_id: contractIds.record,
      field_key: "core.title",
      value: "must not be accepted",
    },
    {
      operation: "set",
      record_id: contractIds.record,
      field_key: "Core Title",
      value: "Replacement",
    },
  ]) {
    assert.throws(
      () =>
        parseConfigureMetadataProjectionRequest({
          ...base,
          overrides: [override],
        }),
      FastiContractParseError,
    );
  }
});

test("all implemented contract routes complete against the loopback Rust fixture", async () => {
  await withRustFixture(async (baseUrl) => {
    const bootstrap = new FastiClient({ baseUrl });
    const initialized = await bootstrap.initializeNode();
    assert.deepEqual(initialized.conformance, conformanceMarker());
    assert.match(initialized.initialization_proof, /^[0-9a-f]{64}$/);

    const enrolled = await bootstrap.enrollFirstClient({
      initialization_proof: initialized.initialization_proof,
    });
    assert.deepEqual(enrolled.conformance, conformanceMarker());
    assert.equal(enrolled.credential_scheme, "Bearer");
    assert.match(enrolled.credential, /^[0-9a-f]{64}$/);

    const client = new FastiClient({
      baseUrl,
      credential: enrolled.credential,
      retryPolicy: { baseDelayMs: 0, maxDelayMs: 0 },
    });
    const discovery = await client.discoverCapabilities();
    assert.deepEqual(discovery.conformance, conformanceMarker());
    assert.equal(
      discovery.contract_version,
      PUBLIC_CAPABILITY_REGISTRY.contract_version,
    );
    assert.equal(
      discovery.capability_base_uri,
      PUBLIC_CAPABILITY_REGISTRY.capability_base_uri,
    );
    assert.deepEqual(
      discovery.surface_profiles,
      PUBLIC_CAPABILITY_REGISTRY.surface_profiles,
    );
    assert.equal(discovery.capabilities.length, 52);
    assert.ok(
      discovery.capabilities.some(
        (capability) =>
          capability.id === "receipt.stream" &&
          capability.bounded_context === "observation.receipts" &&
          capability.scopes.includes("receipt_read") &&
          capability.problems.length > 0 &&
          capability.examples.length > 0 &&
          capability.uat.length > 0,
      ),
    );

    for (const operation of [
      () => client.selectProfile(),
      () => client.rotateCredential(),
      () => client.revokeCredential(),
      () => client.configureListener(),
    ]) {
      await assert.rejects(operation(), (error) => {
        assert.ok(error instanceof FastiProblemError);
        assert.equal(error.problem.status, 501);
        assert.equal(error.problem.code, "capability_unavailable");
        return true;
      });
    }

    const accepted = await client.acceptObservation(observationRequest());
    assert.deepEqual(accepted.conformance, conformanceMarker());
    assert.equal(accepted.disposition, "committed");
    assert.equal(accepted.receipt.operation_id, contractIds.operation);

    const replayed = await client.replayReceipt(accepted.receipt.receipt_id);
    assert.deepEqual(replayed.conformance, conformanceMarker());
    assert.deepEqual(replayed.receipt, accepted.receipt);

    const events = client.receiptEvents({
      retryPolicy: { maxAttempts: 2, baseDelayMs: 0, maxDelayMs: 0 },
    });
    const event = await events.next();
    assert.equal(event.done, false);
    assert.equal(event.value.id, accepted.receipt.receipt_id);
    assert.equal(event.value.data.receipt_id, accepted.receipt.receipt_id);
    assert.equal(event.value.data.operation_id, contractIds.operation);
    assert.equal(RECEIPT_STREAM_CONTRACT.durability, "none");
    assert.equal(
      RECEIPT_STREAM_CONTRACT.fixtureDelivery,
      "finite_replay_then_close",
    );
    const completed = await events.next();
    assert.equal(completed.done, true);

    const afterCursor = client.receiptEvents({
      cursor: event.value.id,
      retryPolicy: { maxAttempts: 2, baseDelayMs: 0, maxDelayMs: 0 },
    });
    assert.equal((await afterCursor.next()).done, true);
  });
});

function receipt(receiptId) {
  return {
    capability_id: "observation.accept",
    correlation_id: ids.correlation,
    receipt_id: receiptId,
    operation_id: ids.operation,
    observation_id: ids.observation,
    resolution: "unresolved",
    committed_at: "2026-08-22T03:00:00Z",
  };
}

function observationRequest() {
  return {
    operation_id: contractIds.operation,
    observed_at: {
      original: "2026-08-22T03:00:00Z",
      precision: "second",
      trust: "device_observed",
    },
    evidence: {
      evidence_id: contractIds.evidence,
      digest: `sha256:${"a".repeat(64)}`,
      byte_length: 42,
    },
  };
}

function observationResponse() {
  return {
    conformance: conformanceMarker(),
    disposition: "committed",
    receipt: {
      receipt_id: contractIds.receipt,
      operation_id: contractIds.operation,
      workspace_id: contractIds.workspace,
      profile_id: contractIds.profile,
      source_client_id: contractIds.client,
      observation_id: contractIds.observation,
      evidence_id: contractIds.evidence,
      payload_digest: `sha256:${"a".repeat(64)}`,
      resolution: "unresolved",
      received_at: "2026-08-22T03:00:01Z",
      committed_at: "2026-08-22T03:00:02Z",
    },
  };
}

function conformanceMarker() {
  return { availability: "fixture_only", durability: "none" };
}

function v7(prefix, fill) {
  return `${prefix}_${fill.repeat(12)}7${fill.repeat(3)}8${fill.repeat(15)}`;
}

function streamClient(body) {
  return new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    fetch: async () =>
      new Response(body, {
        headers: { "content-type": "text/event-stream" },
      }),
  });
}

function sse(id, payload) {
  return `id: ${id}\nevent: receiptCommitted\ndata: ${JSON.stringify(payload)}\n\n`;
}

function providerListResponse() {
  const passed = {
    state: "passed",
    checked_at: "2026-08-30T12:00:00Z",
    safe_problem_code: null,
  };
  return {
    providers: [
      {
        provider_id: "tmdb",
        display_name: "The Movie Database (TMDB)",
        provider_kind: "metadata",
        documentation_url:
          "https://developer.themoviedb.org/docs/authentication-application",
        attribution:
          "This product uses the TMDB API but is not endorsed or certified by TMDB.",
        supported_media_grains: ["film", "series"],
        capabilities: [
          {
            capability_id: "metadata.search",
            purpose: "Search provider metadata",
            credential_requirement: "bearer_token",
            credential_state: "valid",
            credential_source: "credential_store",
            state: "available",
            version: 1,
            writable: true,
            testable: true,
            health: passed,
            credential_test: passed,
          },
        ],
        network_hosts: ["api.themoviedb.org"],
        locale_support: true,
        region_support: true,
        identity_namespaces: ["tmdb.movie", "tmdb.tv"],
      },
    ],
  };
}

function json(response, status, body) {
  response.writeHead(status, { "content-type": "application/json" });
  response.end(JSON.stringify(body));
}

async function withServer(handler, exercise) {
  const server = createServer(handler);
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  assert.notEqual(address, null);
  assert.equal(typeof address, "object");
  try {
    await exercise(`http://127.0.0.1:${address.port}`);
  } finally {
    server.closeAllConnections();
    await new Promise((resolve, reject) => {
      server.close((error) => (error ? reject(error) : resolve()));
    });
  }
}

async function withRustFixture(exercise) {
  await execFileAsync(
    "cargo",
    [
      "build",
      "--quiet",
      "-p",
      "fasti-api",
      "--features",
      "conformance-fixture",
      "--bin",
      "b1-conformance-server",
    ],
    { cwd: repositoryRoot, timeout: 120_000 },
  );
  const executable = path.join(
    repositoryRoot,
    "target",
    "debug",
    process.platform === "win32"
      ? "b1-conformance-server.exe"
      : "b1-conformance-server",
  );
  const child = spawn(executable, ["127.0.0.1:0"], {
    cwd: repositoryRoot,
    env: { ...process.env, RUST_BACKTRACE: "1" },
    stdio: ["ignore", "pipe", "pipe"],
  });
  let stderr = "";
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => {
    stderr += chunk;
  });
  try {
    const readiness = await readReadiness(child, () => stderr);
    assert.equal(readiness.event, "ready");
    assert.equal(readiness.availability, "fixture_only");
    assert.equal(readiness.durability, "none");
    assert.match(readiness.address, /^127\.0\.0\.1:\d+$/);
    try {
      await exercise(`http://${readiness.address}`);
    } catch (error) {
      throw new Error(
        `Rust fixture exercise failed: ${fixtureProcessContext(child, stderr)}`,
        { cause: error },
      );
    }
  } finally {
    if (child.exitCode === null && child.signalCode === null) {
      child.kill("SIGINT");
      await waitForExit(child, 5_000);
      if (child.exitCode === null && child.signalCode === null) {
        child.kill("SIGKILL");
        await waitForExit(child, 5_000);
      }
    }
  }
}

function fixtureProcessContext(child, stderr) {
  const boundedStderr = stderr.slice(-16 * 1024).trim();
  return [
    `exit_code=${child.exitCode ?? "running"}`,
    `signal=${child.signalCode ?? "none"}`,
    `stderr=${boundedStderr || "<empty>"}`,
  ].join(" ");
}

async function readReadiness(child, stderr) {
  child.stdout.setEncoding("utf8");
  return await new Promise((resolve, reject) => {
    let output = "";
    const timer = setTimeout(() => {
      reject(new Error(`Rust fixture readiness timed out: ${stderr()}`));
    }, 10_000);
    const finish = (callback) => {
      clearTimeout(timer);
      child.stdout.removeAllListeners("data");
      child.removeAllListeners("exit");
      callback();
    };
    child.stdout.on("data", (chunk) => {
      output += chunk;
      const newline = output.indexOf("\n");
      if (newline !== -1) {
        const line = output.slice(0, newline);
        finish(() => {
          try {
            resolve(JSON.parse(line));
          } catch (error) {
            reject(error);
          }
        });
      }
    });
    child.once("exit", (code) => {
      finish(() =>
        reject(
          new Error(
            `Rust fixture exited before readiness (${code}): ${stderr()}`,
          ),
        ),
      );
    });
  });
}

async function waitForExit(child, timeoutMs) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  await Promise.race([
    new Promise((resolve) => child.once("exit", resolve)),
    new Promise((resolve) => setTimeout(resolve, timeoutMs)),
  ]);
}
