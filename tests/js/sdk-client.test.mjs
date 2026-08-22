import assert from "node:assert/strict";
import { createServer } from "node:http";
import { test } from "node:test";

import {
  FastiAbortError,
  FastiClient,
  FastiProblemError,
  FastiProtocolError,
  FastiTimeoutError,
  FastiTransportError,
  RECEIPT_STREAM_CONTRACT,
} from "../../packages/sdk/dist/transport.js";

const ids = {
  correlation: `req_${"1".repeat(32)}`,
  observation: `obs_${"2".repeat(32)}`,
  operation: `op_${"3".repeat(32)}`,
  receiptA: `rcp_${"4".repeat(32)}`,
  receiptB: `rcp_${"5".repeat(32)}`,
};

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

test("RFC 9457 responses become typed Fasti problem errors", async () => {
  const problem = {
    type: "https://fasti.scrobble.dev/v1/problems/forbidden",
    title: "Forbidden",
    status: 403,
    detail: "the request is not authorized",
    code: "forbidden",
    capability_id: "system.health",
    safe_state: "no_mutation",
    retryability: "not_retryable",
    next_actions: [{ id: "authenticate", label: "Authenticate" }],
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
      await assert.rejects(client.health(), (error) => {
        assert.ok(error instanceof FastiProblemError);
        assert.deepEqual(error.problem, problem);
        return true;
      });
    },
  );
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

test("receipt SSE reconnects with Last-Event-ID and exact parsed events", async () => {
  let connections = 0;
  const credential = "receipt-reader-secret";
  await withServer(
    (request, response) => {
      assert.equal(request.url, RECEIPT_STREAM_CONTRACT.path);
      assert.equal(request.headers.accept, "text/event-stream");
      assert.equal(request.headers.authorization, `Bearer ${credential}`);
      connections += 1;
      response.writeHead(200, { "content-type": "text/event-stream" });
      if (connections === 1) {
        assert.equal(request.headers["last-event-id"], undefined);
        response.end(sse(ids.receiptA, receipt(ids.receiptA)));
        return;
      }
      assert.equal(request.headers["last-event-id"], ids.receiptA);
      response.end(sse(ids.receiptB, receipt(ids.receiptB)));
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
      assert.equal(second.done, false);
      assert.equal(second.value.id, ids.receiptB);
      await events.return();
      assert.equal(connections, 2);
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
        assert.deepEqual(methods, ["constructor", "health", "receiptEvents"]);
      },
    );
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

function sse(id, payload) {
  return `id: ${id}\nevent: receiptCommitted\ndata: ${JSON.stringify(payload)}\n\n`;
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
