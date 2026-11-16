import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: "./tests/e2e",
  timeout: 60_000,
  expect: { timeout: 10_000 },
  workers: 1,
  fullyParallel: false,
  reporter: "list",
  use: {
    baseURL: "http://127.0.0.1:15173",
    browserName: "chromium",
    viewport: { width: 1440, height: 900 },
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
    launchOptions: process.env.KINEXA_BROWSER_EXECUTABLE
      ? { executablePath: process.env.KINEXA_BROWSER_EXECUTABLE }
      : { channel: "chrome" },
  },
  webServer: [
    {
      command: "node tests/start-engine.mjs",
      url: "http://127.0.0.1:17700/api/v1/health",
      timeout: 120_000,
      reuseExistingServer: false,
    },
    {
      command: "npm run dev -- --host 127.0.0.1 --port 15173",
      url: "http://127.0.0.1:15173",
      env: { VITE_ENGINE_URL: "http://127.0.0.1:17700" },
      reuseExistingServer: false,
    },
  ],
});
