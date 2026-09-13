import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import {
  FastiAbortError,
  FastiClient,
  FastiContractParseError,
  FastiProtocolError,
  LOCAL_SEARCH_MAX_RESPONSE_BYTES,
  parseLocalSearchRequestDto,
  parseLocalSearchResponseDto,
} from "../../packages/sdk/dist/transport.js";

// Actual SDK with synthetic transport responses. Server/store tests own current
// authorization and first-page profile/query binding; a cursor is not authority.
const id = (number) =>
  `rec_01991f588e0070008${number.toString(16).padStart(15, "0")}`;
const digest = `sha256:${"a".repeat(64)}`;
const otherDigest = `sha256:${"b".repeat(64)}`;
const cursor = (number, context_digest = digest) => ({
  last_record_id: id(number),
  context_digest,
});
const request = (after = null, grains = ["film"]) => ({
  query: "Dune",
  grains,
  after,
});
const field = (value, source = "tmdb") => ({
  tier: value === null ? "empty" : "preferred_provider_claim",
  value,
  source: value === null ? null : source,
  is_stale: false,
});
const record = (number, grain = "film") => ({
  record_id: id(number),
  grain,
  status: "active",
  title: field("Dune"),
  poster: field(null),
  original_title: field("Dune"),
  overview: field("A local Record."),
  release_year: field("2021"),
  identifiers: [{ namespace: "tmdb", grain, value: String(number) }],
  latest_activity: null,
});
const page = (numbers = [2, 3], next = null) => ({
  records: numbers.map((number) => record(number)),
  next,
});
const json = (value) =>
  new Response(JSON.stringify(value), {
    headers: { "content-type": "application/json" },
  });
const clientWith = (fetch, options = {}) =>
  new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    credential: "synthetic-local-search-reader",
    fetch,
    ...options,
  });
const retries = {
  retryPolicy: { maxAttempts: 2, baseDelayMs: 0, maxDelayMs: 0 },
};

test("local Search SDK response limit matches the generated OpenAPI operation", async () => {
  const openapi = JSON.parse(
    await readFile(
      new URL("../../contracts/generated/v1/openapi.json", import.meta.url),
      "utf8",
    ),
  );
  assert.equal(
    LOCAL_SEARCH_MAX_RESPONSE_BYTES,
    openapi.paths["/api/v1/search/records"].post["x-fasti-max-response-bytes"],
  );
  assert.equal(LOCAL_SEARCH_MAX_RESPONSE_BYTES, 4 * 1024 * 1024);
});

test("local Search POST sends the bounded read body without a mutation header", async () => {
  const body = request(cursor(1));
  const response = page([2, 3], cursor(5));
  const client = clientWith(async (url, init) => {
    assert.equal(String(url), "http://127.0.0.1:8420/api/v1/search/records");
    assert.equal(init.method, "POST");
    assert.deepEqual(JSON.parse(init.body), body);
    assert.equal(init.credentials, "same-origin");
    const headers = new Headers(init.headers);
    assert.equal(headers.get("content-type"), "application/json");
    assert.equal(
      headers.get("authorization"),
      "Bearer synthetic-local-search-reader",
    );
    assert.equal(headers.get("x-csrf-token"), null);
    return json(response);
  });
  assert.deepEqual(await client.searchRecords(body), response);
});

test("browser local Search read needs no CSRF cookie", async () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, "document");
  Object.defineProperty(globalThis, "document", {
    configurable: true,
    value: { cookie: "" },
  });
  try {
    const client = clientWith(
      async (_url, init) => {
        assert.equal(new Headers(init.headers).get("x-csrf-token"), null);
        assert.equal(new Headers(init.headers).get("authorization"), null);
        return json(page());
      },
      { credential: undefined },
    );
    assert.deepEqual(await client.searchRecords(request()), page());
  } finally {
    if (previous) Object.defineProperty(globalThis, "document", previous);
    else delete globalThis.document;
  }
});

