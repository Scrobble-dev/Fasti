import { createServer } from "node:http";

const server = createServer((request, response) => {
  if (request.method !== "GET" || request.url !== "/api/v1/health") {
    response.writeHead(404).end();
    return;
  }
  response.writeHead(200, { "content-type": "application/json" });
  response.end(JSON.stringify({ status: "healthy", version: "0.1.0-test" }));
});

server.listen(18422, "127.0.0.1");
