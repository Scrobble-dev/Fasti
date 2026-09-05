import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { FastiClient } from "../../packages/sdk/dist/transport.js";

test("provider inventory alone admits browser reads in the generated provider routes", () => {
  const { paths } = JSON.parse(
    readFileSync(
      new URL("../../contracts/generated/v1/openapi.json", import.meta.url),
      "utf8",
    ),
  );
  const bearer = { credential_bearer: [] };
  assert.deepEqual(paths["/api/v1/providers"].get.security, [
    bearer,
    { browser_session_cookie: [] },
  ]);
  for (const [path, method] of [
    ["/api/v1/providers/{provider_id}/credentials/{capability_id}", "put"],
    ["/api/v1/providers/{provider_id}/credentials/{capability_id}", "delete"],
    [
      "/api/v1/providers/{provider_id}/credentials/{capability_id}/tests",
      "post",
    ],
    ["/api/v1/providers/{provider_id}/health", "get"],
  ])
    assert.deepEqual(paths[path][method].security, [bearer]);
});

test("provider inventory SDK read uses same-origin cookies without CSRF or a bearer fallback", async () => {
  let calls = 0;
  const client = new FastiClient({
    baseUrl: "http://127.0.0.1:8420",
    fetch: async (url, init) => {
      calls += 1;
      assert.equal(String(url), "http://127.0.0.1:8420/api/v1/providers");
      assert.equal(init.method, "GET");
      assert.equal(init.credentials, "same-origin");
      const headers = new Headers(init.headers);
      assert.equal(headers.get("authorization"), null);
      assert.equal(headers.get("x-csrf-token"), null);
      return new Response(JSON.stringify({ providers: [] }), {
        headers: {
          "content-type": "application/json",
          "cache-control": "private, no-store",
        },
      });
    },
  });
  assert.deepEqual(await client.listProviders(), { providers: [] });
  assert.equal(calls, 1);
});
