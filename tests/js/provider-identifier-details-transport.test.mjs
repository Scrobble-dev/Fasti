import assert from "node:assert/strict";
import { test } from "node:test";

import {
  FastiAbortError,
  FastiClient,
  FastiContractParseError,
  FastiProtocolError,
  parseProviderIdentifierDetailsQueryParameters,
  parseProviderIdentifierDetailsResponse,
} from "../../packages/sdk/dist/transport.js";

// Actual SDK with synthetic transport responses. Runtime and store tests own
// live provider access, transactional authorization and the no-persistence gate.
const providerRecordId = "438631";
const candidate = () => ({
  provider: "tmdb",
  provider_id: providerRecordId,
  grain: "film",
  kind: "movie",
  title: "Dune",
  original_title: null,
  release_year: 2021,
  authors: [],
  image_url: null,
  overview: "Transient exact-coordinate details.",
});
const details = (locale = "fr-fr") => ({
  outcome: "details",
  provider_id: "tmdb",
  grain: "film",
  provider_record_id: providerRecordId,
  details: candidate(),
  locale,
});
const unavailable = () => ({
  outcome: "unavailable",
  provider_id: "tmdb",
  grain: "film",
  provider_record_id: providerRecordId,
  problem_code: "provider_unavailable",
});
const json = (value) =>
  new Response(JSON.stringify(value), {
    headers: { "content-type": "application/json" },
  });
const clientWith = (fetch, options = {}) =>
  new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    credential: "synthetic-coordinate-reader",
    fetch,
    ...options,
  });
const read = (
  client,
  query = {
    provider_record_id: providerRecordId,
    offline: false,
    locale: "fr-FR",
  },
  options = {},
) => client.readProviderIdentifierDetails("tmdb", "film", query, options);

test("provider identifier details GET binds exact query and credential without mutation headers", async (context) => {
  for (const [query, expectedQuery, response] of [
    [
      {
        provider_record_id: providerRecordId,
        offline: false,
        locale: "fr-FR",
      },
      `provider_record_id=${providerRecordId}&offline=false&locale=fr-FR`,
      details(),
    ],
    [
      { provider_record_id: providerRecordId, offline: true, locale: null },
      `provider_record_id=${providerRecordId}&offline=true`,
      unavailable(),
    ],
    [
      { provider_record_id: providerRecordId, offline: true },
      `provider_record_id=${providerRecordId}&offline=true`,
      unavailable(),
    ],
  ]) {
    await context.test(expectedQuery, async () => {
      const original = structuredClone(query);
      let calls = 0;
      const client = clientWith(async (url, init) => {
        calls += 1;
        assert.equal(
          String(url),
          `http://127.0.0.1:8420/api/v1/search/providers/tmdb/film/details?${expectedQuery}`,
        );
        assert.equal(init.method, "GET");
        assert.equal(init.body, undefined);
        assert.equal(init.credentials, "same-origin");
        const headers = new Headers(init.headers);
        assert.equal(
          headers.get("authorization"),
          "Bearer synthetic-coordinate-reader",
        );
        assert.equal(headers.get("x-csrf-token"), null);
        assert.equal(headers.get("content-type"), null);
        return json(response);
      });
      const pending = read(client, query);
      query.provider_record_id = "999";
      query.offline = !original.offline;
      query.locale = "de-DE";
      assert.deepEqual(await pending, response);
      assert.equal(calls, 1);
    });
  }
});

test("provider identifier detail parsers preserve only the two transient outcomes", () => {
  for (const query of [
    { provider_record_id: providerRecordId, offline: false },
    {
      provider_record_id: providerRecordId,
      offline: false,
      locale: "fr-FR",
    },
    { provider_record_id: providerRecordId, offline: true, locale: null },
  ]) {
    assert.deepEqual(
      parseProviderIdentifierDetailsQueryParameters(query),
      query,
    );
  }
  for (const response of [details(), details(null), unavailable()]) {
    assert.deepEqual(
      parseProviderIdentifierDetailsResponse(response),
      response,
    );
  }
});

