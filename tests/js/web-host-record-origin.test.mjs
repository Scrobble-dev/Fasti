import assert from "node:assert/strict";
import test from "node:test";

import { createWebHost } from "../../apps/web/src/web-host.ts";
import { FastiClient } from "../../packages/sdk/dist/transport.js";

const browserOrigin = "http://127.0.0.1:4173";
const serviceOrigin = "http://127.0.0.1:8422";
const updatedServiceOrigin = "http://127.0.0.1:8433";
const csrfToken = "c".repeat(64);
const recordId = `rec_${"8".repeat(12)}7${"8".repeat(3)}8${"8".repeat(15)}`;
const externalIdentifierId = `xid_${"9".repeat(12)}7${"9".repeat(3)}8${"9".repeat(15)}`;
const contractId = (prefix, fill) =>
  `${prefix}_${fill.repeat(12)}7${fill.repeat(3)}8${fill.repeat(15)}`;

const observationRequest = {
  kind: "consumption_occurrence",
  source: "nuvio",
  source_event_id: "session-42:stop:episode-7",
  observed_at: "2026-08-26T18:10:00Z",
  occurred_at: "2026-08-26T18:09:58Z",
  target_grain: "episode",
  identifiers: [],
  title: "Example episode",
  progress_percent: 100,
  position_seconds: 1440,
  duration_seconds: 1440,
};

const observationResponse = {
  disposition: "committed",
  receipt_id: contractId("rcp", "1"),
  operation_id: contractId("op", "2"),
  workspace_id: contractId("wsp", "3"),
  profile_id: contractId("prf", "4"),
  source_client_id: contractId("cli", "5"),
  observation_id: contractId("obs", "6"),
  evidence_id: contractId("evd", "7"),
  payload_digest: `sha256:${"a".repeat(64)}`,
  resolution: "unresolved",
  received_at: "2026-08-26T18:10:01Z",
  committed_at: "2026-08-26T18:10:02Z",
};

const record = {
  record_id: recordId,
  grain: "film",
  status: "active",
  title: {
    tier: "preferred_provider_claim",
    value: "Origin-bound Record",
    source: "tmdb",
    is_stale: false,
  },
  poster: {
    tier: "preferred_provider_claim",
    value: "https://image.example/poster.jpg",
    source: "tmdb",
    is_stale: false,
  },
  identifiers: [{ namespace: "tmdb.movie", grain: "film", value: "42" }],
  latest_activity: null,
};

const attachRequest = {
  record_id: recordId,
  namespace: "imdb",
  grain: "film",
  value: "tt0000042",
};
const namespaceRequest = {
  namespace: "imdb",
  label: "IMDb",
  grains: ["film"],
  id_pattern: "^tt[0-9]{7,8}$",
  normalization: "none",
  licence_posture: "identifiers_only",
};
const nuvioDocument = [
  { id: "origin-check", title: "Origin check", folders: [] },
];
const outboundPolicy = {
  allow_providers: [],
  deny_providers: [],
  allow_capabilities: [],
  deny_capabilities: [],
  allow_hosts: [],
  deny_hosts: [],
  allow_networks: [],
  deny_networks: [],
};

function replaceGlobal(name, value) {
  const original = Object.getOwnPropertyDescriptor(globalThis, name);
  Object.defineProperty(globalThis, name, {
    configurable: true,
    writable: true,
    value,
  });
  return () => {
    if (original) Object.defineProperty(globalThis, name, original);
    else delete globalThis[name];
  };
}

function jsonResponse(value) {
  return new Response(JSON.stringify(value), {
    headers: { "content-type": "application/json" },
  });
}

