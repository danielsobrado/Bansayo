import { test, expect, type APIRequestContext } from "@playwright/test";

// Martial-arts 18 §3 Phase 1: pick technique → run a set → summary → attempt → progress, on the
// simulator training rig. The simulator kicks only while the session is active.
const API = "http://127.0.0.1:17701/api/v1";

async function motion(request: APIRequestContext, m: string) {
  const r = await request.put(`${API}/simulation`, { data: { motion: m } });
  expect(r.ok()).toBe(true);
}

test("pick a technique, run a set, review an attempt and see progress", async ({ page, request }) => {
  test.setTimeout(240_000);
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await motion(request, "standing");
  await page.goto("/#/train");
  await expect(page.getByText("Engine connected", { exact: false })).toBeVisible();

  // Athlete with motion-data consent.
  await page.getByLabel("Name", { exact: true }).fill("Ana");
  await page.getByRole("button", { name: "Create athlete", exact: true }).click();
  await expect(page.getByLabel("Active athlete")).toHaveValue("ana");

  // Pick: Taekwondo roundhouse kick, right, 1 × 3.
  await page.getByLabel("Technique", { exact: true }).selectOption("taekwondo.dollyeo_chagi");
  await page.getByLabel("Sets", { exact: true }).fill("1");
  await page.getByLabel("Repetitions", { exact: true }).fill("3");
  await page.getByLabel("Rest (s)", { exact: true }).fill("0");
  await page.getByRole("button", { name: "Prepare session", exact: true }).click();
  await expect(page).toHaveURL(/#\/train\/live$/);
  await expect(page.getByRole("region", { name: "Readiness" })).toBeVisible();
  // A new athlete acknowledges the technique's safety notes once (17 §4.3).
  await expect(page.getByRole("list", { name: "Safety notes" })).toContainText("Warm up");
  await page.getByRole("button", { name: "I have read the safety notes" }).click();
  const start = page.getByRole("button", { name: "▶ Start", exact: true });
  await expect(start).toBeEnabled({ timeout: 30_000 });
  await start.click();
  await expect(page.getByRole("heading", { level: 2, name: "active" })).toBeVisible({ timeout: 20_000 });
  await motion(request, "roundhouse_kick");
  await expect(page.getByTestId("rep-count")).toContainText("Rep 1 /", { timeout: 30_000 });
  await expect(page.getByRole("region", { name: "Last repetition" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Open the session summary" })).toBeVisible({ timeout: 60_000 });
  await motion(request, "standing");
  await page.screenshot({ path: "test-results/train-live.png" });

  // Summary → attempt.
  await page.getByRole("link", { name: "Open the session summary" }).click();
  const reps = page.getByRole("table", { name: "Repetitions" });
  await expect(reps.getByRole("link", { name: /Open attempt/ })).toHaveCount(3, { timeout: 20_000 });
  await reps.getByRole("link", { name: "Open attempt 2" }).click();
  await expect(page.getByRole("heading", { level: 1 })).toContainText("Attempt 2");
  await expect(page.getByTestId("phase-timeline")).toBeVisible();
  await expect(page.getByTestId("metric-table")).toContainText("Total time");
  await expect(page.getByRole("toolbar", { name: "Clip player" })).toBeVisible({ timeout: 20_000 });
  await page.getByRole("button", { name: "▶ Play" }).click();
  await expect(page.getByRole("button", { name: "⏸ Pause" })).toBeVisible();
  await page.screenshot({ path: "test-results/technique-attempt.png" });

  // Progress: the simulated session appears as one series class.
  await page.getByRole("link", { name: /Progress/ }).first().click();
  await expect(page.getByText("SIMULATED").first()).toBeVisible();
  await page.getByLabel("Technique", { exact: true }).selectOption("taekwondo.dollyeo_chagi");
  await page.getByLabel("Metric", { exact: true }).selectOption("kick.total_time");
  await expect(page.getByTestId("series-class")).toHaveCount(1, { timeout: 20_000 });
  await page.screenshot({ path: "test-results/progress.png" });

  for (const width of [1440, 1024, 800]) {
    await page.setViewportSize({ width, height: 900 });
    for (const route of ["train", "technique", "progress"]) {
      await page.goto(`/#/${route}`);
      await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    }
  }
  expect(errors).toEqual([]);
});