test("provider identifier details reject receipts snapshots and mixed durable state", () => {
  for (const factory of [details, unavailable]) {
    for (const [field, value] of [
      ["receipt", { candidate_receipt_id: "private" }],
      ["snapshot", { receipt: {} }],
      ["candidate_receipt_id", "scr_01991f588e0070008000000000000001"],
      ["lifetime", { expires_at: "2026-09-07T00:00:00Z" }],
      ["partition", { profile_id: "private" }],
      ["response_cache_policy", { reuse: "no_store" }],
      ["extra", true],
    ]) {
      const response = factory();
      response[field] = value;
      assert.throws(
        () => parseProviderIdentifierDetailsResponse(response),
        FastiContractParseError,
      );
    }
  }

  for (const mutate of [
    (value) => {
      value.problem_code = "provider_unavailable";
    },
    (value) => {
      delete value.details;
    },
    (value) => {
      value.details = null;
    },
  ]) {
    const response = details();
    mutate(response);
    assert.throws(
      () => parseProviderIdentifierDetailsResponse(response),
      FastiContractParseError,
    );
  }
  for (const mutate of [
    (value) => {
      value.details = candidate();
    },
    (value) => {
      value.locale = "fr-fr";
    },
    (value) => {
      delete value.problem_code;
    },
  ]) {
    const response = unavailable();
    mutate(response);
    assert.throws(
      () => parseProviderIdentifierDetailsResponse(response),
      FastiContractParseError,
    );
  }
});

test("provider identifier details bind response envelope and detail identity to the request", async (context) => {
  for (const [name, mutate] of [
    ["envelope provider", (value) => (value.provider_id = "google-books")],
    ["envelope grain", (value) => (value.grain = "series")],
    ["envelope identifier", (value) => (value.provider_record_id = "438632")],
    ["detail provider", (value) => (value.details.provider = "google-books")],
    ["detail grain", (value) => (value.details.grain = "series")],
    ["detail identifier", (value) => (value.details.provider_id = "438632")],
  ]) {
    await context.test(name, async () => {
      const response = details();
      mutate(response);
      await assert.rejects(
        read(clientWith(async () => json(response))),
        FastiProtocolError,
      );
    });
  }
  for (const [name, mutate] of [
    ["provider", (value) => (value.provider_id = "google-books")],
    ["grain", (value) => (value.grain = "series")],
    ["identifier", (value) => (value.provider_record_id = "438632")],
  ]) {
    await context.test(`unavailable ${name}`, async () => {
      const response = unavailable();
      mutate(response);
      await assert.rejects(
        read(clientWith(async () => json(response))),
        FastiProtocolError,
      );
    });
  }
});

test("offline provider identifier details reject disclosed provider payload", async () => {
  const query = { provider_record_id: providerRecordId, offline: true };
  await assert.rejects(
    read(
      clientWith(async () => json(details(null))),
      query,
    ),
    FastiProtocolError,
  );
  assert.deepEqual(
    await read(
      clientWith(async () => json(unavailable())),
      query,
    ),
    unavailable(),
  );
});

test("provider identifier details reject malformed query and route before transport", () => {
  let calls = 0;
  const client = clientWith(async () => {
    calls += 1;
    return json(unavailable());
  });
  for (const query of [
    {},
    { provider_record_id: providerRecordId },
    { provider_record_id: "", offline: false },
    { provider_record_id: "x".repeat(257), offline: false },
    { provider_record_id: providerRecordId, offline: "false" },
    { provider_record_id: providerRecordId, offline: false, locale: "f" },
    {
      provider_record_id: providerRecordId,
      offline: false,
      locale: "x".repeat(17),
    },
    {
      provider_record_id: providerRecordId,
      offline: false,
      candidate_receipt_id: "private",
    },
  ]) {
    assert.throws(
      () => parseProviderIdentifierDetailsQueryParameters(query),
      FastiContractParseError,
    );
    assert.throws(() => read(client, query), FastiProtocolError);
  }
  for (const [provider, grain] of [
    ["tmdb/other", "film"],
    ["tmdb?offline=false", "film"],
    ["tmdb\n", "film"],
    ["tmdb", "film/other"],
    ["tmdb", "Film"],
  ]) {
    assert.throws(() =>
      client.readProviderIdentifierDetails(provider, grain, {
        provider_record_id: providerRecordId,
        offline: false,
      }),
    );
  }
  assert.equal(calls, 0);
});

test("provider identifier details pass CallOptions abort to the in-flight GET", async () => {
  const controller = new AbortController();
  let started;
  const ready = new Promise((resolve) => {
    started = resolve;
  });
  let signal;
  let calls = 0;
  const client = clientWith(async (_url, init) => {
    calls += 1;
    signal = init.signal;
    return new Promise((_resolve, reject) => {
      signal.addEventListener(
        "abort",
        () => reject(new DOMException("Aborted", "AbortError")),
        { once: true },
      );
      started();
    });
  });
  const pending = read(
    client,
    { provider_record_id: providerRecordId, offline: false },
    { signal: controller.signal },
  );
  const rejected = assert.rejects(pending, FastiAbortError);
  await ready;
  controller.abort();
  await rejected;
  assert.equal(signal.aborted, true);
  assert.equal(calls, 1);
});
