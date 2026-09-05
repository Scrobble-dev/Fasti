import assert from "node:assert/strict";
import { test } from "node:test";

import {
  FastiAbortError,
  FastiClient,
  FastiContractParseError,
  FastiProtocolError,
  FastiTransportError,
  parseSearchProviderPageRequest,
  parseSearchProviderPageResponse,
} from "../../packages/sdk/dist/transport.js";

// Public SDK/transport fixtures only. Real authorization, provider execution,
// receipt persistence, and expiry admission remain API/runtime/store gates.
const request = () => ({
  query: "Dune & moon?",
  page: 1,
  locale: "en-ie",
  region: null,
  grains: ["film"],
  offline: false,
});

const candidate = (index = 1) => ({
  candidate_receipt_id: `scr_01991f588e0070008000${index.toString(16).padStart(12, "0")}`,
  grain: "film",
  candidate: {
    provider: "tmdb",
    provider_id: String(index),
    grain: "film",
    kind: "movie",
    title: "Dune",
    original_title: null,
    release_year: 2021,
    authors: [],
    image_url: null,
    overview: "A provider Search candidate, not a saved Record.",
  },
});

const pageResponse = () => ({
  outcome: "page",
  provider_id: "tmdb",
  page: 1,
  candidates: [candidate()],
  next_page: 2,
  cache_state: "fresh",
  lifetime: {
    created_at: "2026-09-05T08:00:00Z",
    fresh_until: "2026-09-05T08:02:00Z",
    stale_until: "2026-09-05T08:10:00Z",
    expires_at: "2026-09-06T08:00:00Z",
  },
  upstream_problem: null,
});

const liveResponse = () => ({
  outcome: "live",
  provider_id: "tmdb",
  page: 1,
  candidates: [candidate().candidate],
  next_page: 2,
});

const observedResponse = () => {
  const value = pageResponse();
  value.cache_state = "observed";
  value.lifetime.fresh_until = value.lifetime.created_at;
  value.lifetime.stale_until = value.lifetime.created_at;
  return value;
};

const json = (value) =>
  new Response(JSON.stringify(value), {
    headers: { "content-type": "application/json" },
  });

const clientWith = (fetch, extra = {}) =>
  new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    credential: "synthetic-scoped-credential",
    fetch,
    ...extra,
  });

async function withDocumentCookie(cookie, callback) {
  const previous = Object.getOwnPropertyDescriptor(globalThis, "document");
  Object.defineProperty(globalThis, "document", {
    configurable: true,
    value: { cookie },
  });
  try {
    await callback();
  } finally {
    if (previous) Object.defineProperty(globalThis, "document", previous);
    else delete globalThis.document;
  }
}

test("provider Search posts bounded query data in the body, not the URL", async () => {
  const body = request();
  const expected = pageResponse();
  let calls = 0;
  const client = clientWith(async (url, init) => {
    calls += 1;
    assert.equal(
      String(url),
      "http://127.0.0.1:8420/api/v1/search/providers/tmdb",
    );
    assert.equal(init.method, "POST");
    assert.deepEqual(JSON.parse(init.body), body);
    assert.equal(init.credentials, "same-origin");
    const headers = new Headers(init.headers);
    assert.equal(headers.get("content-type"), "application/json");
    assert.equal(
      headers.get("authorization"),
      "Bearer synthetic-scoped-credential",
    );
    assert.equal(headers.get("x-csrf-token"), null);
    return json(expected);
  });
  assert.deepEqual(await client.searchProviderPage("tmdb", body), expected);
  assert.equal(calls, 1);
});

test("provider Search parsers preserve fresh, stale, unavailable and empty pages", () => {
  assert.deepEqual(parseSearchProviderPageRequest(request()), request());
  const stale = {
    ...pageResponse(),
    cache_state: "stale_on_error",
    upstream_problem: "provider_unavailable",
  };
  const empty = { ...pageResponse(), candidates: [] };
  const unavailable = {
    outcome: "unavailable",
    provider_id: "tmdb",
    problem_code: "provider_unavailable",
  };
  for (const value of [pageResponse(), stale, empty, unavailable]) {
    assert.deepEqual(parseSearchProviderPageResponse(value), value);
  }
  assert.equal(parseSearchProviderPageResponse(empty).next_page, 2);
});