test("local Search request and cursor reject malformed or invented fields before transport", () => {
  let calls = 0;
  const client = clientWith(async () => {
    calls += 1;
    return json(page());
  });
  for (const body of [
    {},
    { query: "Dune", after: null },
    { ...request(), query: "" },
    { ...request(), query: "q".repeat(257) },
    { ...request(), grains: "film" },
    { ...request(), grains: [null] },
    { ...request(), grains: Array(17).fill("film") },
    ...["offline", "provider_id", "profile_id", "operation_id", "locale"].map(
      (key) => ({ ...request(), [key]: "not accepted" }),
    ),
    { ...request(), after: {} },
    {
      ...request(),
      after: { ...cursor(1), last_record_id: id(1).replace("rec_", "scr_") },
    },
    { ...request(), after: { ...cursor(1), last_record_id: `${id(1)}\n` } },
    {
      ...request(),
      after: { ...cursor(1), context_digest: `sha256:${"a".repeat(63)}` },
    },
    {
      ...request(),
      after: { ...cursor(1), context_digest: `sha256:${"G".repeat(64)}` },
    },
    { ...request(), after: { ...cursor(1), profile_id: "not authority" } },
  ]) {
    assert.throws(
      () => parseLocalSearchRequestDto(body),
      FastiContractParseError,
    );
    assert.throws(() => client.searchRecords(body), FastiProtocolError);
  }
  assert.equal(calls, 0);
});

test("local Search response schema preserves exact summary evidence and bounds the page", () => {
  assert.deepEqual(parseLocalSearchResponseDto(page()), page());
  for (const response of [
    { ...page(), truncated: true },
    { ...page(), next: { ...cursor(5), extra: true } },
    { ...page(), next: { ...cursor(5), context_digest: "bad" } },
    page(Array.from({ length: 101 }, (_, index) => index + 1)),
  ])
    assert.throws(
      () => parseLocalSearchResponseDto(response),
      FastiContractParseError,
    );
});

test("local Search accepts terminal and empty continuing pages without inventing cursor authority", async () => {
  for (const response of [
    page([], null),
    page([], cursor(5)),
    page([2], cursor(5, otherDigest)),
  ]) {
    assert.deepEqual(
      await clientWith(async () => json(response)).searchRecords(request()),
      response,
    );
  }
  // A first-page digest cannot be recomputed from browser input: it also binds
  // the server-authorized workspace/profile/grant. Do not pretend to check it.
  const emptyContinuation = page([], cursor(5));
  assert.deepEqual(
    await clientWith(async () => json(emptyContinuation)).searchRecords(
      request(cursor(1)),
    ),
    emptyContinuation,
  );
});

test("local Search preserves optional cursor omission supported by the generated contract", async () => {
  const body = { query: "Dune", grains: ["film"] };
  const response = { records: [record(2)] };
  assert.deepEqual(parseLocalSearchRequestDto(body), body);
  assert.deepEqual(parseLocalSearchResponseDto(response), response);
  const client = clientWith(async (_url, init) => {
    assert.deepEqual(JSON.parse(init.body), body);
    return json(response);
  });
  assert.deepEqual(await client.searchRecords(body), response);
});

test("local Search binds ordering, grains and cursor progress to the submitted request", async (context) => {
  const invalid = [
    ["duplicate", page([2, 2], cursor(5))],
    ["descending", page([3, 2], cursor(5))],
    ["at prior cursor", page([1, 2], cursor(5))],
    ["behind prior cursor", page([0, 2], cursor(5))],
    ["next unchanged", page([], cursor(1))],
    ["next reversed", page([], cursor(0))],
    ["result past next", page([2, 6], cursor(5))],
    ["changed continuation context", page([2], cursor(5, otherDigest))],
    ["wrong grain", { records: [record(2, "series")], next: null }],
    [
      "noncanonical record",
      { records: [{ ...record(2), record_id: "not-a-record" }], next: null },
    ],
    [
      "wrong typed prefix",
      {
        records: [{ ...record(2), record_id: id(2).replace("rec_", "scr_") }],
        next: null,
      },
    ],
    [
      "terminal newline ID",
      { records: [{ ...record(2), record_id: `${id(2)}\n` }], next: null },
    ],
  ];
  for (const [name, response] of invalid)
    await context.test(name, async () => {
      await assert.rejects(
        clientWith(async () => json(response)).searchRecords(
          request(cursor(1)),
        ),
        FastiProtocolError,
      );
    });
  const mixed = { records: [record(2), record(3, "series")], next: null };
  assert.deepEqual(
    await clientWith(async () => json(mixed)).searchRecords(request(null, [])),
    mixed,
  );
});

