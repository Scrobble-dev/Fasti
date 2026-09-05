import assert from "node:assert/strict";
import { test } from "node:test";
import {
  FastiAbortError,
  FastiClient,
  FastiContractParseError,
  FastiProtocolError,
  parseSearchCandidateDetailsQueryParameters,
  parseSearchCandidateDetailsResponse,
} from "../../packages/sdk/dist/transport.js";

// Actual SDK with synthetic transport responses. Authorization, provider
// refetch, receipt persistence and expiry are verified by API/runtime/store.
const receiptId = "scr_01991f588e0070008000000000000001";
const otherReceiptId = "scr_01991f588e0070008000000000000002";
const candidate = () => ({
  provider: "tmdb",
  provider_id: "438631",
  kind: "movie",
  title: "Dune",
  original_title: null,
  release_year: 2021,
  authors: [],
  image_url: null,
  overview: "Original normalized Search evidence.",
});
const snapshot = () => ({
  receipt: {
    candidate_receipt_id: receiptId,
    grain: "film",
    candidate: candidate(),
  },
  lifetime: {
    created_at: "2026-09-05T08:00:00Z",
    fresh_until: "2026-09-05T08:02:00Z",
    stale_until: "2026-09-05T08:10:00Z",
    expires_at: "2026-09-06T08:00:00Z",
  },
  locale: "en-ie",
});
const original = () => ({ outcome: "snapshot", snapshot: snapshot() });
const refetched = () => ({
  outcome: "refetched",
  snapshot: snapshot(),
  details: {
    ...candidate(),
    overview: "Separately refetched detail evidence.",
  },
  locale: "en-ie",
});
const unavailable = () => ({
  outcome: "unavailable",
  snapshot: snapshot(),
  problem_code: "provider_unavailable",
});
const json = (value) =>
  new Response(JSON.stringify(value), {
    headers: { "content-type": "application/json" },
  });
const clientWith = (fetch, options = {}) =>
  new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    credential: "synthetic-detail-reader",
    fetch,
    ...options,
  });
const read = (client, query = { offline: true }, options = {}) =>
  client.readSearchCandidate("tmdb", "film", receiptId, query, options);

test("candidate details GET encodes explicit mode without a body or CSRF mutation header", async (context) => {
  for (const offline of [true, false]) {
    await context.test(String(offline), async () => {
      const response = offline ? original() : refetched();
      const client = clientWith(async (url, init) => {
        assert.equal(
          String(url),
          `http://127.0.0.1:8420/api/v1/search/candidates/tmdb/film/${receiptId}?offline=${offline}`,
        );
        assert.equal(init.method, "GET");
        assert.equal(init.body, undefined);
        assert.equal(init.credentials, "same-origin");
        const headers = new Headers(init.headers);
        assert.equal(
          headers.get("authorization"),
          "Bearer synthetic-detail-reader",
        );
        assert.equal(headers.get("x-csrf-token"), null);
        assert.equal(headers.get("content-type"), null);
        return json(response);
      });
      assert.deepEqual(await read(client, { offline }), response);
    });
  }
});

test("browser candidate details remain readable without a CSRF cookie", async () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, "document");
  Object.defineProperty(globalThis, "document", {
    configurable: true,
    value: { cookie: "" },
  });
  try {
    let calls = 0;
    const client = clientWith(
      async (_url, init) => {
        calls += 1;
        const headers = new Headers(init.headers);
        assert.equal(headers.get("authorization"), null);
        assert.equal(headers.get("x-csrf-token"), null);
        assert.equal(init.credentials, "same-origin");
        return json(original());
      },
      { credential: undefined },
    );
    assert.equal((await read(client)).outcome, "snapshot");
    assert.equal(calls, 1);
  } finally {
    if (previous) Object.defineProperty(globalThis, "document", previous);
    else delete globalThis.document;
  }
});

test("candidate details parsers preserve all four distinct outcomes and snapshot lifetime", () => {
  for (const offline of [true, false]) {
    assert.deepEqual(parseSearchCandidateDetailsQueryParameters({ offline }), {
      offline,
    });
  }
  for (const response of [
    { outcome: "missing" },
    original(),
    refetched(),
    unavailable(),
  ]) {
    assert.deepEqual(parseSearchCandidateDetailsResponse(response), response);
  }
  const response = parseSearchCandidateDetailsResponse(refetched());
  assert.notEqual(
    response.details.overview,
    response.snapshot.receipt.candidate.overview,
  );
  assert.deepEqual(response.snapshot.lifetime, snapshot().lifetime);
});

test("candidate details reject missing mode, wrong types and forbidden query inputs before transport", () => {
  let calls = 0;
  const client = clientWith(async () => {
    calls += 1;
    return json(original());
  });
  for (const query of [
    {},
    { offline: "true" },
    { offline: null },
    { offline: true, locale: "fr" },
    { offline: true, provider_id: "438631" },
    { offline: true, terms_revision: "caller-policy" },
    { offline: true, query: "Dune" },
  ]) {
    assert.throws(() => read(client, query), FastiProtocolError);
  }
  assert.equal(calls, 0);
});