test("provider Search preserves populated, empty and bounded live-only results without receipts", async () => {
  for (const candidates of [
    [candidate().candidate],
    [],
    Array.from({ length: 100 }, (_, index) => candidate(index + 1).candidate),
  ]) {
    const expected = { ...liveResponse(), candidates };
    assert.deepEqual(parseSearchProviderPageResponse(expected), expected);
    let calls = 0;
    const client = clientWith(async () => {
      calls += 1;
      return json(expected);
    });
    const actual = await client.searchProviderPage("tmdb", request());
    assert.deepEqual(actual, expected);
    assert.equal(
      actual.next_page,
      2,
      "a filtered live page retains continuation",
    );
    assert.deepEqual(Object.keys(actual).sort(), [
      "candidates",
      "next_page",
      "outcome",
      "page",
      "provider_id",
    ]);
    for (const entry of actual.candidates) {
      assert.equal(Object.hasOwn(entry, "candidate_receipt_id"), false);
      assert.equal(Object.hasOwn(entry, "lifetime"), false);
      assert.equal(entry.provider, "tmdb");
    }
    assert.equal(calls, 1);
  }
});

test("provider Search preserves newly observed evidence without relabeling it fresh", async () => {
  for (const candidates of [[candidate()], []]) {
    const expected = { ...observedResponse(), candidates };
    assert.deepEqual(parseSearchProviderPageResponse(expected), expected);
    const client = clientWith(async () => json(expected));
    const actual = await client.searchProviderPage("tmdb", request());
    assert.deepEqual(actual, expected);
    assert.equal(actual.cache_state, "observed");
    assert.equal(actual.lifetime.fresh_until, actual.lifetime.created_at);
    assert.equal(actual.next_page, 2);
  }
});

test("provider Search binds live results to the submitted provider and page", async (context) => {
  for (const [name, mutate] of [
    [
      "outer provider",
      (value) => {
        value.provider_id = "google-books";
      },
    ],
    [
      "page",
      (value) => {
        value.page = 2;
      },
    ],
    [
      "candidate provider",
      (value) => {
        value.candidates[0].provider = "google-books";
      },
    ],
    [
      "candidate grain",
      (value) => {
        value.candidates[0].grain = "series";
      },
    ],
  ]) {
    await context.test(name, async () => {
      const value = liveResponse();
      mutate(value);
      const client = clientWith(async () => json(value));
      await assert.rejects(
        client.searchProviderPage("tmdb", request()),
        FastiProtocolError,
      );
    });
  }
});

test("provider Search rejects live and observed results for offline requests but permits cached pages", async () => {
  const offline = { ...request(), offline: true };
  for (const value of [liveResponse(), observedResponse()]) {
    const client = clientWith(async () => json(value));
    await assert.rejects(
      client.searchProviderPage("tmdb", offline),
      FastiProtocolError,
    );
  }
  for (const value of [
    pageResponse(),
    {
      ...pageResponse(),
      cache_state: "stale_on_error",
      upstream_problem: "provider_unavailable",
    },
  ]) {
    const client = clientWith(async () => json(value));
    assert.deepEqual(await client.searchProviderPage("tmdb", offline), value);
  }
});

test("provider Search live and observed validation uses submitted offline mode despite caller mutation", async (context) => {
  for (const factory of [liveResponse, observedResponse]) {
    for (const submittedOffline of [false, true]) {
      await context.test(
        `${factory.name} offline ${submittedOffline}`,
        async () => {
          const body = { ...request(), offline: submittedOffline };
          const expected = factory();
          let sentOffline;
          const client = clientWith(async (_url, init) => {
            sentOffline = JSON.parse(init.body).offline;
            return json(expected);
          });
          const pending = client.searchProviderPage("tmdb", body);
          body.offline = !submittedOffline;
          if (submittedOffline) {
            await assert.rejects(pending, FastiProtocolError);
          } else {
            assert.deepEqual(await pending, expected);
          }
          assert.equal(sentOffline, submittedOffline);
        },
      );
    }
  }
});

