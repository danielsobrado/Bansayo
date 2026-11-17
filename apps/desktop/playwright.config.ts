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
    launchOptions: process.env.BANSAYO_BROWSER_EXECUTABLE
      ? { executablePath: process.env.BANSAYO_BROWSER_EXECUTABLE }
      : { channel: "chrome" },
  },
  projects: [
    { name: "tracking", testIgnore: /training\.spec\.ts/ },
    {
      name: "training",
      testMatch: /training\.spec\.ts/,
      use: { baseURL: "http://127.0.0.1:15174" },
    },
  ],
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
    // The training journey runs against the training rig (config/sim-training.yaml).
    {
      command: "node tests/start-engine.mjs",
      env: {
        BANSAYO_E2E_CONFIG: "config/sim-training.yaml",
        BANSAYO_E2E_ENGINE_PORT: "17701",
        BANSAYO_E2E_UI_PORT: "15174",
      },
      url: "http://127.0.0.1:17701/api/v1/health",
      timeout: 180_000,
      reuseExistingServer: false,
    },
    {
      command: "npm run dev -- --host 127.0.0.1 --port 15174",
      url: "http://127.0.0.1:15174",
      env: { VITE_ENGINE_URL: "http://127.0.0.1:17701" },
      reuseExistingServer: false,
    },
  ],
});
