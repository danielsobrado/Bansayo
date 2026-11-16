import { fileURLToPath } from "node:url";
import { loadEnv } from "vite";
import { defineConfig } from "vitest/config";

const DEFAULT_ENGINE_URL = "http://127.0.0.1:7700";

export default defineConfig(({ command, mode }) => {
  const env = loadEnv(mode, process.cwd(), "VITE_");
  const engineUrl = new URL(env.VITE_ENGINE_URL || DEFAULT_ENGINE_URL);
  const httpOrigin = engineUrl.origin;
  const wsOrigin = httpOrigin.replace(/^http/, "ws");
  // Vite's dev server injects CSS as <style> elements; the production build does not need it.
  const styleSrc = command === "serve" ? "'self' 'unsafe-inline'" : "'self'";
  const csp = [
    "default-src 'self'",
    "script-src 'self'",
    `style-src ${styleSrc}`,
    "font-src 'self'",
    "img-src 'self'",
    `connect-src 'self' ${httpOrigin} ${wsOrigin}`,
    "object-src 'none'",
    "base-uri 'self'",
    "form-action 'none'",
  ].join("; ");

  return {
    resolve: {
      alias: { "@contracts": fileURLToPath(new URL("../../contracts/ts/index.ts", import.meta.url)) },
    },
    define: { __ENGINE_URL__: JSON.stringify(httpOrigin) },
    server: { port: 5173, strictPort: true, fs: { allow: ["../.."] } },
    preview: { port: 5173, strictPort: true },
    build: { assetsInlineLimit: 0, sourcemap: false },
    plugins: [
      {
        name: "kinexa-csp",
        transformIndexHtml: (html) => html.replace("%KINEXA_CSP%", csp),
      },
    ],
    test: { include: ["src/**/*.test.ts"] },
  };
});