test("provider Search never presents an upstream failure as newly observed evidence", async () => {
  const value = {
    ...observedResponse(),
    upstream_problem: "provider_unavailable",
  };
  const client = clientWith(async () => json(value));
  await assert.rejects(
    client.searchProviderPage("tmdb", request()),
    FastiProtocolError,
  );
});

test("provider Search accepts observed evidence with omitted optional upstream problem", async () => {
  const expected = observedResponse();
  delete expected.upstream_problem;
  assert.deepEqual(parseSearchProviderPageResponse(expected), expected);
  const client = clientWith(async () => json(expected));
  assert.deepEqual(
    await client.searchProviderPage("tmdb", request()),
    expected,
  );
});

test("provider Search live union rejects receipt wrappers, persistence fields and oversized candidates", () => {
  const mutations = [
    (value) => {
      value.lifetime = pageResponse().lifetime;
    },
    (value) => {
      value.cache_state = "observed";
    },
    (value) => {
      value.candidate_receipt_id = candidate().candidate_receipt_id;
    },
    (value) => {
      value.upstream_problem = null;
    },
    (value) => {
      value.response_cache_policy = { reuse: "no_store" };
    },
    (value) => {
      value.raw_headers = { "cache-control": "no-store" };
    },
    (value) => {
      value.candidates = [candidate()];
    },
    (value) => {
      value.candidates[0].candidate_receipt_id =
        candidate().candidate_receipt_id;
    },
    (value) => {
      value.candidates[0].lifetime = pageResponse().lifetime;
    },
    (value) => {
      value.candidates[0].response_cache_policy = { reuse: "no_store" };
    },
    (value) => {
      value.candidates[0].title = "t".repeat(513);
    },
    (value) => {
      value.candidates[0].overview = "o".repeat(4097);
    },
    (value) => {
      value.candidates = Array.from(
        { length: 101 },
        (_, index) => candidate(index + 1).candidate,
      );
    },
  ];
  for (const mutate of mutations) {
    const value = liveResponse();
    mutate(value);
    assert.throws(
      () => parseSearchProviderPageResponse(value),
      FastiContractParseError,
    );
  }
});

test("provider Search rejects malformed outgoing bodies before any fetch", async (context) => {
  const cases = [
    [
      "empty query",
      (body) => {
        body.query = "";
      },
    ],
    [
      "oversized query",
      (body) => {
        body.query = "x".repeat(257);
      },
    ],
    [
      "zero page",
      (body) => {
        body.page = 0;
      },
    ],
    [
      "fractional page",
      (body) => {
        body.page = 1.5;
      },
    ],
    [
      "too many grains",
      (body) => {
        body.grains = Array(33).fill("film");
      },
    ],
    [
      "missing mode",
      (body) => {
        delete body.offline;
      },
    ],
    [
      "wrong mode type",
      (body) => {
        body.offline = "true";
      },
    ],
    [
      "caller policy revision",
      (body) => {
        body.terms_revision = "caller-policy";
      },
    ],
    [
      "caller authorization",
      (body) => {
        body.access = {};
      },
    ],
  ];
  for (const [name, mutate] of cases) {
    await context.test(name, async () => {
      let calls = 0;
      const client = clientWith(async () => {
        calls += 1;
        return json(pageResponse());
      });
      const body = request();
      mutate(body);
      assert.throws(
        () => client.searchProviderPage("tmdb", body),
        FastiProtocolError,
      );
      assert.equal(calls, 0);
    });
  }
});

