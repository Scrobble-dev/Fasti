import { createServer } from "node:http";

const server = createServer((request, response) => {
  response.setHeader("access-control-allow-origin", "*");
  response.setHeader(
    "access-control-allow-methods",
    "GET, HEAD, POST, PUT, DELETE, OPTIONS",
  );
  response.setHeader("access-control-allow-headers", "*");
  if (request.method === "OPTIONS") {
    response.writeHead(204).end();
    return;
  }
  const url = new URL(request.url, "http://127.0.0.1:18422");
  if (request.method !== "GET" && request.method !== "HEAD") {
    response.writeHead(405, { "content-type": "application/problem+json" });
    response.end(JSON.stringify({ title: "Method not allowed", status: 405 }));
    return;
  }

  let status = 200;
  let payload;
  if (url.pathname === "/api/v1/health") {
    payload = { status: "healthy", version: "0.1.0-test" };
  } else if (url.pathname === "/api/v1/profile/nuvio-collections") {
    payload = [];
  } else if (url.pathname === "/api/v1/profile/record-tracking-dispositions") {
    payload = [];
  } else if (
    url.pathname === "/api/v1/records" ||
    url.pathname.startsWith("/api/v1/records/")
  ) {
    payload = [];
  } else if (
    url.pathname === "/api/v1/reviews" ||
    url.pathname.startsWith("/api/v1/reviews/")
  ) {
    payload = [];
  } else if (url.pathname === "/api/v1/integrations") {
    payload = { integrations: [] };
  } else {
    status = 404;
    payload = { title: "Not found", status };
  }
  response.writeHead(status, {
    "content-type":
      status === 200 ? "application/json" : "application/problem+json",
  });
  response.end(request.method === "HEAD" ? undefined : JSON.stringify(payload));
});

server.on("clientError", (err, socket) => {
  if (err.code === "ECONNRESET" || !socket.writable) {
    return;
  }
  socket.end("HTTP/1.1 400 Bad Request\r\n\r\n");
});

server.on("error", (err) => {
  console.error("health-stub server error:", err);
});

process.on("uncaughtException", (err) => {
  console.error("health-stub uncaught exception:", err);
});

server.listen(18422, "127.0.0.1");
