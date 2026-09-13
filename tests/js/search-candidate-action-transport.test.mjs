import assert from "node:assert/strict";
import { test } from "node:test";
import {
  FastiAbortError,
  FastiClient,
  FastiContractParseError,
  FastiProtocolError,
  parseSearchCandidateActionRequest,
  parseSearchCandidateActionResponse,
} from "../../packages/sdk/dist/transport.js";

// Actual SDK with synthetic transport responses. Real authorization, atomic
// persistence and durable replay remain API/runtime/store verification owners.
const receiptId = "scr_01991f588e0070008000000000000001";
const operationId = "op_01991f588e0070008000000000000001";
const recordId = "rec_01991f588e0070008000000000000001";
const otherRecordId = "rec_01991f588e0070008000000000000002";
const request = (kind = "create", evidence_mode = "cached") => ({
  operation_id: operationId,
  action: kind === "create" ? { kind } : { kind, record_id: recordId },
  evidence_mode,
});
const saved = (body = request(), disposition) => ({
  outcome: "saved",
  receipt: {
    operation_id: body.operation_id,
    candidate_receipt_id: receiptId,
    provider_id: "tmdb",
    grain: "film",
    action: structuredClone(body.action),
    evidence_mode: body.evidence_mode,
    record_id: body.action.record_id ?? recordId,
    disposition:
      disposition ?? (body.action.kind === "create" ? "created" : "attached"),
    fetched_at: "2025-01-01T08:00:00Z",
    expires_at: "2025-01-01T08:02:00Z",
    initial_status: "fresh",
    committed_at: "2025-01-01T08:01:00Z",
  },
});
const json = (value) =>
  new Response(JSON.stringify(value), {
    headers: { "content-type": "application/json" },
  });
const clientWith = (fetch, options = {}) =>
  new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    credential: "synthetic-candidate-writer",
    fetch,
    ...options,
  });
const save = (client, body = request(), options = {}) =>
  client.saveSearchCandidate("tmdb", "film", receiptId, body, options);