async function exerciseHost(credential) {
  const calls = [];
  const savedNetworkConfigurations = [];
  const restore = [
    replaceGlobal("window", { location: { origin: browserOrigin } }),
    replaceGlobal("document", {
      cookie: `__Host-fasti_csrf=${csrfToken}`,
    }),
    replaceGlobal("localStorage", {
      getItem(key) {
        assert.equal(key, "fasti-network-config");
        return JSON.stringify({ service_url: serviceOrigin });
      },
      setItem(key, value) {
        assert.equal(key, "fasti-network-config");
        savedNetworkConfigurations.push(JSON.parse(value));
      },
    }),
    replaceGlobal("fetch", async (input, init = {}) => {
      const url = new URL(String(input));
      const headers = new Headers(init.headers);
      const method = init.method ?? "GET";
      const body =
        init.body === undefined ? undefined : JSON.parse(String(init.body));
      calls.push({
        origin: url.origin,
        path: url.pathname,
        recordId: url.searchParams.get("record_id"),
        method,
        credentials: init.credentials,
        authorization: headers.get("authorization"),
        csrf: headers.get("x-csrf-token"),
        body,
      });

      if (method === "GET" && url.pathname === "/api/v1/records") {
        return jsonResponse({ records: [record], truncated: false });
      }
      if (method === "POST" && url.pathname === "/api/v1/records") {
        return jsonResponse({ record_id: recordId, grain: "film" });
      }
      if (method === "POST" && url.pathname === "/api/v1/records/identifiers") {
        return jsonResponse({
          external_identifier_id: externalIdentifierId,
          record_id: recordId,
          created: true,
        });
      }
      if (method === "POST" && url.pathname === "/api/v1/namespaces") {
        return jsonResponse({ namespace: "imdb", created: true });
      }
      if (
        method === "GET" &&
        url.pathname === "/api/v1/profile/record-tracking-dispositions"
      ) {
        return jsonResponse({
          states: [{ record_id: recordId, disposition: "watching" }],
          truncated: false,
        });
      }
      if (
        method === "PUT" &&
        url.pathname ===
          `/api/v1/profile/record-tracking-dispositions/${recordId}`
      ) {
        return jsonResponse({ record_id: recordId, disposition: "watching" });
      }
      if (url.pathname === "/api/v1/profile/nuvio-collections") {
        if (method === "GET") return jsonResponse({ document: nuvioDocument });
        if (method === "PUT") return jsonResponse({ document: body });
        if (method === "DELETE") return jsonResponse({ document: null });
      }
      return assert.fail(`unexpected web-host request ${method} ${url}`);
    }),
  ];

  try {
    const host = createWebHost(browserOrigin, credential);
    const records = await host.listRecords({ record_id: recordId });
    const created = await host.createRecord("film");
    const attached = await host.attachIdentifier(attachRequest);
    const registered = await host.registerNamespace(namespaceRequest);
    const tracking = await host.listTrackingDispositions();
    const trackingSet = await host.setTrackingDisposition(recordId, "watching");
    const collections = await host.getNuvioCollections();
    const collectionsReplaced =
      await host.replaceNuvioCollections(nuvioDocument);
    const collectionsCleared = await host.clearNuvioCollections();
    const savedNetwork = await host.saveNetworkConfiguration({
      service_url: updatedServiceOrigin,
      public_url: null,
      outbound_policy: outboundPolicy,
    });
    const updatedRecords = await host.listRecords({ record_id: recordId });
    const updatedTrackingSet = await host.setTrackingDisposition(
      recordId,
      "watching",
    );
    return {
      calls,
      savedNetworkConfigurations,
      records,
      created,
      attached,
      registered,
      tracking,
      trackingSet,
      collections,
      collectionsReplaced,
      collectionsCleared,
      savedNetwork,
      updatedRecords,
      updatedTrackingSet,
    };
  } finally {
    for (const restoreGlobal of restore.reverse()) restoreGlobal();
  }
}

const expectedRequests = [
  ["GET", "/api/v1/records", undefined],
  ["POST", "/api/v1/records", { grain: "film" }],
  ["POST", "/api/v1/records/identifiers", attachRequest],
  ["POST", "/api/v1/namespaces", namespaceRequest],
  ["GET", "/api/v1/profile/record-tracking-dispositions", undefined],
  [
    "PUT",
    `/api/v1/profile/record-tracking-dispositions/${recordId}`,
    { disposition: "watching" },
  ],
  ["GET", "/api/v1/profile/nuvio-collections", undefined],
  ["PUT", "/api/v1/profile/nuvio-collections", nuvioDocument],
  ["DELETE", "/api/v1/profile/nuvio-collections", undefined],
  ["GET", "/api/v1/records", undefined],
  [
    "PUT",
    `/api/v1/profile/record-tracking-dispositions/${recordId}`,
    { disposition: "watching" },
  ],
];