test("provider Search rejects ambiguous provider paths before transport", () => {
  let calls = 0;
  const client = clientWith(async () => {
    calls += 1;
    return json(pageResponse());
  });
  for (const provider of [
    "",
    "tmdb/other",
    "tmdb?query=other",
    "tmdb#fragment",
    " tmdb",
    "tmdb%2fother",
    "x".repeat(129),
  ]) {
    assert.throws(
      () => client.searchProviderPage(provider, request()),
      /providerId does not match/,
    );
  }
  assert.equal(calls, 0);
});

test("provider Search is never retried on transient HTTP or network failure", async (context) => {
  for (const mode of ["http", "network"]) {
    await context.test(mode, async () => {
      let calls = 0;
      const client = clientWith(
        async () => {
          calls += 1;
          if (mode === "network")
            throw new TypeError("Synthetic transport interruption");
          return new Response(null, { status: 503 });
        },
        { retryPolicy: { maxAttempts: 3, baseDelayMs: 0, maxDelayMs: 0 } },
      );
      await assert.rejects(
        client.searchProviderPage("tmdb", request()),
        FastiTransportError,
      );
      assert.equal(calls, 1);
    });
  }
});

test("browser provider Search sends exact CSRF even for an offline cache request", async () => {
  const csrf = "a".repeat(64);
  await withDocumentCookie(
    `unrelated=1; __Host-fasti_csrf=${csrf}`,
    async () => {
      let calls = 0;
      const client = clientWith(
        async (_url, init) => {
          calls += 1;
          const headers = new Headers(init.headers);
          assert.equal(headers.get("x-csrf-token"), csrf);
          assert.equal(headers.get("authorization"), null);
          assert.equal(init.credentials, "same-origin");
          assert.equal(init.method, "POST");
          assert.equal(JSON.parse(init.body).offline, true);
          return json(pageResponse());
        },
        { credential: undefined },
      );
      await client.searchProviderPage("tmdb", { ...request(), offline: true });
      assert.equal(calls, 1);
    },
  );
});

test("browser provider Search rejects missing, malformed and duplicate CSRF before fetch", async (context) => {
  for (const cookie of [
    "",
    "__Host-fasti_csrf=bad",
    `__Host-fasti_csrf=${"a".repeat(64)}; __Host-fasti_csrf=${"a".repeat(64)}`,
  ]) {
    await context.test(
      cookie ? "invalid or ambiguous cookie" : "missing cookie",
      async () => {
        await withDocumentCookie(cookie, async () => {
          let calls = 0;
          const client = clientWith(
            async () => {
              calls += 1;
              return json(pageResponse());
            },
            { credential: undefined },
          );
          await assert.rejects(
            client.searchProviderPage("tmdb", request()),
            FastiProtocolError,
          );
          assert.equal(calls, 0);
        });
      },
    );
  }
});

test("provider Search cancellation reaches the in-flight transport without retry", async () => {
  const controller = new AbortController();
  let started;
  const ready = new Promise((resolve) => {
    started = resolve;
  });
  let observedSignal;
  let calls = 0;
  const client = clientWith(async (_url, init) => {
    calls += 1;
    observedSignal = init.signal;
    return new Promise((_resolve, reject) => {
      init.signal.addEventListener(
        "abort",
        () => reject(new DOMException("Aborted", "AbortError")),
        { once: true },
      );
      started();
    });
  });
  const pending = client.searchProviderPage("tmdb", request(), {
    signal: controller.signal,
  });
  const rejected = assert.rejects(pending, FastiAbortError);
  await ready;
  controller.abort();
  await rejected;
  assert.equal(observedSignal.aborted, true);
  assert.equal(calls, 1);
});