const retries = {
  retryPolicy: { maxAttempts: 2, baseDelayMs: 0, maxDelayMs: 0 },
};
async function withCookie(cookie, callback) {
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

test("candidate action POST preserves exact intent and all compatible dispositions", async (context) => {
  for (const [kind, disposition] of [
    ["create", "created"],
    ["create", "reused"],
    ["attach", "attached"],
    ["attach", "already_attached"],
  ]) {
    await context.test(disposition, async () => {
      const body = request(kind, "refetch");
      const response = saved(body, disposition);
      const client = clientWith(async (url, init) => {
        assert.equal(
          String(url),
          `http://127.0.0.1:8420/api/v1/search/candidates/tmdb/film/${receiptId}/actions`,
        );
        assert.equal(init.method, "POST");
        assert.equal(init.credentials, "same-origin");
        assert.deepEqual(JSON.parse(init.body), body);
        const headers = new Headers(init.headers);
        assert.equal(headers.get("content-type"), "application/json");
        assert.equal(
          headers.get("authorization"),
          "Bearer synthetic-candidate-writer",
        );
        assert.equal(headers.get("x-csrf-token"), null);
        return json(response);
      });
      assert.deepEqual(await save(client, body), response);
    });
  }
});

test("candidate replay preserves old fresh timestamps and explicit cached stale evidence", async () => {
  const historical = saved();
  assert.deepEqual(
    await save(clientWith(async () => json(historical))),
    historical,
  );
  const stale = saved();
  stale.receipt.initial_status = "stale";
  stale.receipt.expires_at = null;
  assert.deepEqual(await save(clientWith(async () => json(stale))), stale);
});

test("candidate action rejects invented fields, malformed IDs and action unions before transport", () => {
  let calls = 0;
  const client = clientWith(async () => {
    calls += 1;
    return json(saved());
  });
  const invalid = [
    {},
    { ...request(), operation_id: recordId },
    { ...request(), operation_id: `${operationId}\n` },
    { ...request(), action: { kind: "create", record_id: recordId } },
    { ...request(), action: { kind: "attach" } },
    { ...request(), action: { kind: "attach", record_id: receiptId } },
    { ...request(), action: { kind: "delete" } },
    { ...request(), evidence_mode: "automatic" },
    ...["metadata", "fields", "provenance", "actor", "terms_revision"].map(
      (key) => ({ ...request(), [key]: {} }),
    ),
  ];
  for (const body of invalid) {
    assert.throws(
      () => parseSearchCandidateActionRequest(body),
      FastiContractParseError,
    );
    assert.throws(() => save(client, body), FastiProtocolError);
  }
  for (const [provider, grain, receipt] of [
    ["tmdb/other", "film", receiptId],
    ["tmdb\n", "film", receiptId],
    ["tmdb", "film/other", receiptId],
    ["tmdb", "film", recordId],
    ["tmdb", "film", `${receiptId}\n`],
  ]) {
    assert.throws(
      () => client.saveSearchCandidate(provider, grain, receipt, request()),
      TypeError,
    );
  }
  assert.equal(calls, 0);
});

test("candidate action response has a strict public receipt and tagged outcome", () => {
  for (const key of [
    "actor",
    "provenance",
    "claim_id",
    "provider_authority_fingerprint",
  ]) {
    const response = saved();
    response.receipt[key] = "must not be projected";
    assert.throws(
      () => parseSearchCandidateActionResponse(response),
      FastiContractParseError,
    );
  }
  for (const mutate of [
    (r) => {
      r.receipt.operation_id = recordId;
    },
    (r) => {
      r.receipt.record_id = receiptId;
    },
    (r) => {
      r.receipt.fetched_at = "not a date";
    },
    (r) => {
      r.receipt.initial_status = "expired";
    },
    (r) => {
      r.receipt.disposition = "deleted";
    },
    (r) => {
      r.problem_code = "provider_unavailable";
    },
    (r) => {
      r.outcome = "unavailable";
      r.problem_code = "provider_unavailable";
    },
  ]) {
    const response = saved();
    mutate(response);
    assert.throws(
      () => parseSearchCandidateActionResponse(response),
      FastiContractParseError,
    );
  }
});

test("candidate action binds every saved receipt to immutable submitted intent", async (context) => {
  const cases = [
    [
      "operation",
      (r) => {
        r.operation_id = "op_01991f588e0070008000000000000002";
      },
    ],
    [
      "receipt",
      (r) => {
        r.candidate_receipt_id = "scr_01991f588e0070008000000000000002";
      },
    ],
    [
      "provider",
      (r) => {
        r.provider_id = "google-books";
      },
    ],
    [
      "grain",
      (r) => {
        r.grain = "series";
      },
    ],
    [
      "action",
      (r) => {
        r.action = { kind: "create" };
      },
    ],
    [
      "evidence",
      (r) => {
        r.evidence_mode = "refetch";
      },
    ],
    [
      "target",
      (r) => {
        r.action.record_id = otherRecordId;
      },
    ],
    [
      "result",
      (r) => {
        r.record_id = otherRecordId;
      },
    ],
    [
      "disposition",
      (r) => {
        r.disposition = "created";
      },
    ],
  ];
  for (const [name, mutate] of cases)
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
  const createAsAttach = saved();
  createAsAttach.receipt.disposition = "attached";
  await assert.rejects(
    save(clientWith(async () => json(createAsAttach))),
    FastiProtocolError,
  );
});

test("candidate action rejects impossible historical status combinations without recomputing age", async (context) => {
  for (const [name, mode, status, expiry] of [
    ["refetch fresh without expiry", "refetch", "fresh", null],
    ["refetch stale with expiry", "refetch", "stale", "2025-01-01T08:02:00Z"],
    ["fresh without expiry", "cached", "fresh", null],
    ["fresh with omitted expiry", "cached", "fresh", undefined],
    ["refetch with omitted expiry", "refetch", "fresh", undefined],
    ["stale with expiry", "cached", "stale", "2025-01-01T08:02:00Z"],
  ])
    await context.test(name, async () => {
      const body = request("create", mode);
      const response = saved(body);
      response.receipt.initial_status = status;
      response.receipt.expires_at = expiry;
      await assert.rejects(
        save(
          clientWith(async () => json(response)),
          body,
        ),
        FastiProtocolError,
      );
    });
});

test("candidate action accepts historical zero-freshness refetch receipt without renewing timestamps", async () => {
  const body = request("create", "refetch");
  const response = saved(body);
  response.receipt.initial_status = "stale";
  response.receipt.expires_at = null;
  assert.deepEqual(
    await save(
      clientWith(async () => json(response)),
      body,
    ),
    response,
  );
});

test("caller mutation during response wait cannot rewrite candidate action intent", async (context) => {
  for (const responseMatchesOriginal of [true, false])
    await context.test(String(responseMatchesOriginal), async () => {
      const body = request("attach");
      const original = structuredClone(body);
      let release;
      let started;
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
      const pending = save(client, body);
      await ready;
      body.operation_id = "op_01991f588e0070008000000000000002";
      body.action.record_id = otherRecordId;
      body.evidence_mode = "refetch";
      release(json(saved(responseMatchesOriginal ? original : body)));
      if (responseMatchesOriginal)
        assert.deepEqual(await pending, saved(original));
      else await assert.rejects(pending, FastiProtocolError);
    });
});

test("candidate retries send identical serialized operation and nested action despite caller mutation", async (context) => {
  for (const failure of ["http", "network"])
    await context.test(failure, async () => {
      const body = request("attach");
      const original = structuredClone(body);
      const bodies = [];
      const client = clientWith(async (_url, init) => {
        bodies.push(init.body);
        if (bodies.length === 1) {
          body.operation_id = "op_01991f588e0070008000000000000002";
          body.action.kind = "create";
          delete body.action.record_id;
          body.evidence_mode = "refetch";
          if (failure === "network")
            throw new TypeError("synthetic transport loss");
          return new Response("Unavailable", { status: 503 });
        }
        return json(saved(original));
      }, retries);
      assert.deepEqual(await save(client, body), saved(original));
      assert.equal(bodies.length, 2);
      assert.equal(bodies[0], bodies[1]);
      assert.deepEqual(JSON.parse(bodies[1]), original);
    });
});

test("browser cached candidate save requires CSRF mutation proof", async () => {
  const csrf = "a".repeat(64);
  await withCookie(`__Host-fasti_csrf=${csrf}`, async () => {
    const client = clientWith(
      async (_url, init) => {
        const headers = new Headers(init.headers);
        assert.equal(headers.get("x-csrf-token"), csrf);
        assert.equal(headers.get("authorization"), null);
        assert.equal(init.credentials, "same-origin");
        return json(saved());
      },
      { credential: undefined },
    );
    assert.equal((await save(client)).outcome, "saved");
  });
  for (const cookie of [
    "",
    "__Host-fasti_csrf=bad",
    `__Host-fasti_csrf=${csrf}; __Host-fasti_csrf=${csrf}`,
  ]) {
    await withCookie(cookie, async () => {
      let calls = 0;
      const client = clientWith(
        async () => {
          calls += 1;
          return json(saved());
        },
        { credential: undefined },
      );
      await assert.rejects(save(client), FastiProtocolError);
      assert.equal(calls, 0);
    });
  }
});

test("unavailable refetch does not retry as an implicit cached save", async () => {
  let calls = 0;
  const response = {
    outcome: "unavailable",
    problem_code: "provider_unavailable",
  };
  const client = clientWith(async (_url, init) => {
    calls += 1;
    assert.equal(JSON.parse(init.body).evidence_mode, "refetch");
    return json(response);
  }, retries);
  assert.deepEqual(await save(client, request("create", "refetch")), response);
  assert.equal(calls, 1);
});

test("candidate action cancellation reaches transport and does not retry", async () => {
  const controller = new AbortController();
  let started;
  let calls = 0;
  let signal;
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
    save(client, request(), { signal: controller.signal }),
    FastiAbortError,
  );
  await ready;
  controller.abort();
  await rejected;
  assert.equal(signal.aborted, true);
  assert.equal(calls, 1);
});