function assertRequests(
  result,
  { initialOrigin, updatedOrigin, authorization, csrf },
) {
  assert.equal(result.calls.length, expectedRequests.length);
  for (const [index, call] of result.calls.entries()) {
    const [method, path, body] = expectedRequests[index];
    assert.deepEqual(call, {
      origin: index < 9 ? initialOrigin : updatedOrigin,
      path,
      recordId: index === 0 || index === 9 ? recordId : null,
      method,
      credentials: "same-origin",
      authorization,
      csrf: method === "GET" ? null : csrf,
      body,
    });
  }
}

function assertProjections(result) {
  assert.deepEqual(result.records, {
    records: [{ ...record, poster: { ...record.poster, value: null } }],
    truncated: false,
  });
  assert.deepEqual(result.created, { record_id: recordId, grain: "film" });
  assert.deepEqual(result.attached, {
    external_identifier_id: externalIdentifierId,
    record_id: recordId,
    created: true,
  });
  assert.deepEqual(result.registered, { namespace: "imdb", created: true });
  assert.deepEqual(result.tracking, {
    states: [{ record_id: recordId, disposition: "watching" }],
    truncated: false,
  });
  assert.deepEqual(result.trackingSet, {
    record_id: recordId,
    disposition: "watching",
  });
  assert.deepEqual(result.collections, { document: nuvioDocument });
  assert.deepEqual(result.collectionsReplaced, { document: nuvioDocument });
  assert.deepEqual(result.collectionsCleared, { document: null });
  assert.deepEqual(result.updatedRecords, result.records);
  assert.deepEqual(result.updatedTrackingSet, result.trackingSet);
  const expectedNetwork = {
    connection: {
      service_url: {
        value: updatedServiceOrigin,
        source: "saved",
        managed: false,
      },
      public_url: { value: null, source: "saved", managed: false },
    },
    outbound_policy: outboundPolicy,
  };
  assert.deepEqual(result.savedNetwork, expectedNetwork);
  assert.deepEqual(result.savedNetworkConfigurations, [expectedNetwork]);
}

test("browser-session Record and profile operations stay on the browser origin with mutation CSRF", async () => {
  const result = await exerciseHost(undefined);

  assertRequests(result, {
    initialOrigin: browserOrigin,
    updatedOrigin: browserOrigin,
    authorization: null,
    csrf: csrfToken,
  });
  assertProjections(result);
});

test("credential Record and profile operations retain the configured service origin", async () => {
  const result = await exerciseHost(async () => "scoped-record-secret");

  assertRequests(result, {
    initialOrigin: serviceOrigin,
    updatedOrigin: updatedServiceOrigin,
    authorization: "Bearer scoped-record-secret",
    csrf: null,
  });
  assertProjections(result);
});

test("browser-session mutations reject a missing CSRF cookie before fetch", async (context) => {
  let fetchCalls = 0;
  const restore = [
    replaceGlobal("window", { location: { origin: browserOrigin } }),
    replaceGlobal("document", { cookie: "" }),
    replaceGlobal("localStorage", {
      getItem() {
        return JSON.stringify({ service_url: serviceOrigin });
      },
    }),
    replaceGlobal("fetch", async () => {
      fetchCalls += 1;
      return jsonResponse({ record_id: recordId, grain: "film" });
    }),
  ];

  try {
    const host = createWebHost(browserOrigin);
    const mutations = [
      ["create Record", () => host.createRecord("film")],
      ["attach identifier", () => host.attachIdentifier(attachRequest)],
      ["register namespace", () => host.registerNamespace(namespaceRequest)],
      [
        "set tracking disposition",
        () => host.setTrackingDisposition(recordId, "watching"),
      ],
      [
        "replace Nuvio Collections",
        () => host.replaceNuvioCollections(nuvioDocument),
      ],
      ["clear Nuvio Collections", () => host.clearNuvioCollections()],
    ];
    for (const [name, mutate] of mutations) {
      await context.test(name, async () => {
        await assert.rejects(
          mutate(),
          /Browser session mutations require one valid CSRF cookie/,
        );
      });
    }
    assert.equal(fetchCalls, 0);
  } finally {
    for (const restoreGlobal of restore.reverse()) restoreGlobal();
  }
});

