import { cssVariables } from "@fasti/tokens";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";
import { themeBootstrapScript } from "./src/theme.js";

export default defineConfig({
  plugins: [
    {
      name: "fasti-theme-bootstrap",
      transformIndexHtml: {
        order: "pre",
        handler: () => [
          {
            tag: "style",
            children: `${cssVariables}\nhtml, body { background: var(--fasti-background); color: var(--fasti-foreground); }`,
            injectTo: "head-prepend",
          },
          {
            tag: "script",
            children: themeBootstrapScript,
            injectTo: "head-prepend",
          },
        ],
      },
    },
    svelte(),
  ],
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
    proxy: {
      "/api": {
        // Local QA default only. Runtime endpoint settings have a separate owner.
        // Playwright overrides this process-local value for its bounded stub.
        target: process.env.FASTI_QA_PROXY_TARGET ?? "http://127.0.0.1:8420",
        changeOrigin: true,
      },
    },
  },
});