test("candidate details reject unsafe locator components before transport", () => {
  let calls = 0;
  const client = clientWith(async () => {
    calls += 1;
    return json(original());
  });
  for (const [provider, grain, id] of [
    ["tmdb/other", "film", receiptId],
    ["tmdb?offline=false", "film", receiptId],
    ["tmdb\n", "film", receiptId],
    ["tmdb", "film/other", receiptId],
    ["tmdb", "film", receiptId.replace("scr_", "rec_")],
    ["tmdb", "film", `${receiptId}\n`],
    ["tmdb", "film", `${receiptId}?offline=false`],
    ["tmdb", "film", "not-a-receipt"],
  ]) {
    assert.throws(() =>
      client.readSearchCandidate(provider, grain, id, { offline: true }),
    );
  }
  assert.equal(calls, 0);
});

test("candidate details bind every returned snapshot to the exact requested locator", async (context) => {
  for (const [name, mutate] of [
    [
      "receipt",
      (value) => {
        value.snapshot.receipt.candidate_receipt_id = otherReceiptId;
      },
    ],
    [
      "grain",
      (value) => {
        value.snapshot.receipt.grain = "series";
      },
    ],
    [
      "provider",
      (value) => {
        value.snapshot.receipt.candidate.provider = "google-books";
      },
    ],
  ]) {
    await context.test(name, async () => {
      for (const response of [original(), refetched(), unavailable()]) {
        mutate(response);
        const client = clientWith(async () => json(response));
        await assert.rejects(
          read(client, { offline: response.outcome === "snapshot" }),
          FastiProtocolError,
        );
      }
    });
  }
});

test("refetched candidate details must preserve the original provider coordinate", async (context) => {
  for (const [field, value] of [
    ["provider", "google-books"],
    ["kind", "tv"],
    ["provider_id", "438632"],
  ]) {
    await context.test(field, async () => {
      const response = refetched();
      response.details[field] = value;
      const client = clientWith(async () => json(response));
      await assert.rejects(
        read(client, { offline: false }),
        FastiProtocolError,
      );
    });
  }
});

test("candidate details reject outcomes that contradict explicit offline mode", async () => {
  for (const [offline, response] of [
    [true, refetched()],
    [true, unavailable()],
    [false, original()],
  ]) {
    const client = clientWith(async () => json(response));
    await assert.rejects(read(client, { offline }), FastiProtocolError);
  }
  for (const offline of [true, false]) {
    const client = clientWith(async () => json({ outcome: "missing" }));
    assert.deepEqual(await read(client, { offline }), { outcome: "missing" });
  }
});

test("candidate details response mode binds to the submitted boolean despite caller mutation", async (context) => {
  for (const offline of [true, false]) {
    for (const correct of [true, false]) {
      await context.test(`offline=${offline}, correct=${correct}`, async () => {
        const query = { offline };
        const response = (correct ? offline : !offline)
          ? original()
          : refetched();
        let sentMode;
        const client = clientWith(async (url) => {
          sentMode = new URL(url).searchParams.get("offline");
          return json(response);
        });
        const pending = read(client, query);
        query.offline = !offline;
        if (correct) assert.deepEqual(await pending, response);
        else await assert.rejects(pending, FastiProtocolError);
        assert.equal(sentMode, String(offline));
      });
    }
  }
});

test("candidate details union rejects malformed and mixed evidence shapes", () => {
  for (const mutate of [
    (value) => {
      delete value.outcome;
    },
    (value) => {
      value.outcome = "unknown";
    },
    (value) => {
      value.problem_code = "provider_unavailable";
    },
    (value) => {
      value.partition = {};
    },
    (value) => {
      value.snapshot.extra = true;
    },
    (value) => {
      value.snapshot.receipt.candidate.extra = true;
    },
    (value) => {
      value.snapshot.receipt.candidate_receipt_id =
        "rec_01991f588e0070008000000000000001";
    },
    (value) => {
      value.snapshot.lifetime.created_at = "not-a-date";
    },
    (value) => {
      value.snapshot.lifetime.expires_at = "2026-02-30T08:00:00Z";
    },
  ]) {
    const response = refetched();
    mutate(response);
    assert.throws(
      () => parseSearchCandidateDetailsResponse(response),
      FastiContractParseError,
    );
  }
  assert.throws(
    () =>
      parseSearchCandidateDetailsResponse({
        outcome: "missing",
        snapshot: snapshot(),
      }),
    FastiContractParseError,
  );
});

test("candidate details GET uses existing bounded safe retry", async (context) => {
  for (const mode of ["http", "network"]) {
    await context.test(mode, async () => {
      let calls = 0;
      const client = clientWith(
        async () => {
          calls += 1;
          if (calls === 1) {
            if (mode === "network")
              throw new TypeError("Synthetic interrupted read");
            return new Response(null, { status: 503 });
          }
          return json(original());
        },
        { retryPolicy: { maxAttempts: 2, baseDelayMs: 0, maxDelayMs: 0 } },
      );
      assert.deepEqual(await read(client), original());
      assert.equal(calls, 2);
    });
  }
});

test("candidate details cancellation reaches the in-flight GET", async () => {
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
    { offline: false },
    { signal: controller.signal },
  );
  const rejected = assert.rejects(pending, FastiAbortError);
  await ready;
  controller.abort();
  await rejected;
  assert.equal(signal.aborted, true);
  assert.equal(calls, 1);
});

test("candidate details retain the generic response byte cap", async () => {
  const client = clientWith(async () =>
    json({ ...original(), padding: "x".repeat(513 * 1024) }),
  );
  await assert.rejects(read(client), (error) => {
    assert.ok(error instanceof FastiProtocolError);
    assert.match(error.message, /bounded body size/);
    return true;
  });
});
