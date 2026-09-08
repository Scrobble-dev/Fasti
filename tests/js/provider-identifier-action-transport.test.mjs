import assert from "node:assert/strict";
import { test } from "node:test";

import {
  FastiClient,
  FastiContractParseError,
  FastiProtocolError,
  parseProviderIdentifierActionRequest,
  parseProviderIdentifierActionResponse,
} from "../../packages/sdk/dist/transport.js";

// Actual SDK with synthetic transport responses. Provider refetch,
// authorization, atomic persistence and durable replay remain runtime gates.
const operationId = "op_01991f588e0070008000000000000001";
const recordId = "rec_01991f588e0070008000000000000001";
const otherRecordId = "rec_01991f588e0070008000000000000002";
const providerRecordId = "438631";
const request = (kind = "create") => ({
  operation_id: operationId,
  provider_record_id: providerRecordId,
  action: kind === "create" ? { kind } : { kind, record_id: recordId },
});
const saved = (body = request(), disposition) => ({
  outcome: "saved",
  receipt: {
    operation_id: body.operation_id,
    provider_id: "tmdb",
    provider_record_id: body.provider_record_id,
    grain: "film",
    action: structuredClone(body.action),
    origin: "user_selected_provider_identifier",
    record_id: body.action.record_id ?? recordId,
    disposition:
      disposition ?? (body.action.kind === "create" ? "created" : "attached"),
    committed_at: "2026-09-05T08:01:00Z",
  },
});
const json = (value) =>
  new Response(JSON.stringify(value), {
    headers: { "content-type": "application/json" },
  });
const clientWith = (fetch, options = {}) =>
  new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    credential: "synthetic-identifier-writer",
    fetch,
    ...options,
  });
const save = (client, body = request(), options = {}) =>
  client.saveProviderIdentifier("tmdb", "film", body, options);

test("provider identifier action POST preserves exact intent and credential boundary", async (context) => {
  for (const [kind, disposition] of [
    ["create", "created"],
    ["create", "reused"],
    ["attach", "attached"],
    ["attach", "already_attached"],
  ]) {
    await context.test(disposition, async () => {
      const body = request(kind);
      const response = saved(body, disposition);
      const client = clientWith(async (url, init) => {
        assert.equal(
          String(url),
          "http://127.0.0.1:8420/api/v1/search/providers/tmdb/film/actions",
        );
        assert.equal(init.method, "POST");
        assert.equal(init.credentials, "same-origin");
        assert.deepEqual(JSON.parse(init.body), body);
        const headers = new Headers(init.headers);
        assert.equal(headers.get("content-type"), "application/json");
        assert.equal(
          headers.get("authorization"),
          "Bearer synthetic-identifier-writer",
        );
        assert.equal(headers.get("x-csrf-token"), null);
        return json(response);
      });
      assert.deepEqual(await save(client, body), response);
    });
  }
});

test("provider identifier action rejects malformed intent before transport", () => {
  let calls = 0;
  const client = clientWith(async () => {
    calls += 1;
    return json(saved());
  });
  for (const body of [
    {},
    { ...request(), operation_id: recordId },
    { ...request(), provider_record_id: "" },
    { ...request(), provider_record_id: "x".repeat(257) },
    { ...request(), action: { kind: "create", record_id: recordId } },
    { ...request(), action: { kind: "attach" } },
    { ...request(), action: { kind: "delete" } },
    { ...request(), grain: "film" },
    { ...request(), metadata: {} },
  ]) {
    assert.throws(
      () => parseProviderIdentifierActionRequest(body),
      FastiContractParseError,
    );
    assert.throws(() => save(client, body), FastiProtocolError);
  }
  for (const [provider, grain] of [
    ["tmdb/other", "film"],
    ["tmdb", "film/other"],
    ["tmdb", "Film"],
  ]) {
    assert.throws(
      () => client.saveProviderIdentifier(provider, grain, request()),
      TypeError,
    );
  }
  assert.equal(calls, 0);
});

test("provider identifier action exposes only the canonical tagged receipt", () => {
  assert.deepEqual(parseProviderIdentifierActionRequest(request()), request());
  assert.deepEqual(parseProviderIdentifierActionResponse(saved()), saved());
  for (const key of [
    "candidate",
    "candidate_receipt_id",
    "metadata",
    "provenance",
    "response_cache_policy",
  ]) {
    const response = saved();
    response.receipt[key] = {};
    assert.throws(
      () => parseProviderIdentifierActionResponse(response),
      FastiContractParseError,
    );
  }
});

test("provider identifier action binds every saved receipt to submitted intent", async (context) => {
  for (const [name, mutate] of [
    [
      "operation",
      (receipt) => (receipt.operation_id = operationId.replace(/1$/, "2")),
    ],
    ["provider", (receipt) => (receipt.provider_id = "google-books")],
    ["identifier", (receipt) => (receipt.provider_record_id = "438632")],
    ["grain", (receipt) => (receipt.grain = "series")],
    ["origin", (receipt) => (receipt.origin = "provider_search")],
    ["action", (receipt) => (receipt.action = { kind: "create" })],
    ["target", (receipt) => (receipt.action.record_id = otherRecordId)],
    ["result", (receipt) => (receipt.record_id = otherRecordId)],
    ["disposition", (receipt) => (receipt.disposition = "created")],
  ]) {
    await context.test(name, async () => {
      const body = request("attach");
      const response = saved(body);
      mutate(response.receipt);
      await assert.rejects(
        save(
          clientWith(async () => json(response)),
          body,
        ),
        FastiProtocolError,
      );
    });
  }
});

test("provider identifier action retries the identical stable operation body", async () => {
  const body = request("attach");
  const original = structuredClone(body);
  const bodies = [];
  const client = clientWith(
    async (_url, init) => {
      bodies.push(init.body);
      if (bodies.length === 1) {
        body.operation_id = operationId.replace(/1$/, "2");
        body.provider_record_id = "438632";
        body.action = { kind: "create" };
        return new Response("Unavailable", { status: 503 });
      }
      return json(saved(original));
    },
    { retryPolicy: { maxAttempts: 2, baseDelayMs: 0, maxDelayMs: 0 } },
  );
  assert.deepEqual(await save(client, body), saved(original));
  assert.equal(bodies.length, 2);
  assert.equal(bodies[0], bodies[1]);
  assert.deepEqual(JSON.parse(bodies[1]), original);
});