test("local Search captures input cursor and grain set before caller mutation", async (context) => {
  for (const correctResponse of [true, false])
    await context.test(String(correctResponse), async () => {
      const body = request(cursor(1));
      const original = structuredClone(body);
      let started;
      let release;
      const ready = new Promise((resolve) => {
        started = resolve;
      });
      const client = clientWith(async (_url, init) => {
        assert.deepEqual(JSON.parse(init.body), original);
        started();
        return new Promise((resolve) => {
          release = resolve;
        });
      });
      const pending = client.searchRecords(body);
      await ready;
      body.query = "Changed";
      body.grains[0] = "series";
      body.after.last_record_id = id(10);
      body.after.context_digest = otherDigest;
      const response = correctResponse
        ? page([2, 3], cursor(5))
        : { records: [record(11, "series")], next: cursor(12, otherDigest) };
      release(json(response));
      if (correctResponse) assert.deepEqual(await pending, response);
      else await assert.rejects(pending, FastiProtocolError);
    });
});

test("local Search safe retries preserve identical serialized body after caller mutation", async (context) => {
  for (const failure of ["http", "network"])
    await context.test(failure, async () => {
      const body = request(cursor(1));
      const original = structuredClone(body);
      const bodies = [];
      const response = page([2, 3], cursor(5));
      const client = clientWith(async (_url, init) => {
        bodies.push(init.body);
        if (bodies.length === 1) {
          body.query = "Changed";
          body.grains.splice(0, 1, "series");
          body.after.last_record_id = id(10);
          body.after.context_digest = otherDigest;
          if (failure === "network")
            throw new TypeError("Synthetic read interruption");
          return new Response("Unavailable", { status: 503 });
        }
        return json(response);
      }, retries);
      assert.deepEqual(await client.searchRecords(body), response);
      assert.equal(bodies.length, 2);
      assert.equal(bodies[0], bodies[1]);
      assert.deepEqual(JSON.parse(bodies[1]), original);
    });
});

test("local Search accepts a valid dense 100-Record page above the generic 512 KiB limit", async () => {
  const response = page(Array.from({ length: 100 }, (_, index) => index + 1));
  for (const item of response.records) {
    item.title = field("Dune" + "t".repeat(4000));
    item.overview = field("v".repeat(4096));
    item.original_title = field("o".repeat(4096));
  }
  const bytes = Buffer.byteLength(JSON.stringify(response));
  assert.ok(bytes > 512 * 1024);
  assert.ok(bytes < 4 * 1024 * 1024);
  assert.deepEqual(
    await clientWith(async () => json(response)).searchRecords(request()),
    response,
  );
});

test("local Search rejects responses above 4 MiB before schema parsing without retry", async () => {
  let calls = 0;
  const client = clientWith(async () => {
    calls += 1;
    return json({ ...page(), padding: "x".repeat(4 * 1024 * 1024 + 1) });
  }, retries);
  await assert.rejects(client.searchRecords(request()), (error) => {
    assert.ok(error instanceof FastiProtocolError);
    assert.match(error.message, /bounded body size/);
    return true;
  });
  assert.equal(calls, 1);
});

test("local Search cancellation reaches the in-flight read without retry", async () => {
  const controller = new AbortController();
  let started;
  let signal;
  let calls = 0;
  const ready = new Promise((resolve) => {
    started = resolve;
  });
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
  }, retries);
  const rejected = assert.rejects(
    client.searchRecords(request(), { signal: controller.signal }),
    FastiAbortError,
  );
  await ready;
  controller.abort();
  await rejected;
  assert.equal(signal.aborted, true);
  assert.equal(calls, 1);
});