test("observation submission uses the seventh hybrid mutation boundary", async () => {
  const calls = [];
  const restore = [
    replaceGlobal("document", {
      cookie: `unrelated=1; __Host-fasti_csrf=${csrfToken}`,
    }),
  ];
  const fetch = async (input, init = {}) => {
    const headers = new Headers(init.headers);
    calls.push({
      url: String(input),
      method: init.method,
      credentials: init.credentials,
      authorization: headers.get("authorization"),
      csrf: headers.get("x-csrf-token"),
      body: JSON.parse(String(init.body)),
    });
    return jsonResponse(observationResponse);
  };

  try {
    const browserClient = new FastiClient({ baseUrl: browserOrigin, fetch });
    assert.deepEqual(
      await browserClient.submitObservation(observationRequest),
      observationResponse,
    );

    const bearerClient = new FastiClient({
      baseUrl: serviceOrigin,
      credential: "scoped-observation-secret",
      fetch,
    });
    assert.deepEqual(
      await bearerClient.submitObservation(observationRequest),
      observationResponse,
    );

    assert.deepEqual(calls, [
      {
        url: `${browserOrigin}/api/v1/observations`,
        method: "POST",
        credentials: "same-origin",
        authorization: null,
        csrf: csrfToken,
        body: observationRequest,
      },
      {
        url: `${serviceOrigin}/api/v1/observations`,
        method: "POST",
        credentials: "same-origin",
        authorization: "Bearer scoped-observation-secret",
        csrf: null,
        body: observationRequest,
      },
    ]);
  } finally {
    for (const restoreGlobal of restore.reverse()) restoreGlobal();
  }
});

test("observation submission rejects invalid browser CSRF before fetch", async (context) => {
  const cookies = [
    ["missing", ""],
    ["malformed", "__Host-fasti_csrf=bad"],
    [
      "duplicate",
      `__Host-fasti_csrf=${csrfToken}; __Host-fasti_csrf=${csrfToken}`,
    ],
  ];

  for (const [name, cookie] of cookies) {
    await context.test(name, async () => {
      let fetchCalls = 0;
      const restore = replaceGlobal("document", { cookie });
      try {
        const client = new FastiClient({
          baseUrl: browserOrigin,
          fetch: async () => {
            fetchCalls += 1;
            return jsonResponse(observationResponse);
          },
        });
        await assert.rejects(
          client.submitObservation(observationRequest),
          /Browser session mutations require one valid CSRF cookie/,
        );
        assert.equal(fetchCalls, 0);
      } finally {
        restore();
      }
    });
  }
});

test("browser-session routing does not promote scoped provider or metadata operations", async () => {
  const calls = [];
  const restore = [
    replaceGlobal("window", { location: { origin: browserOrigin } }),
    replaceGlobal("document", {
      cookie: `__Host-fasti_csrf=${csrfToken}`,
    }),
    replaceGlobal("localStorage", {
      getItem() {
        return JSON.stringify({ service_url: serviceOrigin });
      },
      setItem() {},
    }),
    replaceGlobal("fetch", async (input, init = {}) => {
      const headers = new Headers(init.headers);
      calls.push({
        url: String(input),
        credentials: init.credentials,
        authorization: headers.get("authorization"),
        csrf: headers.get("x-csrf-token"),
      });
      throw new Error("bounded provider boundary fixture");
    }),
  ];

  try {
    const host = createWebHost(browserOrigin);
    assert.equal(host.readMetadataProjection, undefined);
    assert.equal(host.configureMetadataProjection, undefined);
    assert.equal(host.refreshMetadataClaims, undefined);
    await host.saveNetworkConfiguration({
      service_url: updatedServiceOrigin,
      public_url: null,
      outbound_policy: outboundPolicy,
    });
    await assert.rejects(
      host.saveProviderCredential("tmdb", "metadata.search", "fixture-secret"),
    );
    assert.deepEqual(calls, [
      {
        url: `${updatedServiceOrigin}/api/v1/providers/tmdb/credentials/metadata.search`,
        credentials: "same-origin",
        authorization: null,
        csrf: null,
      },
    ]);
  } finally {
    for (const restoreGlobal of restore.reverse()) restoreGlobal();
  }
});
