import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig, loadEnv } from "vite";

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, new URL(".", import.meta.url).pathname, ["FASTI_"]);
  const apiUrl = originUrl(
    env.FASTI_API_URL ?? "http://127.0.0.1:8420",
    "FASTI_API_URL",
  );
  const apiUrlManaged = env.FASTI_API_URL !== undefined;
  const webPort = portNumber(env.FASTI_WEB_PORT ?? "5173", "FASTI_WEB_PORT");

  return {
    plugins: [svelte()],
    clearScreen: false,
    define: {
      "import.meta.env.VITE_FASTI_API_URL": JSON.stringify(apiUrl),
      "import.meta.env.VITE_FASTI_API_URL_MANAGED":
        JSON.stringify(apiUrlManaged),
    },
    server: {
      port: webPort,
      proxy: {
        "/api": {
          target: apiUrl,
          changeOrigin: true,
        },
      },
    },
  };
});

function portNumber(value: string, name: string): number {
  if (!/^\d+$/.test(value)) throw new Error(`${name} must be a port number`);
  const port = Number(value);
  if (port < 1 || port > 65_535) {
    throw new Error(`${name} must be between 1 and 65535`);
  }
  return port;
}

function originUrl(value: string, name: string): string {
  const url = new URL(value);
  if (
    (url.protocol !== "http:" && url.protocol !== "https:") ||
    url.username !== "" ||
    url.password !== "" ||
    url.pathname !== "/" ||
    url.search !== "" ||
    url.hash !== ""
  ) {
    throw new Error(`${name} must be an HTTP or HTTPS origin URL`);
  }
  return url.origin;
}
