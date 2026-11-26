import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir:"./tests/visual",
  testMatch:"sensor-studio.spec.ts",
  timeout:60_000,
  workers:1,
  outputDir:"test-results/sensor-studio",
  use:{baseURL:"http://127.0.0.1:15175",browserName:"chromium",viewport:{width:1440,height:900},screenshot:"only-on-failure",trace:"retain-on-failure"},
  webServer:{command:"npm run dev -- --host 127.0.0.1 --port 15175",url:"http://127.0.0.1:15175",env:{VITE_ENGINE_URL:"http://127.0.0.1:15175"},reuseExistingServer:false},
});
