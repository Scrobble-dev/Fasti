import assert from "node:assert/strict";
import { execFile, spawn } from "node:child_process";
import { createServer } from "node:http";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { test } from "node:test";

import {
  FastiAbortError,
  FastiClient,
  FastiProblemError,
  FastiProtocolError,
  FastiTimeoutError,
  FastiTransportError,
  parseAcceptObservationRequest,
  parseHealthResponse,
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
  correlation: `req_${"1".repeat(32)}`,
  observation: `obs_${"2".repeat(32)}`,
  operation: `op_${"3".repeat(32)}`,
  receiptA: `rcp_${"4".repeat(32)}`,
  receiptB: `rcp_${"5".repeat(32)}`,
};

const contractIds = {
  operation: v7("op", "1"),
  receipt: v7("rcp", "2"),
  workspace: v7("wsp", "3"),
  profile: v7("prf", "4"),
  client: v7("cli", "5"),
  observation: v7("obs", "6"),
  evidence: v7("evd", "7"),
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
        assert.deepEqual(methods, [
          "acceptObservation",
          "configureListener",
          "constructor",
          "discoverCapabilities",
          "enrollFirstClient",
          "health",
          "initializeNode",
          "receiptEvents",
          "replayReceipt",
          "revokeCredential",
          "rotateCredential",
          "selectProfile",
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
        committed_at: "2026-02-30T03:00:00Z",
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

test("base URL semantics reject application paths instead of silently discarding them", () => {
  assert.throws(
    () => new FastiClient({ baseUrl: "http://127.0.0.1:8420/fasti" }),
    /without an application path/,
  );
  assert.doesNotThrow(
    () => new FastiClient({ baseUrl: "http://127.0.0.1:8420/" }),
  );
});

test("generated public metadata preserves complete registry and surface dispositions", () => {
  assert.equal(PUBLIC_CAPABILITY_REGISTRY.capabilities.length, 22);
  assert.equal(
    Object.keys(PUBLIC_CAPABILITY_REGISTRY.surface_profiles).length,
    7,
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
    binding: "json-ld:{capability_id}",
    state: "required",
  });
  assert.deepEqual(profile.okf, {
    binding: "okf:{capability_id}",
    state: "required",
  });
  assert.notStrictEqual(profile.json_ld, profile.okf);
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

test("all implemented B1 SDK routes complete against the loopback Rust fixture", async () => {
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
    assert.equal(discovery.capabilities.length, 22);
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
    await exercise(`http://${readiness.address}`);
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