test("provider Search binds page responses to the requested source and page", async (context) => {
  for (const [name, mutate] of [
    [
      "outer provider",
      (value) => {
        value.provider_id = "google-books";
      },
    ],
    [
      "page",
      (value) => {
        value.page = 2;
      },
    ],
    [
      "candidate provider",
      (value) => {
        value.candidates[0].candidate.provider = "google-books";
      },
    ],
    [
      "receipt grain",
      (value) => {
        value.candidates[0].grain = "series";
      },
    ],
    [
      "candidate grain",
      (value) => {
        value.candidates[0].candidate.grain = "series";
      },
    ],
  ]) {
    await context.test(name, async () => {
      const value = pageResponse();
      mutate(value);
      const client = clientWith(async () => json(value));
      await assert.rejects(
        client.searchProviderPage("tmdb", request()),
        FastiProtocolError,
      );
    });
  }
  const client = clientWith(async () =>
    json({
      outcome: "unavailable",
      provider_id: "google-books",
      problem_code: "provider_unavailable",
    }),
  );
  await assert.rejects(
    client.searchProviderPage("tmdb", request()),
    FastiProtocolError,
  );
});

test("provider Search response binding uses the submitted page despite caller mutation", async (context) => {
  for (const responsePage of [1, 2]) {
    await context.test(`response page ${responsePage}`, async () => {
      const body = request();
      const response = { ...pageResponse(), page: responsePage };
      let sentPage;
      const client = clientWith(async (_url, init) => {
        sentPage = JSON.parse(init.body).page;
        return json(response);
      });
      const pending = client.searchProviderPage("tmdb", body);
      body.page = 2;
      if (responsePage === 1) {
        assert.deepEqual(await pending, response);
      } else {
        await assert.rejects(pending, FastiProtocolError);
      }
      assert.equal(sentPage, 1);
    });
  }
});

test("provider Search response union rejects ambiguous, extra and malformed evidence", () => {
  for (const mutate of [
    (value) => {
      value.outcome = "unknown";
    },
    (value) => {
      delete value.outcome;
    },
    (value) => {
      value.problem_code = "provider_unavailable";
    },
    (value) => {
      value.partition = { grant_id: "not-public" };
    },
    (value) => {
      value.candidates[0].candidate.extra = true;
    },
    (value) => {
      value.candidates[0].candidate_receipt_id =
        value.candidates[0].candidate_receipt_id.replace("scr_", "rec_");
    },
    (value) => {
      value.cache_state = "cached";
    },
    (value) => {
      value.candidates = Array.from({ length: 101 }, (_, index) =>
        candidate(index + 1),
      );
    },
  ]) {
    const value = pageResponse();
    mutate(value);
    assert.throws(
      () => parseSearchProviderPageResponse(value),
      FastiContractParseError,
    );
  }
  assert.throws(
    () =>
      parseSearchProviderPageResponse({
        outcome: "unavailable",
        provider_id: "tmdb",
        problem_code: "provider_unavailable",
        candidates: [],
      }),
    FastiContractParseError,
  );
});

test("provider Search accepts a valid bounded page above the generic 512 KiB response limit", async () => {
  const value = pageResponse();
  value.candidates = Array.from({ length: 100 }, (_, index) => {
    const entry = candidate(index + 1);
    entry.candidate.title = "t".repeat(512);
    entry.candidate.original_title = "o".repeat(512);
    entry.candidate.overview = "v".repeat(4096);
    entry.candidate.authors = Array(10).fill("a".repeat(128));
    return entry;
  });
  assert.ok(Buffer.byteLength(JSON.stringify(value)) > 512 * 1024);
  const client = clientWith(async () => json(value));
  const result = await client.searchProviderPage("tmdb", request());
  assert.equal(result.candidates.length, 100);
  assert.deepEqual(result, value);
});

test("provider Search rejects a response beyond its operation-specific byte limit before schema parsing", async () => {
  const limit = 100 * (64 * 1024 + 1024) + 8 * 1024;
  let calls = 0;
  const client = clientWith(async () => {
    calls += 1;
    return json({ ...pageResponse(), padding: "x".repeat(limit + 1) });
  });
  await assert.rejects(
    client.searchProviderPage("tmdb", request()),
    (error) => {
      assert.ok(error instanceof FastiProtocolError);
      assert.match(error.message, /bounded body size/);
      return true;
    },
  );
  assert.equal(calls, 1);
});
