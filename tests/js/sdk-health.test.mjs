import assert from "node:assert/strict";
import { createServer } from "node:http";
import { after, before, test } from "node:test";

import { FastiClient } from "../../packages/sdk/dist/index.js";

let baseUrl;
let server;

before(async () => {
  server = createServer((request, response) => {
    if (request.url === "/api/v1/health") {
      response.writeHead(200, { "content-type": "application/json" });
      response.end(JSON.stringify({ status: "healthy", version: "0.1.0" }));
      return;
    }

    response.writeHead(404);
    response.end();
  });

  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  assert.notEqual(address, null);
  assert.equal(typeof address, "object");
  baseUrl = `http://127.0.0.1:${address.port}`;
});

after(async () => {
  await new Promise((resolve, reject) => {
    server.close((error) => (error ? reject(error) : resolve()));
  });
});

test("health returns the implemented daemon status", async () => {
  const client = new FastiClient({ baseUrl: `${baseUrl}/` });
  assert.deepEqual(await client.health(), {
    status: "healthy",
    version: "0.1.0",
  });
});
